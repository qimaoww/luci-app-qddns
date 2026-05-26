use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use qddns::http::{HttpClient, HttpRequest, RetryPolicy};

mod support;
use support::{MockHttpServer, MockResponse};

fn client(timeout_ms: u64) -> HttpClient {
    HttpClient::new(Duration::from_millis(timeout_ms))
}

fn request(url: String) -> HttpRequest {
    HttpRequest {
        method: "GET".into(),
        url,
        headers: BTreeMap::new(),
        body: String::new(),
    }
}

#[test]
fn http_client_success_with_timeout() {
    let server = match MockHttpServer::try_single_response(200, "ok") {
        Ok(server) => server,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "local TCP bind unavailable in this sandbox; skipping HTTP client success test"
            );
            return;
        }
        Err(err) => panic!("bind mock server: {err}"),
    };

    let response = client(300)
        .execute(&request(server.url("/ok")), RetryPolicy::none())
        .expect("HTTP request succeeds");

    assert_eq!(response.status, 200);
    assert_eq!(response.body, "ok");
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/ok");
}

#[test]
fn http_timeout_is_enforced() {
    let server = match MockHttpServer::try_responses(vec![
        MockResponse::new(200, "late").with_delay(Duration::from_millis(500))
    ]) {
        Ok(server) => server,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("local TCP bind unavailable in this sandbox; skipping HTTP timeout test");
            return;
        }
        Err(err) => panic!("bind mock server: {err}"),
    };

    let started = Instant::now();
    let err = client(100)
        .execute(&request(server.url("/slow")), RetryPolicy::none())
        .expect_err("slow response must time out");

    assert!(
        started.elapsed() < Duration::from_secs(5),
        "timeout took too long: {:?}",
        started.elapsed()
    );
    assert!(
        err.to_string().contains("timed out"),
        "unexpected error: {err}"
    );
}

#[test]
fn http_retry_policy() {
    let lookup = match MockHttpServer::try_responses(vec![
        MockResponse::new(500, "first failure"),
        MockResponse::new(200, "198.51.100.20"),
    ]) {
        Ok(server) => server,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("local TCP bind unavailable in this sandbox; skipping HTTP retry test");
            return;
        }
        Err(err) => panic!("bind mock server: {err}"),
    };

    let response = client(300)
        .execute(&request(lookup.url("/lookup")), RetryPolicy::idempotent(2))
        .expect("lookup retry succeeds");
    assert_eq!(response.status, 200);
    assert_eq!(response.body, "198.51.100.20");
    assert_eq!(lookup.requests().len(), 2);

    let update = MockHttpServer::try_responses(vec![
        MockResponse::new(500, "update failed"),
        MockResponse::new(200, "should not be used"),
    ])
    .expect("bind update mock server");
    let err = client(300)
        .execute(&request(update.url("/update")), RetryPolicy::none())
        .expect_err("non-idempotent update should not retry");
    assert!(
        err.to_string().contains("HTTP 500"),
        "unexpected error: {err}"
    );
    assert_eq!(update.requests().len(), 1);
}

#[test]
fn http_errors_redact_request_secrets() {
    let secret = "test-hmac-key-123";
    let server = match MockHttpServer::try_single_response(500, format!("upstream echoed {secret}"))
    {
        Ok(server) => server,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("local TCP bind unavailable in this sandbox; skipping HTTP redaction test");
            return;
        }
        Err(err) => panic!("bind mock server: {err}"),
    };

    let mut headers = BTreeMap::new();
    headers.insert("Authorization".into(), format!("Bearer {secret}"));
    let err = client(300)
        .execute(
            &HttpRequest {
                method: "POST".into(),
                url: server.url("/secret"),
                headers,
                body: format!("secret={secret}"),
            },
            RetryPolicy::none(),
        )
        .expect_err("500 response should fail");

    let text = err.to_string();
    assert!(!text.contains(secret), "secret leaked in error: {text}");
    assert!(
        text.contains("[redacted]"),
        "error should preserve redaction marker: {text}"
    );
}

#[test]
fn provider_errors_are_redacted() {
    let secret = "test-secret-123";
    let server = match MockHttpServer::try_single_response(500, format!("bad token {secret}")) {
        Ok(server) => server,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "local TCP bind unavailable in this sandbox; skipping provider redaction test"
            );
            return;
        }
        Err(err) => panic!("bind mock server: {err}"),
    };

    let mut headers = BTreeMap::new();
    headers.insert("Authorization".into(), format!("Bearer {secret}"));
    let err = client(300)
        .execute(
            &HttpRequest {
                method: "GET".into(),
                url: server.url("/secret"),
                headers,
                body: String::new(),
            },
            RetryPolicy::none(),
        )
        .expect_err("provider error should fail");

    let text = err.to_string();
    assert!(
        !text.contains(secret),
        "provider secret leaked in error: {text}"
    );
    assert!(
        text.contains("HTTP 500"),
        "error should keep status context: {text}"
    );
}
