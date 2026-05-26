#![allow(dead_code)]

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.path.join(name);
        fs::write(&path, content).expect("write fixture");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug, Clone)]
pub struct CapturedRequest {
    pub method: String,
    pub path: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub status: u16,
    pub body: String,
    pub delay: Duration,
}

impl MockResponse {
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
            delay: Duration::from_millis(0),
        }
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

#[derive(Debug)]
pub struct MockHttpServer {
    base_url: String,
    shutdown: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl MockHttpServer {
    pub fn try_single_response(status: u16, body: impl Into<String>) -> std::io::Result<Self> {
        Self::try_responses(vec![MockResponse::new(status, body)])
    }

    pub fn try_responses(responses: Vec<MockResponse>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr().expect("mock addr");
        let shutdown = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let pending = Arc::new(Mutex::new(VecDeque::from(responses)));
        let thread_shutdown = Arc::clone(&shutdown);
        let thread_requests = Arc::clone(&requests);
        let thread_pending = Arc::clone(&pending);
        let handle = thread::spawn(move || loop {
            if thread_shutdown.load(Ordering::SeqCst) {
                break;
            }

            match listener.accept() {
                Ok((stream, _)) => {
                    handle_connection(stream, &thread_requests, &thread_pending);
                    if thread_pending.lock().expect("responses lock").is_empty() {
                        break;
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        });

        Ok(Self {
            base_url: format!("http://{addr}"),
            shutdown,
            requests,
            handle: Some(handle),
        })
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    pub fn requests(&self) -> Vec<CapturedRequest> {
        self.requests.lock().expect("requests lock").clone()
    }
}

impl Drop for MockHttpServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.base_url.trim_start_matches("http://"));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    requests: &Arc<Mutex<Vec<CapturedRequest>>>,
    responses: &Arc<Mutex<VecDeque<MockResponse>>>,
) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut first = String::new();
    reader.read_line(&mut first).expect("read request line");
    if first.trim().is_empty() {
        return;
    }
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("read header");
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    let mut body_bytes = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body_bytes).expect("read body");
    }
    let request_body = String::from_utf8_lossy(&body_bytes).to_string();
    requests
        .lock()
        .expect("requests lock")
        .push(CapturedRequest {
            method,
            path,
            body: request_body,
        });

    let response = responses
        .lock()
        .expect("responses lock")
        .pop_front()
        .unwrap_or_else(|| MockResponse::new(500, "unexpected request"));
    if !response.delay.is_zero() {
        thread::sleep(response.delay);
    }
    let reason = if response.status < 400 { "OK" } else { "ERROR" };
    let response_text = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response.status,
        reason,
        response.body.len(),
        response.body
    );
    stream
        .write_all(response_text.as_bytes())
        .expect("write response");
}

pub fn run_qddnsctl(config: &Path, args: &[&str]) -> Output {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    Command::new(cargo)
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config)
        .args(args)
        .output()
        .expect("run qddnsctl")
}

pub fn assert_secret_absent(secret: &str, values: &[&[u8]]) {
    for value in values {
        let text = String::from_utf8_lossy(value);
        assert!(!text.contains(secret), "secret leaked in output: {text}");
    }
}
