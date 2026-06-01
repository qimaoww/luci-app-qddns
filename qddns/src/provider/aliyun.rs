use serde_json::Value;

use crate::config::{ProviderConfig, RuleConfig};
use crate::error::{Error, Result};
use crate::http::{HttpClient, RetryPolicy};

use super::{
    execute_aliyun_api, fqdn, lookup_retry_policy, parse_provider_response_json,
    redact_provider_error, RemoteRecord, SyncOutcome,
};

pub(crate) fn fetch_record(
    http: &HttpClient,
    provider: &ProviderConfig,
    rule: &RuleConfig,
) -> Result<RemoteRecord> {
    let (access_key_id, access_key_secret) = provider.aliyun_credentials();
    let access_key_id = access_key_id.ok_or_else(|| {
        Error::new(format!(
            "provider '{}' missing access_key_id",
            provider.name
        ))
    })?;
    let access_key_secret = access_key_secret.ok_or_else(|| {
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
        http,
        lookup_retry_policy(rule),
        "DescribeSubDomainRecords",
        &[
            ("SubDomain", fqdn(rule)),
            ("Type", rule.record_type.as_str().to_string()),
        ],
        access_key_id,
        access_key_secret,
    )?;
    let json_value = parse_provider_response_json(&response.body)?;
    let (address, record_id) =
        extract_record(&json_value, &rr, &[access_key_id, access_key_secret])?;

    Ok(RemoteRecord {
        address,
        record_id,
        detail: format!("aliyun subdomain={}", fqdn(rule)),
    })
}

pub(crate) fn update_record(
    http: &HttpClient,
    provider: &ProviderConfig,
    rule: &RuleConfig,
    remote: &RemoteRecord,
    target_ip: &str,
) -> Result<SyncOutcome> {
    let (access_key_id, access_key_secret) = provider.aliyun_credentials();
    let access_key_id = access_key_id.ok_or_else(|| {
        Error::new(format!(
            "provider '{}' missing access_key_id",
            provider.name
        ))
    })?;
    let access_key_secret = access_key_secret.ok_or_else(|| {
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
        ("Type", rule.record_type.as_str().to_string()),
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

    let response = execute_aliyun_api(
        http,
        RetryPolicy::none(),
        action,
        &params,
        access_key_id,
        access_key_secret,
    )?;
    let json_value = parse_provider_response_json(&response.body)?;
    ensure_success(&json_value, &[access_key_id, access_key_secret])?;

    Ok(SyncOutcome {
        changed: remote.address.as_deref() != Some(target_ip),
        remote_before: remote.address.clone(),
        remote_after: target_ip.into(),
        detail: format!("aliyun {} {}", action, fqdn(rule)),
    })
}

fn ensure_success(value: &Value, secrets: &[&str]) -> Result<()> {
    let obj = value
        .as_object()
        .ok_or_else(|| Error::new("aliyun response must be object"))?;
    if let Some(code) = obj.get("Code").and_then(Value::as_str) {
        let message = obj
            .get("Message")
            .and_then(Value::as_str)
            .unwrap_or("unknown aliyun error");
        return Err(Error::new(redact_provider_error(
            &format!("{code}: {message}"),
            secrets,
        )));
    }
    Ok(())
}

fn extract_record(
    value: &Value,
    rr: &str,
    secrets: &[&str],
) -> Result<(Option<String>, Option<String>)> {
    ensure_success(value, secrets)?;
    let records = value
        .as_object()
        .and_then(|obj| obj.get("DomainRecords"))
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("Record"))
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("aliyun response missing DomainRecords.Record"))?;
    for item in records {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let current_rr = obj.get("RR").and_then(Value::as_str).unwrap_or("");
        if current_rr != rr {
            continue;
        }
        let address = obj
            .get("Value")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let record_id = obj
            .get("RecordId")
            .and_then(Value::as_str)
            .map(ToString::to_string);
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
            provider: "aliyun".into(),
            source: "wan4".into(),
            probe_interface: None,
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
    fn aliyun_update_params_use_rule_shape() {
        let rule = rule();
        let rr = if rule.record_name == "@" {
            "".to_string()
        } else {
            rule.record_name.clone()
        };
        let params = [
            ("RR", rr),
            ("Type", rule.record_type.as_str().to_string()),
            ("Value", "198.51.100.9".to_string()),
            ("TTL", rule.ttl.to_string()),
            ("DomainName", rule.zone.clone()),
        ];

        assert!(params.contains(&("RR", "home".to_string())));
        assert!(params.contains(&("Type", "A".to_string())));
        assert!(params.contains(&("DomainName", "example.com".to_string())));
    }

    #[test]
    fn aliyun_error_parser_redacts_provider_secret() {
        let secret = "test-secret-123";
        let err = ensure_success(
            &json!({"Code": "InvalidAccessKey", "Message": format!("bad secret {secret}")}),
            &[secret],
        )
        .expect_err("aliyun error should fail");

        let text = err.to_string();
        assert!(!text.contains(secret), "secret leaked: {text}");
        assert!(
            text.contains("InvalidAccessKey"),
            "status context lost: {text}"
        );
    }

    #[test]
    fn aliyun_record_parser_matches_rr() {
        let (address, record_id) = extract_record(
            &json!({"DomainRecords": {"Record": [
                {"RR": "www", "Value": "198.51.100.8", "RecordId": "old"},
                {"RR": "home", "Value": "198.51.100.9", "RecordId": "record-1"}
            ]}}),
            "home",
            &[],
        )
        .unwrap();

        assert_eq!(address.as_deref(), Some("198.51.100.9"));
        assert_eq!(record_id.as_deref(), Some("record-1"));
    }
}
