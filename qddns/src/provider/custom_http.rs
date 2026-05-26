use std::collections::BTreeMap;

use crate::config::{CustomHttpConfig, ProviderConfig, RuleConfig};
use crate::error::{Error, Result};
use crate::http::{HttpClient, RetryPolicy};
use serde_json::Value;

use super::{
    execute_http_request, find_ip_in_text, lookup_retry_policy, parse_provider_response_json,
    render_template, RemoteRecord, SyncOutcome,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomHttpRequest {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

pub fn build_custom_http_request(
    provider: &ProviderConfig,
    rule: &RuleConfig,
    target_ip: &str,
) -> Result<CustomHttpRequest> {
    let custom = custom_http(provider)?;
    let url = custom
        .url
        .clone()
        .ok_or_else(|| Error::new(format!("provider '{}' missing url", provider.name)))?;
    let method = custom
        .method
        .clone()
        .unwrap_or_else(|| "POST".into())
        .to_uppercase();
    let headers = parse_headers(custom.headers_json.as_deref())?;
    let body_template = custom.body_template.clone().unwrap_or_default();
    let body = render_template(&body_template, rule, target_ip, None);

    Ok(CustomHttpRequest {
        method,
        url: render_template(&url, rule, target_ip, None),
        headers,
        body,
    })
}

pub fn parse_custom_http_success(provider: &ProviderConfig, body: &str) -> Result<bool> {
    match custom_http(provider)?.success_contains.as_deref() {
        Some(needle) if !needle.is_empty() => Ok(body.contains(needle)),
        _ => Ok(true),
    }
}

pub(crate) fn fetch_record(
    http: &HttpClient,
    provider: &ProviderConfig,
    rule: &RuleConfig,
) -> Result<RemoteRecord> {
    let custom = custom_http(provider)?;
    let lookup_url = custom
        .lookup_url
        .as_deref()
        .or(custom.url.as_deref())
        .ok_or_else(|| {
            Error::new(format!(
                "provider '{}' missing lookup_url or url for custom_http",
                provider.name
            ))
        })?;
    let method = custom
        .lookup_method
        .as_deref()
        .or(custom.method.as_deref())
        .unwrap_or("GET")
        .to_uppercase();
    let headers = parse_headers(
        custom
            .lookup_headers_json
            .as_deref()
            .or(custom.headers_json.as_deref()),
    )?;
    let rendered_url = render_template(lookup_url, rule, "", None);
    let body = if method == "GET" || method == "HEAD" {
        String::new()
    } else {
        render_template(
            custom.body_template.as_deref().unwrap_or(""),
            rule,
            "",
            None,
        )
    };
    let response = execute_http_request(
        http,
        lookup_retry_policy(rule),
        &method,
        &rendered_url,
        &headers,
        &body,
    )?;
    let address = extract_lookup_address(provider, &response.body)?;

    Ok(RemoteRecord {
        address,
        record_id: None,
        detail: format!("custom_http lookup status={}", response.status),
    })
}

pub(crate) fn update_record(
    http: &HttpClient,
    provider: &ProviderConfig,
    rule: &RuleConfig,
    remote: &RemoteRecord,
    target_ip: &str,
) -> Result<SyncOutcome> {
    let request = build_custom_http_request(provider, rule, target_ip)?;
    let response = execute_http_request(
        http,
        RetryPolicy::none(),
        &request.method,
        &request.url,
        &request.headers,
        &request.body,
    )?;
    let ok = parse_custom_http_success(provider, &response.body)?;
    if !ok {
        return Err(Error::new(format!(
            "custom_http update for '{}' did not match success marker",
            rule.name
        )));
    }

    Ok(SyncOutcome {
        changed: remote.address.as_deref() != Some(target_ip),
        remote_before: remote.address.clone(),
        remote_after: target_ip.into(),
        detail: format!("custom_http updated status={}", response.status),
    })
}

fn custom_http(provider: &ProviderConfig) -> Result<&CustomHttpConfig> {
    provider.custom_http().ok_or_else(|| {
        Error::new(format!(
            "provider '{}' is not a custom_http provider",
            provider.name
        ))
    })
}

fn parse_headers(raw: Option<&str>) -> Result<BTreeMap<String, String>> {
    let mut headers = BTreeMap::new();
    let Some(raw) = raw else {
        return Ok(headers);
    };
    if raw.trim().is_empty() {
        return Ok(headers);
    }
    let value = parse_provider_response_json(raw)?;
    let obj = value
        .as_object()
        .ok_or_else(|| Error::new("headers_json must be a JSON object"))?;
    for (key, value) in obj {
        let text = value
            .as_str()
            .ok_or_else(|| Error::new("headers_json values must be strings"))?;
        headers.insert(key.clone(), text.to_string());
    }
    Ok(headers)
}

fn extract_lookup_address(provider: &ProviderConfig, body: &str) -> Result<Option<String>> {
    if let Some(pointer_path) = custom_http(provider)?.lookup_json_pointer.as_deref() {
        let json_value = parse_provider_response_json(body)?;
        let value = json_value.pointer(pointer_path).ok_or_else(|| {
            Error::new(format!(
                "custom_http lookup_json_pointer '{}' not found",
                pointer_path
            ))
        })?;
        return match value {
            Value::Null => Ok(None),
            Value::String(text) => Ok(Some(text.clone())),
            Value::Number(number) => Ok(Some(number.to_string())),
            _ => Err(Error::new(
                "custom_http lookup_json_pointer must resolve to string or number",
            )),
        };
    }

    Ok(find_ip_in_text(body))
}
