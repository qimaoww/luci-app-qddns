use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use crate::error::{Error, Result};
use ureq::http::{Method, Request, Response};
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, Either, LazyBuffers, NextTimeout, RustlsConnector,
    Transport,
};

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
        self.execute_with_interface(request, None, retry)
    }

    pub fn execute_with_interface(
        &self,
        request: &HttpRequest,
        interface: Option<&str>,
        retry: RetryPolicy,
    ) -> Result<HttpResponse> {
        let attempts = retry.max_attempts.max(1);
        let mut last_error = None;

        for attempt in 1..=attempts {
            match self.execute_once(request, interface) {
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

    fn execute_once(&self, request: &HttpRequest, interface: Option<&str>) -> Result<HttpResponse> {
        if request.url.starts_with("file://") {
            return Err(Error::new("unsupported provider url scheme: file"));
        }

        let agent = if let Some(interface) = interface.filter(|value| !value.trim().is_empty()) {
            let config = ureq::config::Config::builder()
                .http_status_as_error(false)
                .timeout_global(Some(self.timeout))
                .build();
            let connector = InterfaceBoundConnector::new(interface.to_string());
            ureq::Agent::with_parts(
                config,
                connector.chain(RustlsConnector::default()),
                DefaultResolver::default(),
            )
        } else {
            let config = ureq::config::Config::builder()
                .http_status_as_error(false)
                .timeout_global(Some(self.timeout))
                .build();
            ureq::Agent::new_with_config(config)
        };

        let method = request.method.parse::<Method>().map_err(|err| {
            Error::new(format!("invalid HTTP method '{}': {err}", request.method))
        })?;
        let mut builder = Request::builder().method(method).uri(&request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }

        let result = if request.body.is_empty()
            || request.method.eq_ignore_ascii_case("GET")
            || request.method.eq_ignore_ascii_case("HEAD")
        {
            let request = builder
                .body(())
                .map_err(|err| Error::new(format!("invalid HTTP request: {err}")))?;
            agent.run(request)
        } else {
            let request = builder
                .body(request.body.clone())
                .map_err(|err| Error::new(format!("invalid HTTP request: {err}")))?;
            agent.run(request)
        };

        match result {
            Ok(response) => response_to_http(response, request),
            Err(ureq::Error::StatusCode(status)) => Err(Error::new(format!(
                "HTTP {status} from provider endpoint"
            ))),
            Err(err) => {
                let message = sanitize_http_error_text(&err.to_string(), request);
                let lowered = message.to_ascii_lowercase();
                if lowered.contains("timed out") || lowered.contains("timeout") {
                    Err(Error::new(format!("HTTP request timed out: {message}")))
                } else {
                    Err(Error::new(format!("HTTP request failed: {message}")))
                }
            }
        }
    }
}

fn response_to_http(mut response: Response<ureq::Body>, request: &HttpRequest) -> Result<HttpResponse> {
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|err| Error::new(format!("failed to read HTTP response body: {err}")))?;
    if status >= 400 {
        let body = sanitize_http_error_text(&body, request);
        return Err(Error::new(format!(
            "HTTP {status} from provider endpoint: {}",
            trim_error_body(&body)
        )));
    }
    Ok(HttpResponse { status, body })
}

fn is_retryable_error(err: &Error) -> bool {
    let text = err.to_string();
    text.contains("HTTP 5")
        || text.contains("timed out")
        || text.contains("timeout")
        || text.contains("connection")
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
    ["authorization", "cookie", "password", "secret", "token", "key"]
        .iter()
        .any(|marker| lowered.contains(marker))
}

fn push_secret_candidates(candidates: &mut Vec<String>, value: &str) {
    let trimmed = value.trim();
    if trimmed.len() >= 4 {
        candidates.push(trimmed.to_string());
    }
    for token in trimmed.split(|ch: char| {
        ch.is_whitespace() || matches!(ch, '&' | '=' | ',' | ':' | '"' | '\'' | '{' | '}' | '[' | ']')
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

#[derive(Debug, Clone)]
struct InterfaceBoundConnector {
    interface: String,
}

impl InterfaceBoundConnector {
    fn new(interface: String) -> Self {
        Self { interface }
    }

    fn connect_tcp(&self, addr: SocketAddr, timeout: Duration) -> Result<TcpStream> {
        let socket = socket2::Socket::new(
            match addr {
                SocketAddr::V4(_) => socket2::Domain::IPV4,
                SocketAddr::V6(_) => socket2::Domain::IPV6,
            },
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )
        .map_err(|err| Error::new(format!("failed to create socket: {err}")))?;

        socket
            .bind_device(Some(self.interface.as_bytes()))
            .map_err(|err| Error::new(format!("failed to bind interface '{}': {err}", self.interface)))?;
        socket
            .set_nonblocking(false)
            .map_err(|err| Error::new(format!("failed to configure socket: {err}")))?;

        socket
            .connect_timeout(&addr.into(), timeout)
            .map_err(|err| Error::new(format!("failed to connect socket: {err}")))?;

        Ok(socket.into())
    }
}

impl fmt::Display for InterfaceBoundConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.interface)
    }
}

impl<In: Transport> Connector<In> for InterfaceBoundConnector {
    type Out = Either<In, InterfaceBoundTransport>;

    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<In>,
    ) -> std::result::Result<Option<Self::Out>, ureq::Error> {
        if chained.is_some() {
            return Ok(chained.map(Either::A));
        }

        let timeout = details
            .timeout
            .not_zero()
            .map(|duration| *duration)
            .unwrap_or(Duration::from_secs(30));

        for addr in &details.addrs {
            match self.connect_tcp(*addr, timeout) {
                Ok(stream) => {
                    if details.config.no_delay() {
                        stream.set_nodelay(true)?;
                    }
                    let buffers = LazyBuffers::new(
                        details.config.input_buffer_size(),
                        details.config.output_buffer_size(),
                    );
                    return Ok(Some(Either::B(InterfaceBoundTransport::new(stream, buffers))));
                }
                Err(err) => {
                    return Err(std::io::Error::new(std::io::ErrorKind::Other, err.to_string()).into())
                }
            }
        }

        Err(ureq::Error::ConnectionFailed)
    }
}

struct InterfaceBoundTransport {
    stream: TcpStream,
    buffers: LazyBuffers,
}

impl InterfaceBoundTransport {
    fn new(stream: TcpStream, buffers: LazyBuffers) -> Self {
        Self { stream, buffers }
    }
}

impl fmt::Debug for InterfaceBoundTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InterfaceBoundTransport")
            .field("addr", &self.stream.peer_addr().ok())
            .finish()
    }
}

impl Transport for InterfaceBoundTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(
        &mut self,
        amount: usize,
        timeout: NextTimeout,
    ) -> std::result::Result<(), ureq::Error> {
        if let Some(duration) = timeout.not_zero() {
            self.stream.set_write_timeout(Some(*duration))?;
        }
        self.stream.write_all(&self.buffers.output()[..amount])?;
        Ok(())
    }

    fn await_input(&mut self, timeout: NextTimeout) -> std::result::Result<bool, ureq::Error> {
        if let Some(duration) = timeout.not_zero() {
            self.stream.set_read_timeout(Some(*duration))?;
        }
        let input = self.buffers.input_append_buf();
        let amount = self.stream.read(input)?;
        self.buffers.input_appended(amount);
        Ok(amount > 0)
    }

    fn is_open(&mut self) -> bool {
        true
    }
}
