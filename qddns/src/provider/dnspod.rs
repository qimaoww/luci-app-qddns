use serde_json::{json, Value};

use crate::config::{ProviderConfig, RuleConfig};
use crate::error::{Error, Result};
use crate::http::{HttpClient, RetryPolicy};

use super::{
    execute_tencent_json_api, fqdn, lookup_retry_policy, parse_provider_response_json,
    redact_provider_error, RemoteRecord, SyncOutcome,
};

pub(crate) fn fetch_record(
    http: &HttpClient,
    provider: &ProviderConfig,
    rule: &RuleConfig,
) -> Result<RemoteRecord> {
    let (secret_id, secret_key) = provider.dnspod_credentials();
    let secret_id = secret_id
        .ok_or_else(|| Error::new(format!("provider '{}' missing secret_id", provider.name)))?;
    let secret_key = secret_key
        .ok_or_else(|| Error::new(format!("provider '{}' missing secret_key", provider.name)))?;
    let body = fetch_body(rule);
    let response = execute_tencent_json_api(
        http,
        lookup_retry_policy(rule),
        "dnspod.tencentcloudapi.com",
        "DescribeRecordList",
        &body,
        secret_id,
        secret_key,
    )?;
    let json_value = parse_provider_response_json(&response.body)?;
    ensure_tencent_success(&json_value, &[secret_id, secret_key])?;
    let (address, record_id) = extract_record(&json_value, rule)?;

    Ok(RemoteRecord {
        address,
        record_id,
        detail: format!("dnspod domain={}", rule.zone),
    })
}

pub(crate) fn update_record(
    http: &HttpClient,
    provider: &ProviderConfig,
    rule: &RuleConfig,
    remote: &RemoteRecord,
    target_ip: &str,
) -> Result<SyncOutcome> {
    let (secret_id, secret_key) = provider.dnspod_credentials();
    let secret_id = secret_id
        .ok_or_else(|| Error::new(format!("provider '{}' missing secret_id", provider.name)))?;
    let secret_key = secret_key
        .ok_or_else(|| Error::new(format!("provider '{}' missing secret_key", provider.name)))?;

    let action = if remote.record_id.is_some() {
        "ModifyRecord"
    } else {
        "CreateRecord"
    };
    let body = if let Some(record_id) = remote.record_id.as_deref() {
        json!({
            "Domain": rule.zone,
            "Subdomain": rule.record_name,
            "RecordType": rule.record_type.as_str(),
            "RecordLine": "默认",
            "RecordId": record_id.parse::<u64>().map(Value::from).unwrap_or_else(|_| Value::from(record_id)),
            "Value": target_ip,
            "TTL": rule.ttl,
        })
        .to_string()
    } else {
        json!({
            "Domain": rule.zone,
            "Subdomain": rule.record_name,
            "RecordType": rule.record_type.as_str(),
            "RecordLine": "默认",
            "Value": target_ip,
            "TTL": rule.ttl,
        })
        .to_string()
    };
    let response = execute_tencent_json_api(
        http,
        RetryPolicy::none(),
        "dnspod.tencentcloudapi.com",
        action,
        &body,
        secret_id,
        secret_key,
    )?;
    let json_value = parse_provider_response_json(&response.body)?;
    ensure_tencent_success(&json_value, &[secret_id, secret_key])?;

    Ok(SyncOutcome {
        changed: remote.address.as_deref() != Some(target_ip),
        remote_before: remote.address.clone(),
        remote_after: target_ip.into(),
        detail: format!("dnspod {} {}", action, fqdn(rule)),
    })
}

fn ensure_tencent_success(value: &Value, secrets: &[&str]) -> Result<()> {
    let response = value
        .as_object()
        .and_then(|obj| obj.get("Response"))
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new("tencent response missing Response object"))?;
    if let Some(err_obj) = response.get("Error").and_then(Value::as_object) {
        let code = err_obj
            .get("Code")
            .and_then(Value::as_str)
            .unwrap_or("TencentError");
        let message = err_obj
            .get("Message")
            .and_then(Value::as_str)
            .unwrap_or("unknown tencent error");
        return Err(Error::new(redact_provider_error(
            &format!("{code}: {message}"),
            secrets,
        )));
    }
    Ok(())
}

fn fetch_body(rule: &RuleConfig) -> String {
    json!({
        "Domain": rule.zone,
        "Subdomain": rule.record_name,
        "RecordType": rule.record_type.as_str(),
    })
    .to_string()
}

fn extract_record(value: &Value, rule: &RuleConfig) -> Result<(Option<String>, Option<String>)> {
    let records = value
        .as_object()
        .and_then(|obj| obj.get("Response"))
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("RecordList"))
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("dnspod response missing RecordList"))?;

    let target_name = rule.record_name.as_str();
    for item in records {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let record_type = obj.get("Type").and_then(Value::as_str).unwrap_or("");
        let subdomain = obj.get("Name").and_then(Value::as_str).unwrap_or("");
        if record_type != rule.record_type.as_str() || subdomain != target_name {
            continue;
        }
        let address = obj
            .get("Value")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let record_id = match obj.get("RecordId") {
            Some(Value::String(text)) => Some(text.clone()),
            Some(Value::Number(number)) => Some(number.to_string()),
            _ => None,
        };
        return Ok((address, record_id));
    }

    Ok((None, None))
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
            provider: "dnspod".into(),
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
    fn dnspod_fetch_body_uses_rule_shape() {
        let value: Value = serde_json::from_str(&fetch_body(&rule())).unwrap();

        assert_eq!(value["Domain"], "example.com");
        assert_eq!(value["Subdomain"], "home");
        assert_eq!(value["RecordType"], "A");
    }

    #[test]
    fn dnspod_error_parser_redacts_provider_secret() {
        let secret = "test-secret-123";
        let err = ensure_tencent_success(
            &json!({"Response": {"Error": {"Code": "AuthFailure", "Message": format!("bad key {secret}")}}}),
            &[secret],
        )
        .expect_err("dnspod error should fail");

        let text = err.to_string();
        assert!(!text.contains(secret), "secret leaked: {text}");
        assert!(text.contains("AuthFailure"), "status context lost: {text}");
    }

    #[test]
    fn dnspod_record_parser_matches_type_and_name() {
        let (address, record_id) = extract_record(
            &json!({"Response": {"RecordList": [
                {"Type": "AAAA", "Name": "home", "Value": "2001:db8::1", "RecordId": 8},
                {"Type": "A", "Name": "home", "Value": "198.51.100.9", "RecordId": 9}
            ]}}),
            &rule(),
        )
        .unwrap();

        assert_eq!(address.as_deref(), Some("198.51.100.9"));
        assert_eq!(record_id.as_deref(), Some("9"));
    }
}
