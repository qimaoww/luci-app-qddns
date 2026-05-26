use serde_json::{json, Value};

use crate::config::{ProviderConfig, RuleConfig};
use crate::error::{Error, Result};
use crate::http::{HttpClient, RetryPolicy};

use super::{
    auth_headers, encode_component, execute_http_request, fqdn, lookup_retry_policy,
    parse_provider_response_json, redact_provider_error, RemoteRecord, SyncOutcome,
};

pub(crate) fn fetch_record(
    http: &HttpClient,
    provider: &ProviderConfig,
    rule: &RuleConfig,
) -> Result<RemoteRecord> {
    let token = provider
        .cloudflare_api_token()
        .ok_or_else(|| Error::new(format!("provider '{}' missing api_token", provider.name)))?;
    let zone_name = encode_component(&rule.zone);
    let record_name = encode_component(&fqdn(rule));
    let record_type = encode_component(rule.record_type.as_str());
    let zone_resp = execute_http_request(
        http,
        lookup_retry_policy(rule),
        "GET",
        &format!("https://api.cloudflare.com/client/v4/zones?name={zone_name}&status=active"),
        &auth_headers("Authorization", &format!("Bearer {token}")),
        "",
    )?;
    let zone_json = parse_provider_response_json(&zone_resp.body)?;
    ensure_success(&zone_json, &[token])?;
    let zone_id = extract_zone_id(&zone_json)?;

    let record_resp = execute_http_request(
        http,
        lookup_retry_policy(rule),
        "GET",
        &format!(
            "https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records?type={record_type}&name={record_name}"
        ),
        &auth_headers("Authorization", &format!("Bearer {token}")),
        "",
    )?;
    let record_json = parse_provider_response_json(&record_resp.body)?;
    ensure_success(&record_json, &[token])?;
    let (address, record_id) = extract_record(&record_json)?;

    Ok(RemoteRecord {
        address,
        record_id,
        detail: format!("cloudflare zone={}", rule.zone),
    })
}

pub(crate) fn update_record(
    http: &HttpClient,
    provider: &ProviderConfig,
    rule: &RuleConfig,
    remote: &RemoteRecord,
    target_ip: &str,
) -> Result<SyncOutcome> {
    let token = provider
        .cloudflare_api_token()
        .ok_or_else(|| Error::new(format!("provider '{}' missing api_token", provider.name)))?;
    let zone_name = encode_component(&rule.zone);
    let zone_resp = execute_http_request(
        http,
        lookup_retry_policy(rule),
        "GET",
        &format!("https://api.cloudflare.com/client/v4/zones?name={zone_name}&status=active"),
        &auth_headers("Authorization", &format!("Bearer {token}")),
        "",
    )?;
    let zone_json = parse_provider_response_json(&zone_resp.body)?;
    ensure_success(&zone_json, &[token])?;
    let zone_id = extract_zone_id(&zone_json)?;

    let payload = build_update_payload(rule, target_ip);
    let method;
    let url;
    if let Some(record_id) = remote.record_id.as_deref() {
        method = "PUT";
        url =
            format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records/{record_id}");
    } else {
        method = "POST";
        url = format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records");
    }

    let mut headers = auth_headers("Authorization", &format!("Bearer {token}"));
    headers.insert("Content-Type".into(), "application/json".into());
    let response =
        execute_http_request(http, RetryPolicy::none(), method, &url, &headers, &payload)?;
    let response_json = parse_provider_response_json(&response.body)?;
    ensure_success(&response_json, &[token])?;

    Ok(SyncOutcome {
        changed: remote.address.as_deref() != Some(target_ip),
        remote_before: remote.address.clone(),
        remote_after: target_ip.into(),
        detail: format!("cloudflare {} {}", method, fqdn(rule)),
    })
}

fn build_update_payload(rule: &RuleConfig, target_ip: &str) -> String {
    json!({
        "type": rule.record_type.as_str(),
        "name": fqdn(rule),
        "content": target_ip,
        "ttl": rule.ttl,
        "proxied": rule.proxied,
    })
    .to_string()
}

fn ensure_success(value: &Value, secrets: &[&str]) -> Result<()> {
    let obj = value
        .as_object()
        .ok_or_else(|| Error::new("cloudflare response must be JSON object"))?;
    if obj.get("success").and_then(Value::as_bool).unwrap_or(false) {
        return Ok(());
    }

    let error_message = obj
        .get("errors")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_object)
        .and_then(|map| map.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("unknown cloudflare error");
    Err(Error::new(redact_provider_error(error_message, secrets)))
}

fn extract_zone_id(value: &Value) -> Result<String> {
    value
        .as_object()
        .and_then(|obj| obj.get("result"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("id"))
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .ok_or_else(|| Error::new("cloudflare zone not found"))
}

fn extract_record(value: &Value) -> Result<(Option<String>, Option<String>)> {
    let Some(first) = value
        .as_object()
        .and_then(|obj| obj.get("result"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
    else {
        return Ok((None, None));
    };

    let address = first
        .as_object()
        .and_then(|obj| obj.get("content"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let record_id = first
        .as_object()
        .and_then(|obj| obj.get("id"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    Ok((address, record_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RuleConfig;
    use serde_json::json;

    fn rule() -> RuleConfig {
        RuleConfig {
            name: "home".into(),
            enabled: true,
            provider: "cf".into(),
            source: "wan4".into(),
            record_type: "A".into(),
            zone: "example.com".into(),
            record_name: "home".into(),
            ttl: 300,
            proxied: false,
            check_interval: 60,
            force_interval: 3600,
            retry_count: 3,
            retry_backoff: 30,
        }
    }

    #[test]
    fn cloudflare_update_payload_uses_rule_shape() {
        let payload = build_update_payload(&rule(), "198.51.100.9");
        let value: Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(value["type"], "A");
        assert_eq!(value["name"], "home.example.com");
        assert_eq!(value["content"], "198.51.100.9");
        assert_eq!(value["ttl"], 300);
        assert_eq!(value["proxied"], false);
    }

    #[test]
    fn cloudflare_error_parser_redacts_provider_secret() {
        let secret = "test-secret-123";
        let err = ensure_success(
            &json!({"success": false, "errors": [{"message": format!("bad token {secret}")}]}),
            &[secret],
        )
        .expect_err("cloudflare error should fail");

        let text = err.to_string();
        assert!(!text.contains(secret), "secret leaked: {text}");
        assert!(
            text.contains("[redacted]"),
            "missing redaction marker: {text}"
        );
    }

    #[test]
    fn cloudflare_record_parser_reads_first_record() {
        let (address, record_id) = extract_record(&json!({
            "result": [{"id": "record-1", "content": "198.51.100.9"}]
        }))
        .unwrap();

        assert_eq!(address.as_deref(), Some("198.51.100.9"));
        assert_eq!(record_id.as_deref(), Some("record-1"));
    }
}
