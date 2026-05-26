use std::collections::BTreeMap;

use crate::config::{ProviderConfig, ProviderKind, RuleConfig};
use crate::error::{Error, Result};
use crate::http::{HttpClient, HttpRequest, HttpResponse, RetryPolicy};
use base64::Engine;
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

mod aliyun;
mod cloudflare;
mod custom_http;
mod dnspod;

pub use custom_http::{build_custom_http_request, parse_custom_http_success, CustomHttpRequest};

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

#[derive(Debug, Clone)]
pub struct ShellProviderAdapter {
    http: HttpClient,
}

impl ShellProviderAdapter {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            http: HttpClient::from_timeout_secs(timeout_secs),
        }
    }
}

impl Default for ShellProviderAdapter {
    fn default() -> Self {
        Self::new(15)
    }
}

impl ProviderAdapter for ShellProviderAdapter {
    fn fetch_record(&self, provider: &ProviderConfig, rule: &RuleConfig) -> Result<RemoteRecord> {
        match &provider.kind {
            ProviderKind::CustomHttp(_) => custom_http::fetch_record(&self.http, provider, rule),
            ProviderKind::Cloudflare { .. } => cloudflare::fetch_record(&self.http, provider, rule),
            ProviderKind::DnsPod { .. } => dnspod::fetch_record(&self.http, provider, rule),
            ProviderKind::Aliyun { .. } => aliyun::fetch_record(&self.http, provider, rule),
        }
    }

    fn update_record(
        &self,
        provider: &ProviderConfig,
        rule: &RuleConfig,
        remote: &RemoteRecord,
        target_ip: &str,
    ) -> Result<SyncOutcome> {
        match &provider.kind {
            ProviderKind::CustomHttp(_) => {
                custom_http::update_record(&self.http, provider, rule, remote, target_ip)
            }
            ProviderKind::Cloudflare { .. } => {
                cloudflare::update_record(&self.http, provider, rule, remote, target_ip)
            }
            ProviderKind::DnsPod { .. } => {
                dnspod::update_record(&self.http, provider, rule, remote, target_ip)
            }
            ProviderKind::Aliyun { .. } => {
                aliyun::update_record(&self.http, provider, rule, remote, target_ip)
            }
        }
    }
}

pub fn parse_provider_response_json(body: &str) -> Result<Value> {
    serde_json::from_str(body).map_err(|_| Error::new("malformed provider JSON"))
}

pub(crate) fn execute_http_request(
    http: &HttpClient,
    retry: RetryPolicy,
    method: &str,
    url: &str,
    headers: &BTreeMap<String, String>,
    body: &str,
) -> Result<HttpResponse> {
    http.execute(
        &HttpRequest {
            method: method.to_string(),
            url: url.to_string(),
            headers: headers.clone(),
            body: body.to_string(),
        },
        retry,
    )
}

pub(crate) fn lookup_retry_policy(rule: &RuleConfig) -> RetryPolicy {
    RetryPolicy::idempotent(rule.retry_count as usize + 1)
}

pub(crate) fn render_template(
    template: &str,
    rule: &RuleConfig,
    ip: &str,
    remote: Option<&RemoteRecord>,
) -> String {
    let remote_ip = remote
        .and_then(|value| value.address.as_deref())
        .unwrap_or("");
    let record_id = remote
        .and_then(|value| value.record_id.as_deref())
        .unwrap_or("");
    let fqdn = fqdn(rule);
    template
        .replace("{{ip}}", ip)
        .replace("{{zone}}", &rule.zone)
        .replace("{{record_name}}", &rule.record_name)
        .replace("{{fqdn}}", &fqdn)
        .replace("{{record_type}}", rule.record_type.as_str())
        .replace("{{ttl}}", &rule.ttl.to_string())
        .replace("{{proxied}}", if rule.proxied { "true" } else { "false" })
        .replace("{{remote_ip}}", remote_ip)
        .replace("{{record_id}}", record_id)
}

pub(crate) fn fqdn(rule: &RuleConfig) -> String {
    if rule.record_name == "@" {
        rule.zone.clone()
    } else {
        format!("{}.{}", rule.record_name, rule.zone)
    }
}

pub(crate) fn auth_headers(name: &str, value: &str) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    headers.insert(name.into(), value.into());
    headers
}

