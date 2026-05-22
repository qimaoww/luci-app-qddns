use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use crate::config::{ProviderConfig, RuleConfig};
use crate::error::{Error, Result};
use crate::json::{self, JsonValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRecord {
    pub address: Option<String>,
    pub record_id: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    pub changed: bool,
    pub remote_before: Option<String>,
    pub remote_after: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomHttpRequest {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

pub trait ProviderAdapter: Send + Sync {
    fn fetch_record(&self, provider: &ProviderConfig, rule: &RuleConfig) -> Result<RemoteRecord>;
    fn update_record(
        &self,
        provider: &ProviderConfig,
        rule: &RuleConfig,
        remote: &RemoteRecord,
        target_ip: &str,
    ) -> Result<SyncOutcome>;
}

#[derive(Debug, Default)]
pub struct ShellProviderAdapter;

impl ProviderAdapter for ShellProviderAdapter {
    fn fetch_record(&self, provider: &ProviderConfig, rule: &RuleConfig) -> Result<RemoteRecord> {
        match provider.provider_type.as_str() {
            "custom_http" => fetch_custom_http(provider, rule),
            "cloudflare" => fetch_cloudflare(provider, rule),
            "dnspod" => fetch_dnspod(provider, rule),
            "aliyun" => fetch_aliyun(provider, rule),
            other => Err(Error::new(format!(
                "provider '{}' has unsupported type '{other}'",
                provider.name
            ))),
        }
    }

    fn update_record(
        &self,
        provider: &ProviderConfig,
        rule: &RuleConfig,
        remote: &RemoteRecord,
        target_ip: &str,
    ) -> Result<SyncOutcome> {
        match provider.provider_type.as_str() {
            "custom_http" => update_custom_http(provider, rule, remote, target_ip),
            "cloudflare" => update_cloudflare(provider, rule, remote, target_ip),
            "dnspod" => update_dnspod(provider, rule, remote, target_ip),
            "aliyun" => update_aliyun(provider, rule, remote, target_ip),
            other => Err(Error::new(format!(
                "provider '{}' has unsupported type '{other}'",
                provider.name
            ))),
        }
    }
}

pub fn build_custom_http_request(
    provider: &ProviderConfig,
    rule: &RuleConfig,
    target_ip: &str,
) -> Result<CustomHttpRequest> {
    let url = provider
        .url
        .clone()
        .ok_or_else(|| Error::new(format!("provider '{}' missing url", provider.name)))?;
    let method = provider
        .method
        .clone()
        .unwrap_or_else(|| "POST".into())
        .to_uppercase();
    let headers = parse_headers(provider.headers_json.as_deref())?;
    let body_template = provider.body_template.clone().unwrap_or_default();
    let body = render_template(&body_template, rule, target_ip, None);

    Ok(CustomHttpRequest {
        method,
        url: render_template(&url, rule, target_ip, None),
        headers,
        body,
    })
}

pub fn parse_custom_http_success(provider: &ProviderConfig, body: &str) -> Result<bool> {
    match provider.success_contains.as_deref() {
        Some(needle) if !needle.is_empty() => Ok(body.contains(needle)),
        _ => Ok(true),
    }
}

fn fetch_custom_http(provider: &ProviderConfig, rule: &RuleConfig) -> Result<RemoteRecord> {
    let lookup_url = provider.lookup_url.as_deref().or(provider.url.as_deref()).ok_or_else(|| {
        Error::new(format!(
            "provider '{}' missing lookup_url or url for custom_http",
            provider.name
        ))
    })?;
    let method = provider
        .lookup_method
        .as_deref()
        .or(provider.method.as_deref())
        .unwrap_or("GET")
        .to_uppercase();
    let headers = parse_headers(
        provider
            .lookup_headers_json
            .as_deref()
            .or(provider.headers_json.as_deref()),
    )?;
    let rendered_url = render_template(lookup_url, rule, "", None);
    let body = if method == "GET" || method == "HEAD" {
        String::new()
    } else {
        render_template(
            provider.body_template.as_deref().unwrap_or(""),
            rule,
            "",
            None,
        )
    };
    let response = execute_http_request(&method, &rendered_url, &headers, &body)?;
    let address = extract_custom_http_lookup_address(provider, &response.stdout)?;

    Ok(RemoteRecord {
        address,
        record_id: None,
        detail: format!("custom_http lookup status={}", response.status),
    })
}

fn update_custom_http(
    provider: &ProviderConfig,
    rule: &RuleConfig,
    remote: &RemoteRecord,
    target_ip: &str,
) -> Result<SyncOutcome> {
    let request = build_custom_http_request(provider, rule, target_ip)?;
    let response = execute_http_request(&request.method, &request.url, &request.headers, &request.body)?;
    let ok = parse_custom_http_success(provider, &response.stdout)?;
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

fn fetch_cloudflare(provider: &ProviderConfig, rule: &RuleConfig) -> Result<RemoteRecord> {
    let token = provider
        .api_token
        .as_deref()
        .ok_or_else(|| Error::new(format!("provider '{}' missing api_token", provider.name)))?;
    let zone_name = encode_component(&rule.zone);
    let record_name = encode_component(&fqdn(rule));
    let record_type = encode_component(&rule.record_type);
    let zone_resp = execute_http_request(
        "GET",
        &format!(
            "https://api.cloudflare.com/client/v4/zones?name={zone_name}&status=active"
        ),
        &auth_headers("Authorization", &format!("Bearer {token}")),
        "",
    )?;
    let zone_json = json::parse(&zone_resp.stdout)?;
    ensure_cloudflare_success(&zone_json)?;
    let zone_id = extract_cf_zone_id(&zone_json)?;

    let record_resp = execute_http_request(
        "GET",
        &format!(
            "https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records?type={record_type}&name={record_name}"
        ),
        &auth_headers("Authorization", &format!("Bearer {token}")),
        "",
    )?;
    let record_json = json::parse(&record_resp.stdout)?;
    ensure_cloudflare_success(&record_json)?;
    let (address, record_id) = extract_cf_record(&record_json)?;

    Ok(RemoteRecord {
        address,
        record_id,
        detail: format!("cloudflare zone={}", rule.zone),
    })
}

fn update_cloudflare(
    provider: &ProviderConfig,
    rule: &RuleConfig,
    remote: &RemoteRecord,
    target_ip: &str,
) -> Result<SyncOutcome> {
    let token = provider
        .api_token
        .as_deref()
        .ok_or_else(|| Error::new(format!("provider '{}' missing api_token", provider.name)))?;
    let zone_name = encode_component(&rule.zone);
    let zone_resp = execute_http_request(
        "GET",
        &format!(
            "https://api.cloudflare.com/client/v4/zones?name={zone_name}&status=active"
        ),
        &auth_headers("Authorization", &format!("Bearer {token}")),
        "",
    )?;
    let zone_json = json::parse(&zone_resp.stdout)?;
    ensure_cloudflare_success(&zone_json)?;
    let zone_id = extract_cf_zone_id(&zone_json)?;

    let payload = build_cf_update_payload(rule, target_ip);
    let method;
    let url;
    if let Some(record_id) = remote.record_id.as_deref() {
        method = "PUT";
        url = format!(
            "https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records/{record_id}"
        );
    } else {
        method = "POST";
        url = format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records");
    }

    let mut headers = auth_headers("Authorization", &format!("Bearer {token}"));
    headers.insert("Content-Type".into(), "application/json".into());
    let response = execute_http_request(method, &url, &headers, &payload)?;
    let response_json = json::parse(&response.stdout)?;
    ensure_cloudflare_success(&response_json)?;

    Ok(SyncOutcome {
        changed: remote.address.as_deref() != Some(target_ip),
        remote_before: remote.address.clone(),
        remote_after: target_ip.into(),
        detail: format!("cloudflare {} {}", method, fqdn(rule)),
    })
}

fn fetch_dnspod(provider: &ProviderConfig, rule: &RuleConfig) -> Result<RemoteRecord> {
    let secret_id = provider
        .secret_id
        .as_deref()
        .ok_or_else(|| Error::new(format!("provider '{}' missing secret_id", provider.name)))?;
    let secret_key = provider
        .secret_key
        .as_deref()
        .ok_or_else(|| Error::new(format!("provider '{}' missing secret_key", provider.name)))?;
    let body = format!(
        "{{\"Domain\":\"{}\",\"Subdomain\":\"{}\",\"RecordType\":\"{}\"}}",
        json::escape_string(&rule.zone),
        json::escape_string(&rule.record_name),
        json::escape_string(&rule.record_type)
    );
    let response = execute_tencent_json_api(
        "dnspod.tencentcloudapi.com",
        "DescribeRecordList",
        &body,
        secret_id,
        secret_key,
    )?;
    let json_value = json::parse(&response.stdout)?;
    ensure_tencent_success(&json_value)?;
    let (address, record_id) = extract_dnspod_record(&json_value, rule)?;

    Ok(RemoteRecord {
        address,
        record_id,
        detail: format!("dnspod domain={}", rule.zone),
    })
}

fn update_dnspod(
    provider: &ProviderConfig,
    rule: &RuleConfig,
    remote: &RemoteRecord,
    target_ip: &str,
) -> Result<SyncOutcome> {
    let secret_id = provider
        .secret_id
        .as_deref()
        .ok_or_else(|| Error::new(format!("provider '{}' missing secret_id", provider.name)))?;
    let secret_key = provider
        .secret_key
        .as_deref()
        .ok_or_else(|| Error::new(format!("provider '{}' missing secret_key", provider.name)))?;

    let action = if remote.record_id.is_some() {
        "ModifyRecord"
    } else {
        "CreateRecord"
    };
    let body = if let Some(record_id) = remote.record_id.as_deref() {
        format!(
            "{{\"Domain\":\"{}\",\"Subdomain\":\"{}\",\"RecordType\":\"{}\",\"RecordLine\":\"默认\",\"RecordId\":{},\"Value\":\"{}\",\"TTL\":{}}}",
            json::escape_string(&rule.zone),
            json::escape_string(&rule.record_name),
            json::escape_string(&rule.record_type),
            record_id,
            json::escape_string(target_ip),
            rule.ttl
        )
    } else {
        format!(
            "{{\"Domain\":\"{}\",\"Subdomain\":\"{}\",\"RecordType\":\"{}\",\"RecordLine\":\"默认\",\"Value\":\"{}\",\"TTL\":{}}}",
            json::escape_string(&rule.zone),
            json::escape_string(&rule.record_name),
            json::escape_string(&rule.record_type),
            json::escape_string(target_ip),
            rule.ttl
        )
    };
    let response = execute_tencent_json_api(
        "dnspod.tencentcloudapi.com",
        action,
        &body,
        secret_id,
        secret_key,
    )?;
    let json_value = json::parse(&response.stdout)?;
    ensure_tencent_success(&json_value)?;

    Ok(SyncOutcome {
        changed: remote.address.as_deref() != Some(target_ip),
        remote_before: remote.address.clone(),
        remote_after: target_ip.into(),
        detail: format!("dnspod {} {}", action, fqdn(rule)),
    })
}

fn fetch_aliyun(provider: &ProviderConfig, rule: &RuleConfig) -> Result<RemoteRecord> {
    let access_key_id = provider.access_key_id.as_deref().ok_or_else(|| {
        Error::new(format!("provider '{}' missing access_key_id", provider.name))
    })?;
    let access_key_secret = provider.access_key_secret.as_deref().ok_or_else(|| {
        Error::new(format!(
            "provider '{}' missing access_key_secret",
            provider.name
        ))
    })?;
    let rr = if rule.record_name == "@" {
        "".to_string()
    } else {
        rule.record_name.clone()
    };
    let response = execute_aliyun_api(
        "DescribeSubDomainRecords",
        &[
            ("SubDomain", fqdn(rule)),
            ("Type", rule.record_type.clone()),
        ],
        access_key_id,
        access_key_secret,
    )?;
    let json_value = json::parse(&response.stdout)?;
    let (address, record_id) = extract_aliyun_record(&json_value, &rr)?;

    Ok(RemoteRecord {
        address,
        record_id,
        detail: format!("aliyun subdomain={}", fqdn(rule)),
    })
}

fn update_aliyun(
    provider: &ProviderConfig,
    rule: &RuleConfig,
    remote: &RemoteRecord,
    target_ip: &str,
) -> Result<SyncOutcome> {
    let access_key_id = provider.access_key_id.as_deref().ok_or_else(|| {
        Error::new(format!("provider '{}' missing access_key_id", provider.name))
    })?;
    let access_key_secret = provider.access_key_secret.as_deref().ok_or_else(|| {
        Error::new(format!(
            "provider '{}' missing access_key_secret",
            provider.name
        ))
    })?;
    let rr = if rule.record_name == "@" {
        "".to_string()
    } else {
        rule.record_name.clone()
    };

    let action;
    let mut params = vec![
        ("RR", rr.clone()),
        ("Type", rule.record_type.clone()),
        ("Value", target_ip.to_string()),
        ("TTL", rule.ttl.to_string()),
        ("DomainName", rule.zone.clone()),
    ];
    if let Some(record_id) = remote.record_id.as_deref() {
        action = "UpdateDomainRecord";
        params.push(("RecordId", record_id.to_string()));
    } else {
        action = "AddDomainRecord";
    }

    let response = execute_aliyun_api(action, &params, access_key_id, access_key_secret)?;
    let json_value = json::parse(&response.stdout)?;
    ensure_aliyun_success(&json_value)?;

    Ok(SyncOutcome {
        changed: remote.address.as_deref() != Some(target_ip),
        remote_before: remote.address.clone(),
        remote_after: target_ip.into(),
        detail: format!("aliyun {} {}", action, fqdn(rule)),
    })
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    stdout: String,
}

fn execute_http_request(
    method: &str,
    url: &str,
    headers: &BTreeMap<String, String>,
    body: &str,
) -> Result<HttpResponse> {
    if let Some(path) = url.strip_prefix("file://") {
        return execute_file_request(method, path, headers, body);
    }

    let mut command = Command::new("curl");
    command.args(["-sS", "-X", method, "-w", "\n__QDDNS_STATUS__:%{http_code}"]);
    for (name, value) in headers {
        command.args(["-H", &format!("{name}: {value}")]);
    }
    if !body.is_empty() && method != "GET" && method != "HEAD" {
        command.args(["--data", body]);
    }
    command.arg(url);

    let output = command
        .output()
        .map_err(|err| Error::new(format!("failed to execute curl: {err}")))?;
    if !output.status.success() {
        return Err(Error::new(format!(
            "curl request failed with status {}",
            output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let marker = "\n__QDDNS_STATUS__:";
    let (body_text, status_text) = stdout
        .rsplit_once(marker)
        .ok_or_else(|| Error::new("curl response missing status marker"))?;
    let status = status_text
        .trim()
        .parse::<u16>()
        .map_err(|err| Error::new(format!("invalid HTTP status from curl: {err}")))?;
    if status >= 400 {
        return Err(Error::new(format!(
            "HTTP {} from provider endpoint: {}",
            status,
            body_text.trim()
        )));
    }
    Ok(HttpResponse {
        status,
        stdout: body_text.to_string(),
    })
}

fn execute_file_request(
    method: &str,
    path: &str,
    _headers: &BTreeMap<String, String>,
    body: &str,
) -> Result<HttpResponse> {
    let upper = method.to_uppercase();
    match upper.as_str() {
        "GET" | "HEAD" => {
            let stdout = fs::read_to_string(path)
                .map_err(|err| Error::new(format!("failed to read file source '{path}': {err}")))?;
            Ok(HttpResponse { status: 200, stdout })
        }
        _ => {
            fs::write(path, body)
                .map_err(|err| Error::new(format!("failed to write file source '{path}': {err}")))?;
            Ok(HttpResponse {
                status: 200,
                stdout: body.to_string(),
            })
        }
    }
}

fn parse_headers(raw: Option<&str>) -> Result<BTreeMap<String, String>> {
    let mut headers = BTreeMap::new();
    let Some(raw) = raw else {
        return Ok(headers);
    };
    if raw.trim().is_empty() {
        return Ok(headers);
    }
    let value = json::parse(raw)?;
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

fn extract_custom_http_lookup_address(provider: &ProviderConfig, body: &str) -> Result<Option<String>> {
    if let Some(pointer_path) = provider.lookup_json_pointer.as_deref() {
        let json_value = json::parse(body)?;
        let value = json::pointer(&json_value, pointer_path).ok_or_else(|| {
            Error::new(format!(
                "custom_http lookup_json_pointer '{}' not found",
                pointer_path
            ))
        })?;
        return match value {
            JsonValue::Null => Ok(None),
            JsonValue::String(text) => Ok(Some(text.clone())),
            JsonValue::Number(number) => Ok(Some(number.clone())),
            _ => Err(Error::new("custom_http lookup_json_pointer must resolve to string or number")),
        };
    }

    Ok(find_ip_in_text(body))
}

fn render_template(
    template: &str,
    rule: &RuleConfig,
    ip: &str,
    remote: Option<&RemoteRecord>,
) -> String {
    let remote_ip = remote.and_then(|value| value.address.as_deref()).unwrap_or("");
    let record_id = remote.and_then(|value| value.record_id.as_deref()).unwrap_or("");
    let fqdn = fqdn(rule);
    template
        .replace("{{ip}}", ip)
        .replace("{{zone}}", &rule.zone)
        .replace("{{record_name}}", &rule.record_name)
        .replace("{{fqdn}}", &fqdn)
        .replace("{{record_type}}", &rule.record_type)
        .replace("{{ttl}}", &rule.ttl.to_string())
        .replace("{{proxied}}", if rule.proxied { "true" } else { "false" })
        .replace("{{remote_ip}}", remote_ip)
        .replace("{{record_id}}", record_id)
}

fn fqdn(rule: &RuleConfig) -> String {
    if rule.record_name == "@" {
        rule.zone.clone()
    } else {
        format!("{}.{}", rule.record_name, rule.zone)
    }
}

fn build_cf_update_payload(rule: &RuleConfig, target_ip: &str) -> String {
    format!(
        "{{\"type\":\"{}\",\"name\":\"{}\",\"content\":\"{}\",\"ttl\":{},\"proxied\":{}}}",
        json::escape_string(&rule.record_type),
        json::escape_string(&fqdn(rule)),
        json::escape_string(target_ip),
        rule.ttl,
        if rule.proxied { "true" } else { "false" }
    )
}

fn ensure_cloudflare_success(value: &JsonValue) -> Result<()> {
    let obj = value
        .as_object()
        .ok_or_else(|| Error::new("cloudflare response must be JSON object"))?;
    if obj
        .get("success")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false)
    {
        return Ok(());
    }

    let error_message = obj
        .get("errors")
        .and_then(JsonValue::as_array)
        .and_then(|items| items.first())
        .and_then(JsonValue::as_object)
        .and_then(|map| map.get("message"))
        .and_then(JsonValue::as_str)
        .unwrap_or("unknown cloudflare error");
    Err(Error::new(error_message))
}

fn extract_cf_zone_id(value: &JsonValue) -> Result<String> {
    value
        .as_object()
        .and_then(|obj| obj.get("result"))
        .and_then(JsonValue::as_array)
        .and_then(|items| items.first())
        .and_then(JsonValue::as_object)
        .and_then(|obj| obj.get("id"))
        .and_then(JsonValue::as_str)
        .map(|value| value.to_string())
        .ok_or_else(|| Error::new("cloudflare zone not found"))
}

fn extract_cf_record(value: &JsonValue) -> Result<(Option<String>, Option<String>)> {
    let Some(first) = value
        .as_object()
        .and_then(|obj| obj.get("result"))
        .and_then(JsonValue::as_array)
        .and_then(|items| items.first())
    else {
        return Ok((None, None));
    };

    let address = first
        .as_object()
        .and_then(|obj| obj.get("content"))
        .and_then(JsonValue::as_str)
        .map(ToString::to_string);
    let record_id = first
        .as_object()
        .and_then(|obj| obj.get("id"))
        .and_then(JsonValue::as_str)
        .map(ToString::to_string);
    Ok((address, record_id))
}

fn ensure_tencent_success(value: &JsonValue) -> Result<()> {
    let response = value
        .as_object()
        .and_then(|obj| obj.get("Response"))
        .and_then(JsonValue::as_object)
        .ok_or_else(|| Error::new("tencent response missing Response object"))?;
    if let Some(err_obj) = response.get("Error").and_then(JsonValue::as_object) {
        let code = err_obj
            .get("Code")
            .and_then(JsonValue::as_str)
            .unwrap_or("TencentError");
        let message = err_obj
            .get("Message")
            .and_then(JsonValue::as_str)
            .unwrap_or("unknown tencent error");
        return Err(Error::new(format!("{code}: {message}")));
    }
    Ok(())
}

fn extract_dnspod_record(value: &JsonValue, rule: &RuleConfig) -> Result<(Option<String>, Option<String>)> {
    let records = value
        .as_object()
        .and_then(|obj| obj.get("Response"))
        .and_then(JsonValue::as_object)
        .and_then(|obj| obj.get("RecordList"))
        .and_then(JsonValue::as_array)
        .ok_or_else(|| Error::new("dnspod response missing RecordList"))?;

    let target_name = rule.record_name.as_str();
    for item in records {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let record_type = obj.get("Type").and_then(JsonValue::as_str).unwrap_or("");
        let subdomain = obj.get("Name").and_then(JsonValue::as_str).unwrap_or("");
        if record_type != rule.record_type || subdomain != target_name {
            continue;
        }
        let address = obj
            .get("Value")
            .and_then(JsonValue::as_str)
            .map(ToString::to_string);
        let record_id = match obj.get("RecordId") {
            Some(JsonValue::String(text)) => Some(text.clone()),
            Some(JsonValue::Number(number)) => Some(number.clone()),
            _ => None,
        };
        return Ok((address, record_id));
    }

    Ok((None, None))
}

fn ensure_aliyun_success(value: &JsonValue) -> Result<()> {
    let obj = value
        .as_object()
        .ok_or_else(|| Error::new("aliyun response must be object"))?;
    if let Some(code) = obj.get("Code").and_then(JsonValue::as_str) {
        let message = obj
            .get("Message")
            .and_then(JsonValue::as_str)
            .unwrap_or("unknown aliyun error");
        return Err(Error::new(format!("{code}: {message}")));
    }
    Ok(())
}

fn extract_aliyun_record(value: &JsonValue, rr: &str) -> Result<(Option<String>, Option<String>)> {
    ensure_aliyun_success(value)?;
    let records = value
        .as_object()
        .and_then(|obj| obj.get("DomainRecords"))
        .and_then(JsonValue::as_object)
        .and_then(|obj| obj.get("Record"))
        .and_then(JsonValue::as_array)
        .ok_or_else(|| Error::new("aliyun response missing DomainRecords.Record"))?;
    for item in records {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let current_rr = obj.get("RR").and_then(JsonValue::as_str).unwrap_or("");
        if current_rr != rr {
            continue;
        }
        let address = obj
            .get("Value")
            .and_then(JsonValue::as_str)
            .map(ToString::to_string);
        let record_id = obj
            .get("RecordId")
            .and_then(JsonValue::as_str)
            .map(ToString::to_string);
        return Ok((address, record_id));
    }
    Ok((None, None))
}

fn execute_tencent_json_api(
    host: &str,
    action: &str,
    body: &str,
    secret_id: &str,
    secret_key: &str,
) -> Result<HttpResponse> {
    let timestamp = unix_now();
    let date = iso_date(timestamp);
    let service = host.split('.').next().unwrap_or("dnspod");
    let hashed_payload = openssl_digest("sha256", body)?;
    let canonical_headers = format!(
        "content-type:application/json; charset=utf-8\nhost:{host}\n"
    );
    let signed_headers = "content-type;host";
    let canonical_request = format!(
        "POST\n/\n\n{canonical_headers}\n{signed_headers}\n{hashed_payload}"
    );
    let credential_scope = format!("{date}/{service}/tc3_request");
    let string_to_sign = format!(
        "TC3-HMAC-SHA256\n{timestamp}\n{credential_scope}\n{}",
        openssl_digest("sha256", &canonical_request)?
    );

    let secret_date = openssl_hmac_hex("sha256", format!("TC3{secret_key}").as_bytes(), date.as_bytes())?;
    let secret_service = openssl_hmac_hex("sha256", &hex_to_bytes(&secret_date)?, service.as_bytes())?;
    let secret_signing = openssl_hmac_hex("sha256", &hex_to_bytes(&secret_service)?, b"tc3_request")?;
    let signature = openssl_hmac_hex(
        "sha256",
        &hex_to_bytes(&secret_signing)?,
        string_to_sign.as_bytes(),
    )?;

    let authorization = format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );
    let mut headers = BTreeMap::new();
    headers.insert("Authorization".into(), authorization);
    headers.insert("Content-Type".into(), "application/json; charset=utf-8".into());
    headers.insert("Host".into(), host.into());
    headers.insert("X-TC-Action".into(), action.into());
    headers.insert("X-TC-Timestamp".into(), timestamp.to_string());
    headers.insert("X-TC-Version".into(), "2021-03-23".into());
    headers.insert("X-TC-Language".into(), "en-US".into());

    execute_http_request("POST", &format!("https://{host}"), &headers, body)
}

fn execute_aliyun_api(
    action: &str,
    params: &[(&str, String)],
    access_key_id: &str,
    access_key_secret: &str,
) -> Result<HttpResponse> {
    let timestamp = iso_timestamp(unix_now());
    let nonce = format!("qddns-{}", unix_now());
    let mut pairs = vec![
        ("AccessKeyId".to_string(), access_key_id.to_string()),
        ("Action".to_string(), action.to_string()),
        ("Format".to_string(), "JSON".to_string()),
        ("SignatureMethod".to_string(), "HMAC-SHA1".to_string()),
        ("SignatureNonce".to_string(), nonce),
        ("SignatureVersion".to_string(), "1.0".to_string()),
        ("Timestamp".to_string(), timestamp),
        ("Version".to_string(), "2015-01-09".to_string()),
    ];
    for (key, value) in params {
        pairs.push(((*key).to_string(), value.clone()));
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let canonical = pairs
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let string_to_sign = format!("GET&%2F&{}", percent_encode(&canonical));
    let signature = openssl_hmac_base64("sha1", format!("{access_key_secret}&").as_bytes(), string_to_sign.as_bytes())?;
    let url = format!(
        "https://alidns.aliyuncs.com/?{}&Signature={}",
        canonical,
        percent_encode(&signature)
    );
    execute_http_request("GET", &url, &BTreeMap::new(), "")
}

fn auth_headers(name: &str, value: &str) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    headers.insert(name.into(), value.into());
    headers
}

fn openssl_digest(algo: &str, input: &str) -> Result<String> {
    let output = Command::new("openssl")
        .args(["dgst", &format!("-{algo}"), "-hex"])
        .arg("-r")
        .arg("/dev/stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| Error::new(format!("failed to spawn openssl digest: {err}")))?;
    write_to_child_and_collect(output, input.as_bytes(), "digest")
}

fn openssl_hmac_hex(algo: &str, key: &[u8], data: &[u8]) -> Result<String> {
    let key_hex = bytes_to_hex(key);
    let output = Command::new("openssl")
        .args(["dgst", &format!("-{algo}"), "-mac", "HMAC", "-macopt", &format!("hexkey:{key_hex}"), "-hex"])
        .arg("-r")
        .arg("/dev/stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| Error::new(format!("failed to spawn openssl hmac: {err}")))?;
    write_to_child_and_collect(output, data, "hmac")
}

fn openssl_hmac_base64(algo: &str, key: &[u8], data: &[u8]) -> Result<String> {
    let key_hex = bytes_to_hex(key);
    let mut child = Command::new("openssl")
        .args(["dgst", &format!("-{algo}"), "-mac", "HMAC", "-macopt", &format!("hexkey:{key_hex}"), "-binary"])
        .arg("/dev/stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| Error::new(format!("failed to spawn openssl hmac binary: {err}")))?;
    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(data)?;
    }
    let output = child
        .wait_with_output()
        .map_err(|err| Error::new(format!("failed to collect openssl hmac binary: {err}")))?;
    if !output.status.success() {
        return Err(Error::new("openssl hmac binary command failed"));
    }

    let mut base64 = Command::new("openssl")
        .args(["base64", "-A"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| Error::new(format!("failed to spawn openssl base64: {err}")))?;
    if let Some(stdin) = base64.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(&output.stdout)?;
    }
    let output = base64
        .wait_with_output()
        .map_err(|err| Error::new(format!("failed to collect openssl base64: {err}")))?;
    if !output.status.success() {
        return Err(Error::new("openssl base64 command failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn write_to_child_and_collect(
    mut child: std::process::Child,
    input: &[u8],
    context: &str,
) -> Result<String> {
    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(input)?;
    }
    let output = child
        .wait_with_output()
        .map_err(|err| Error::new(format!("failed to collect openssl {context}: {err}")))?;
    if !output.status.success() {
        return Err(Error::new(format!("openssl {context} command failed")));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value = stdout.split_whitespace().next().unwrap_or("").trim().to_string();
    if value.is_empty() {
        return Err(Error::new(format!("openssl {context} returned empty output")));
    }
    Ok(value)
}

fn percent_encode(input: &str) -> String {
    let mut out = String::new();
    for b in input.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            other => out.push_str(&format!("%{:02X}", other)),
        }
    }
    out
}

fn encode_component(input: &str) -> String {
    percent_encode(input)
}

fn iso_date(timestamp: u64) -> String {
    let iso = iso_timestamp(timestamp);
    iso.split('T').next().unwrap_or("1970-01-01").to_string()
}

fn iso_timestamp(timestamp: u64) -> String {
    let output = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ", "-r", &timestamp.to_string()])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "1970-01-01T00:00:00Z".into(),
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn bytes_to_hex(input: &[u8]) -> String {
    let mut out = String::new();
    for byte in input {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn hex_to_bytes(input: &str) -> Result<Vec<u8>> {
    let trimmed = input.trim();
    if trimmed.len() % 2 != 0 {
        return Err(Error::new("hex string must have even length"));
    }
    let mut out = Vec::with_capacity(trimmed.len() / 2);
    let bytes = trimmed.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        let high = hex_value(bytes[idx])?;
        let low = hex_value(bytes[idx + 1])?;
        out.push((high << 4) | low);
        idx += 2;
    }
    Ok(out)
}

fn hex_value(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(Error::new("invalid hex digit")),
    }
}

fn find_ip_in_text(text: &str) -> Option<String> {
    for token in text.split(|c: char| c.is_whitespace() || [',', ';', '[', ']', '(', ')', '"'].contains(&c)) {
        let trimmed = token.trim_matches(|c: char| c == ':' || c == '"' || c == '\'');
        if trimmed.parse::<std::net::IpAddr>().is_ok() {
            return Some(trimmed.to_string());
        }
    }
    None
}
