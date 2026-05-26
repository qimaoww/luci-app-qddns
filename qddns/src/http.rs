use std::collections::BTreeMap;
use std::time::Duration;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_attempts: usize,
}

impl RetryPolicy {
    pub fn none() -> Self {
        Self { max_attempts: 1 }
    }

    pub fn idempotent(max_attempts: usize) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpClient {
    timeout: Duration,
}

impl HttpClient {
    pub fn new(timeout: Duration) -> Self {
        let timeout = if timeout.is_zero() {
            Duration::from_secs(1)
        } else {
            timeout
        };
        Self { timeout }
    }

    pub fn from_timeout_secs(timeout_secs: u64) -> Self {
        Self::new(Duration::from_secs(timeout_secs.clamp(1, 30)))
    }

    pub fn execute(&self, request: &HttpRequest, retry: RetryPolicy) -> Result<HttpResponse> {
        let attempts = retry.max_attempts.max(1);
        let mut last_error = None;

        for attempt in 1..=attempts {
            match self.execute_once(request) {
                Ok(response) => return Ok(response),
                Err(err) => {
                    let retryable = is_retryable_error(&err);
                    if attempt == attempts || !retryable {
                        return Err(err);
                    }
                    last_error = Some(err);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| Error::new("HTTP request failed")))
    }

    fn execute_once(&self, request: &HttpRequest) -> Result<HttpResponse> {
        if request.url.starts_with("file://") {
            return Err(Error::new("unsupported provider url scheme: file"));
        }

        let agent = ureq::AgentBuilder::new().timeout(self.timeout).build();
        let mut call = agent
            .request(&request.method, &request.url)
            .timeout(self.timeout);
        for (name, value) in &request.headers {
            call = call.set(name, value);
        }

        let result = if request.body.is_empty()
            || request.method.eq_ignore_ascii_case("GET")
            || request.method.eq_ignore_ascii_case("HEAD")
        {
            call.call()
        } else {
            call.send_string(&request.body)
        };

        match result {
            Ok(response) => response_to_http(response),
            Err(ureq::Error::Status(status, response)) => {
                let body = response.into_string().unwrap_or_default();
                let body = sanitize_http_error_text(&body, request);
                Err(Error::new(format!(
                    "HTTP {status} from provider endpoint: {}",
                    trim_error_body(&body)
                )))
            }
            Err(ureq::Error::Transport(err)) => {
                let message = sanitize_http_error_text(&err.to_string(), request);
                if message.to_ascii_lowercase().contains("timed out") {
                    Err(Error::new(format!("HTTP request timed out: {message}")))
                } else {
                    Err(Error::new(format!("HTTP request failed: {message}")))
                }
            }
        }
    }
}

fn response_to_http(response: ureq::Response) -> Result<HttpResponse> {
    let status = response.status();
    let body = response
        .into_string()
        .map_err(|err| Error::new(format!("failed to read HTTP response body: {err}")))?;
    Ok(HttpResponse { status, body })
}

fn is_retryable_error(err: &Error) -> bool {
    let text = err.to_string();
    text.contains("HTTP 5") || text.contains("timed out") || text.contains("connection")
}

fn sanitize_http_error_text(text: &str, request: &HttpRequest) -> String {
    let mut candidates = Vec::new();
    for (name, value) in &request.headers {
        if is_sensitive_name(name) {
            push_secret_candidates(&mut candidates, value);
        }
    }
    if contains_sensitive_marker(&request.body) {
        push_secret_candidates(&mut candidates, &request.body);
    }
    if contains_sensitive_marker(&request.url) {
        push_secret_candidates(&mut candidates, &request.url);
    }

    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.len()));
    candidates.dedup();

    let mut sanitized = text.to_string();
    for candidate in candidates {
        sanitized = sanitized.replace(&candidate, "[redacted]");
    }
    sanitized
}

fn is_sensitive_name(name: &str) -> bool {
    contains_sensitive_marker(name)
}

fn contains_sensitive_marker(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    [
        "authorization",
        "cookie",
        "password",
        "secret",
        "token",
        "key",
    ]
    .iter()
    .any(|marker| lowered.contains(marker))
}

fn push_secret_candidates(candidates: &mut Vec<String>, value: &str) {
    let trimmed = value.trim();
    if trimmed.len() >= 4 {
        candidates.push(trimmed.to_string());
    }
    for token in trimmed.split(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '&' | '=' | ',' | ':' | '"' | '\'' | '{' | '}' | '[' | ']'
            )
    }) {
        if token.len() >= 4 && token != "Bearer" && token != "Basic" {
            candidates.push(token.to_string());
        }
    }
}

fn trim_error_body(body: &str) -> String {
    const LIMIT: usize = 200;
    let trimmed = body.trim();
    if trimmed.len() <= LIMIT {
        trimmed.to_string()
    } else {
        let end = trimmed
            .char_indices()
            .map(|(idx, _)| idx)
            .take_while(|idx| *idx <= LIMIT)
            .last()
            .unwrap_or(0);
        format!("{}...", &trimmed[..end])
    }
}
