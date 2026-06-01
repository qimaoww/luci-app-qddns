use std::time::Duration;

use qddns::config::{AddressFamily, RecordType, RuleConfig, SourceConfig, SourceKind};
use qddns::http::HttpClient;
use qddns::source::{resolve_source_with_http, resolve_source_with_rule_and_http};

mod support;
use support::{MockHttpServer, MockResponse};

fn public_probe_with_family(url: String, family: Option<AddressFamily>) -> SourceConfig {
    SourceConfig {
        name: "probe_ip".into(),
        kind: SourceKind::PublicProbe {
            family,
            probe_url: Some(url),
        },
    }
}

fn public_probe(url: String) -> SourceConfig {
    public_probe_with_family(url, Some(AddressFamily::Ipv4))
}

fn rule_with_probe_interface(interface: &str) -> RuleConfig {
    RuleConfig {
        name: "home".into(),
        enabled: true,
        provider: "cf".into(),
        source: "probe_ip".into(),
        probe_interface: Some(interface.into()),
        record_type: RecordType::A,
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
fn public_probe_http_uses_configured_timeout() {
    let server = match MockHttpServer::try_responses(vec![
        MockResponse::new(200, "203.0.113.45").with_delay(Duration::from_millis(500))
    ]) {
        Ok(server) => server,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "local TCP bind unavailable in this sandbox; skipping public probe timeout test"
            );
            return;
        }
        Err(err) => panic!("bind mock server: {err}"),
    };

    let source = public_probe(server.url("/probe"));
    let http = HttpClient::new(Duration::from_millis(100));
    let err = resolve_source_with_http(&source, &http).expect_err("slow probe must time out");

    assert!(
        err.to_string().contains("timed out"),
        "unexpected error: {err}"
    );
}

#[test]
fn public_probe_http_extracts_ip_from_response() {
    let server =
        match MockHttpServer::try_single_response(200, "Current IP Address: 203.0.113.46\n") {
            Ok(server) => server,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!(
                    "local TCP bind unavailable in this sandbox; skipping public probe HTTP test"
                );
                return;
            }
            Err(err) => panic!("bind mock server: {err}"),
        };

    let source = public_probe(server.url("/probe"));
    let http = HttpClient::new(Duration::from_millis(300));
    let resolved = resolve_source_with_http(&source, &http).expect("public probe resolves");

    assert_eq!(resolved.address.to_string(), "203.0.113.46");
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn public_probe_http_accepts_bound_interface_when_configured() {
    if !cfg!(target_os = "linux") {
        eprintln!("SO_BINDTODEVICE is Linux-only; skipping bound public probe request test");
        return;
    }

    let server = match MockHttpServer::try_single_response(200, "203.0.113.47") {
        Ok(server) => server,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "local TCP bind unavailable in this sandbox; skipping bound public probe test"
            );
            return;
        }
        Err(err) => panic!("bind mock server: {err}"),
    };

    let source = public_probe(server.url("/probe"));
    let rule = rule_with_probe_interface("lo");
    let mut rule = rule;
    rule.record_type = RecordType::Aaaa;
    let http = HttpClient::new(Duration::from_millis(300));
    let resolved = resolve_source_with_rule_and_http(&source, Some(&rule), &http)
        .expect("bound public probe resolves");

    assert_eq!(resolved.address.to_string(), "203.0.113.47");
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn public_probe_uses_rule_probe_interface_for_a_records() {
    let server = match MockHttpServer::try_single_response(200, "203.0.113.48") {
        Ok(server) => server,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "local TCP bind unavailable in this sandbox; skipping A public probe binding test"
            );
            return;
        }
        Err(err) => panic!("bind mock server: {err}"),
    };

    let source = public_probe(server.url("/probe"));
    let rule = rule_with_probe_interface("lo");
    let mut rule = rule;
    rule.record_type = RecordType::A;
    let http = HttpClient::new(Duration::from_millis(300));
    let resolved = resolve_source_with_rule_and_http(&source, Some(&rule), &http)
        .expect("A public probe should use the configured probe interface");

    assert_eq!(resolved.address.to_string(), "203.0.113.48");
    assert_eq!(server.requests().len(), 1);
}
