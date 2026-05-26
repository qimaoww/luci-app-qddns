use std::time::Duration;

use qddns::config::{AddressFamily, SourceConfig, SourceKind};
use qddns::http::HttpClient;
use qddns::source::resolve_source_with_http;

mod support;
use support::{MockHttpServer, MockResponse};

fn public_probe(url: String) -> SourceConfig {
    SourceConfig {
        name: "probe_ip".into(),
        kind: SourceKind::PublicProbe {
            family: Some(AddressFamily::Ipv4),
            probe_url: Some(url),
        },
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
