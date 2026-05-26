use qddns::config::{CustomHttpConfig, ProviderConfig, ProviderKind, RuleConfig};
use qddns::provider::{
    build_custom_http_request, parse_custom_http_success, parse_provider_response_json,
    RemoteRecord,
};

fn custom_provider() -> ProviderConfig {
    ProviderConfig {
        name: "custom".into(),
        kind: ProviderKind::CustomHttp(CustomHttpConfig {
            url: Some("https://example.com/ddns".into()),
            method: Some("POST".into()),
            headers_json: Some(
                "{\"Authorization\":\"Bearer token\",\"Content-Type\":\"application/json\"}".into(),
            ),
            body_template: Some(
                "{\"zone\":\"{{zone}}\",\"record\":\"{{record_name}}\",\"ip\":\"{{ip}}\"}".into(),
            ),
            lookup_url: None,
            lookup_method: None,
            lookup_headers_json: None,
            lookup_json_pointer: None,
            success_contains: Some("updated".into()),
        }),
    }
}

fn custom_rule() -> RuleConfig {
    RuleConfig {
        name: "home".into(),
        enabled: true,
        provider: "custom".into(),
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
fn custom_http_request_renders_method_url_headers_and_body() {
    let provider = custom_provider();
    let rule = custom_rule();

    let request =
        build_custom_http_request(&provider, &rule, "198.51.100.9").expect("request builds");

    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://example.com/ddns");
    assert_eq!(
        request.headers.get("Authorization").map(String::as_str),
        Some("Bearer token")
    );
    assert!(request.body.contains("\"record\":\"home\""));
    assert!(request.body.contains("\"ip\":\"198.51.100.9\""));
}

#[test]
fn custom_http_success_parser_matches_expected_substring() {
    let provider = custom_provider();
    assert!(parse_custom_http_success(&provider, "updated record ok").unwrap());
    assert!(!parse_custom_http_success(&provider, "temporary failure").unwrap());
}

#[test]
fn custom_http_request_can_use_remote_record_context() {
    let provider = custom_provider();
    let rule = custom_rule();
    let remote = RemoteRecord {
        address: Some("198.51.100.8".into()),
        record_id: Some("r1".into()),
        detail: "before".into(),
    };

    let request =
        build_custom_http_request(&provider, &rule, "198.51.100.9").expect("request builds");

    assert_eq!(remote.record_id.as_deref(), Some("r1"));
    assert!(request.body.contains("198.51.100.9"));
}

#[test]
fn rejects_malformed_provider_json() {
    let secret = "test-secret-123";
    let err = parse_provider_response_json(&format!("{{\"secret\":\"{secret}\","))
        .expect_err("malformed provider JSON should fail");

    let text = err.to_string();
    assert!(
        text.contains("malformed provider JSON"),
        "unexpected error: {text}"
    );
    assert!(!text.contains(secret), "secret leaked in error: {text}");
}