pub(crate) fn execute_tencent_json_api(
    http: &HttpClient,
    retry: RetryPolicy,
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
    let canonical_headers = format!("content-type:application/json; charset=utf-8\nhost:{host}\n");
    let signed_headers = "content-type;host";
    let canonical_request =
        format!("POST\n/\n\n{canonical_headers}\n{signed_headers}\n{hashed_payload}");
    let credential_scope = format!("{date}/{service}/tc3_request");
    let string_to_sign = format!(
        "TC3-HMAC-SHA256\n{timestamp}\n{credential_scope}\n{}",
        openssl_digest("sha256", &canonical_request)?
    );

    let secret_date = openssl_hmac_hex(
        "sha256",
        format!("TC3{secret_key}").as_bytes(),
        date.as_bytes(),
    )?;
    let secret_service =
        openssl_hmac_hex("sha256", &hex_to_bytes(&secret_date)?, service.as_bytes())?;
    let secret_signing =
        openssl_hmac_hex("sha256", &hex_to_bytes(&secret_service)?, b"tc3_request")?;
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
    headers.insert(
        "Content-Type".into(),
        "application/json; charset=utf-8".into(),
    );
    headers.insert("Host".into(), host.into());
    headers.insert("X-TC-Action".into(), action.into());
    headers.insert("X-TC-Timestamp".into(), timestamp.to_string());
    headers.insert("X-TC-Version".into(), "2021-03-23".into());
    headers.insert("X-TC-Language".into(), "en-US".into());

    execute_http_request(
        http,
        retry,
        "POST",
        &format!("https://{host}"),
        &headers,
        body,
    )
}

pub(crate) fn execute_aliyun_api(
    http: &HttpClient,
    retry: RetryPolicy,
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
    let signature = openssl_hmac_base64(
        "sha1",
        format!("{access_key_secret}&").as_bytes(),
        string_to_sign.as_bytes(),
    )?;
    let url = format!(
        "https://alidns.aliyuncs.com/?{}&Signature={}",
        canonical,
        percent_encode(&signature)
    );
    execute_http_request(http, retry, "GET", &url, &BTreeMap::new(), "")
}

pub(crate) fn percent_encode(input: &str) -> String {
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

pub(crate) fn encode_component(input: &str) -> String {
    percent_encode(input)
}

fn openssl_digest(algo: &str, input: &str) -> Result<String> {
    match algo {
        "sha256" => {
            let digest = Sha256::digest(input.as_bytes());
            Ok(hex::encode(digest))
        }
        other => Err(Error::new(format!("unsupported digest algorithm: {other}"))),
    }
}

fn openssl_hmac_hex(algo: &str, key: &[u8], data: &[u8]) -> Result<String> {
    match algo {
        "sha256" => {
            let mut mac = Hmac::<Sha256>::new_from_slice(key)
                .map_err(|err| Error::new(format!("invalid hmac key: {err}")))?;
            mac.update(data);
            Ok(hex::encode(mac.finalize().into_bytes()))
        }
        "sha1" => {
            let mut mac = Hmac::<Sha1>::new_from_slice(key)
                .map_err(|err| Error::new(format!("invalid hmac key: {err}")))?;
            mac.update(data);
            Ok(hex::encode(mac.finalize().into_bytes()))
        }
        other => Err(Error::new(format!("unsupported hmac algorithm: {other}"))),
    }
}

fn openssl_hmac_base64(algo: &str, key: &[u8], data: &[u8]) -> Result<String> {
    let digest = match algo {
        "sha1" => {
            let mut mac = Hmac::<Sha1>::new_from_slice(key)
                .map_err(|err| Error::new(format!("invalid hmac key: {err}")))?;
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }
        "sha256" => {
            let mut mac = Hmac::<Sha256>::new_from_slice(key)
                .map_err(|err| Error::new(format!("invalid hmac key: {err}")))?;
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }
        other => return Err(Error::new(format!("unsupported hmac algorithm: {other}"))),
    };
    Ok(base64::engine::general_purpose::STANDARD.encode(digest))
}

fn iso_date(timestamp: u64) -> String {
    let iso = iso_timestamp(timestamp);
    iso.split('T').next().unwrap_or("1970-01-01").to_string()
}

fn iso_timestamp(timestamp: u64) -> String {
    match OffsetDateTime::from_unix_timestamp(timestamp as i64) {
        Ok(value) => value
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into()),
        Err(_) => "1970-01-01T00:00:00Z".into(),
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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

pub(crate) fn find_ip_in_text(text: &str) -> Option<String> {
    for token in
        text.split(|c: char| c.is_whitespace() || [',', ';', '[', ']', '(', ')', '"'].contains(&c))
    {
        let trimmed = token.trim_matches(|c: char| c == ':' || c == '"' || c == '\'');
        if trimmed.parse::<std::net::IpAddr>().is_ok() {
            return Some(trimmed.to_string());
        }
    }
    None
}

pub(crate) fn redact_provider_error(text: &str, secrets: &[&str]) -> String {
    let mut redacted = text.to_string();
    for secret in secrets {
        if secret.len() >= 4 {
            redacted = redacted.replace(secret, "[redacted]");
        }
    }
    redacted
}

#[cfg(test)]
mod tests;
