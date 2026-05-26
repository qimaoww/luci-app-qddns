use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use qddns::config::{
    AddressFamily, Config, CustomHttpConfig, ProviderConfig, ProviderKind, RuleConfig,
    SourceConfig, SourceKind,
};
use qddns::daemon::{self, DaemonOptions};
use qddns::logstore::{append_log, read_logs, LogEntry};
use qddns::state::{
    runtime_status_json, serialize_runtime_state, RuleState, RuntimeState, StateStore,
};
use serde_json::Value;

mod support;
use support::{MockHttpServer, MockResponse, TempDir};

fn write_config(path: &Path, state_dir: &Path, log_dir: &Path, lookup_url: &str, update_url: &str) {
    fs::write(
        path,
        format!(
            r#"
config qddns 'main'
    option enabled '1'
    option state_dir '{}'
    option log_dir '{}'

config source 'wan4'
    option type 'local_addr'
    option family 'ipv4'
    option address '198.51.100.10'

config provider 'custom'
    option type 'custom_http'
    option lookup_url '{}'
    option url '{}'
    option method 'POST'
    option body_template '{{"ip":"{{{{ip}}}}","record":"{{{{record_name}}}}","zone":"{{{{zone}}}}","result":"updated"}}'
    option success_contains 'updated'

config rule 'home'
    option enabled '1'
    option provider 'custom'
    option source 'wan4'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'home'
    option ttl '300'
    option proxied '0'
    option check_interval '60'
    option force_interval '3600'
    option retry_count '3'
    option retry_backoff '30'
"#,
            state_dir.display(),
            log_dir.display(),
            lookup_url,
            update_url
        ),
    )
    .unwrap();
}

fn write_broken_update_config(path: &Path, state_dir: &Path, log_dir: &Path) {
    fs::write(
        path,
        format!(
            r#"
config qddns 'main'
    option enabled '1'
    option state_dir '{}'
    option log_dir '{}'

config source 'wan4'
    option type 'local_addr'
    option family 'ipv4'
    option address '198.51.100.10'

config provider 'custom'
    option type 'custom_http'
    option method 'POST'
    option body_template '{{"ip":"{{{{ip}}}}"}}'
    option success_contains 'updated'

config rule 'home'
    option enabled '1'
    option provider 'custom'
    option source 'wan4'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'home'
    option ttl '300'
    option proxied '0'
    option check_interval '600'
    option force_interval '3600'
    option retry_count '1'
    option retry_backoff '30'
"#,
            state_dir.display(),
            log_dir.display(),
        ),
    )
    .unwrap();
}

fn start_rule_mock() -> Option<MockHttpServer> {
    match MockHttpServer::try_responses(vec![
        MockResponse::new(200, "198.51.100.99\n"),
        MockResponse::new(
            200,
            "{\"ip\":\"198.51.100.10\",\"record\":\"home\",\"zone\":\"example.com\",\"result\":\"updated\"}",
        ),
    ]) {
        Ok(server) => Some(server),
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("local TCP bind unavailable in this sandbox; skipping HTTP-backed runtime run");
            None
        }
        Err(err) => panic!("bind mock server: {err}"),
    }
}

#[test]
fn append_and_read_logs_roundtrip() {
    let temp = TempDir::new("qddns-runtime-test");
    append_log(
        temp.path().to_str().unwrap(),
        "home",
        &LogEntry {
            timestamp: 100,
            level: "info".into(),
            scope: "home".into(),
            message: "updated".into(),
        },
    )
    .unwrap();
    append_log(
        temp.path().to_str().unwrap(),
        "home",
        &LogEntry {
            timestamp: 101,
            level: "error".into(),
            scope: "home".into(),
            message: "failed".into(),
        },
    )
    .unwrap();

    let logs = read_logs(temp.path().to_str().unwrap(), Some("home"), 10).unwrap();
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].message, "updated");
    assert_eq!(logs[1].message, "failed");
}

#[test]
fn read_logs_rejects_invalid_scope() {
    let temp = TempDir::new("qddns-runtime-test");
    let err = read_logs(temp.path().to_str().unwrap(), Some("../system"), 10).unwrap_err();
    assert!(
        err.to_string().contains("invalid log scope"),
        "error was: {err}"
    );
}

#[test]
fn run_rule_once_writes_rule_log_and_state_file() {
    let Some(server) = start_rule_mock() else {
        return;
    };
    let temp = TempDir::new("qddns-runtime-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    write_config(
        &config_path,
        &state_dir,
        &log_dir,
        &server.url("/lookup"),
        &server.url("/update"),
    );

    daemon::run_rule_once(config_path.to_str().unwrap(), "home").unwrap();

    let log_path = log_dir.join("home.log");
    assert!(
        log_path.exists(),
        "expected rule log at {}",
        log_path.display()
    );

    let state_path = state_dir.join("runtime.state");
    assert!(
        state_path.exists(),
        "expected runtime state at {}",
        state_path.display()
    );

    let state_text = fs::read_to_string(&state_path).unwrap();
    assert!(state_text.contains("home"), "state file was: {state_text}");
    assert!(
        state_text.contains("\"daemon_running\":false"),
        "state file was: {state_text}"
    );

    let marker_path = state_dir.join("daemon.status");
    assert!(
        !marker_path.exists(),
        "daemon marker should be absent after one-shot run"
    );

    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].path, "/lookup");
    assert_eq!(requests[1].path, "/update");
    assert!(
        requests[1].body.contains("198.51.100.10"),
        "update request was: {}",
        requests[1].body
    );

    #[cfg(unix)]
    {
        assert_eq!(
            fs::metadata(&log_dir).unwrap().permissions().mode() & 0o777,
            0o750
        );
        assert_eq!(
            fs::metadata(&state_dir).unwrap().permissions().mode() & 0o777,
            0o750
        );
        assert_eq!(
            fs::metadata(&log_path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(
            fs::metadata(state_path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}

#[test]
fn daemon_once_batch_keeps_runtime_marked_not_running() {
    let Some(server) = start_rule_mock() else {
        return;
    };
    let temp = TempDir::new("qddns-runtime-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    write_config(
        &config_path,
        &state_dir,
        &log_dir,
        &server.url("/lookup"),
        &server.url("/update"),
    );

    daemon::run(DaemonOptions {
        config: config_path.display().to_string(),
        once: true,
    })
    .unwrap();

    let state_text = fs::read_to_string(state_dir.join("runtime.state")).unwrap();
    assert!(
        state_text.contains("\"daemon_running\":false"),
        "state file was: {state_text}"
    );
    assert!(!state_dir.join("daemon.status").exists());
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn failed_daemon_once_clears_stale_daemon_running_state() {
    let temp = TempDir::new("qddns-runtime-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    write_config(
        &config_path,
        &state_dir,
        &log_dir,
        "http://127.0.0.1/unused-lookup",
        "http://127.0.0.1/unused-update",
    );
    let broken_config = fs::read_to_string(&config_path)
        .unwrap()
        .replace(
            "    option lookup_url 'http://127.0.0.1/unused-lookup'\n",
            "",
        )
        .replace("    option url 'http://127.0.0.1/unused-update'\n", "");
    fs::write(&config_path, broken_config).unwrap();
    fs::write(
        state_dir.join("runtime.state"),
        serialize_runtime_state(&RuntimeState {
            daemon_running: true,
            updated_at: Some(123),
            rules: BTreeMap::new(),
        }),
    )
    .unwrap();
    fs::write(state_dir.join("daemon.status"), "running=1\n").unwrap();

    let err = daemon::run(DaemonOptions {
        config: config_path.display().to_string(),
        once: true,
    })
    .unwrap_err();
    assert!(
        err.to_string().contains("missing lookup_url or url"),
        "unexpected error: {err}"
    );

    let state_text = fs::read_to_string(state_dir.join("runtime.state")).unwrap();
    assert!(
        state_text.contains("\"daemon_running\":false"),
        "state file was: {state_text}"
    );
    assert!(
        state_text.contains("\"status\":\"error\""),
        "state file was: {state_text}"
    );
    assert!(
        !state_dir.join("daemon.status").exists(),
        "daemon marker should be removed after failure"
    );
}

#[test]
fn once_returns_nonzero_when_any_rule_fails() {
    let temp = TempDir::new("qddns-runtime-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    write_broken_update_config(&config_path, &state_dir, &log_dir);

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .args(["run", "--quiet", "--bin", "qddnsd", "--", "--config"])
        .arg(config_path.as_os_str())
        .arg("--once")
        .output()
        .expect("run qddnsd once");

    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("home") || stderr.contains("missing lookup_url or url"),
        "stderr should explain failing rule: {stderr}"
    );
}

#[test]
fn manual_run_does_not_touch_daemon_marker() {
    let temp = TempDir::new("qddns-runtime-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    write_broken_update_config(&config_path, &state_dir, &log_dir);
    fs::write(
        state_dir.join("runtime.state"),
        serialize_runtime_state(&RuntimeState {
            daemon_running: true,
            updated_at: Some(123),
            rules: BTreeMap::new(),
        }),
    )
    .unwrap();
    fs::write(state_dir.join("daemon.status"), "running=1\n").unwrap();

    let err = daemon::run_rule_once(config_path.to_str().unwrap(), "home").unwrap_err();
    assert!(err.to_string().contains("missing lookup_url or url"));

    assert_eq!(
        fs::read_to_string(state_dir.join("daemon.status")).unwrap(),
        "running=1\n"
    );
    let state_text = fs::read_to_string(state_dir.join("runtime.state")).unwrap();
    assert!(
        state_text.contains("\"daemon_running\":true"),
        "manual run must not mark daemon stopped: {state_text}"
    );
}

#[test]
fn retry_count_is_consumed_by_rule_runner() {
    let temp = TempDir::new("qddns-runtime-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    write_broken_update_config(&config_path, &state_dir, &log_dir);
    fs::write(
        state_dir.join("runtime.state"),
        r#"{"daemon_running":true,"updated_at":100,"rules":{"home":{"status":"error","current_ip":null,"remote_ip":null,"last_result":"error","last_error":"previous","last_update":null,"last_check":100,"next_run":130,"retry_attempts":1}}}"#,
    )
    .unwrap();

    let _ = daemon::run_rule_once(config_path.to_str().unwrap(), "home");
    let state_text = fs::read_to_string(state_dir.join("runtime.state")).unwrap();
    assert!(
        state_text.contains("\"retry_attempts\":2"),
        "state was: {state_text}"
    );
    let parsed = qddns::state::parse_runtime_state(&state_text).unwrap();
    let rule = &parsed.rules["home"];
    assert_eq!(
        rule.next_run.unwrap() - rule.last_check.unwrap(),
        600,
        "retry_count should be exhausted after the second consecutive failure"
    );
}

#[test]
fn runtime_state_serializes_and_parses_rule_status() {
    let mut runtime = RuntimeState::default();
    runtime.daemon_running = true;
    runtime.updated_at = Some(200);
    runtime.rules.insert(
        "home".into(),
        RuleState {
            status: "success".into(),
            current_ip: Some("198.51.100.10".into()),
            remote_ip: Some("198.51.100.10".into()),
            last_result: Some("unchanged".into()),
            last_error: None,
            last_update: Some(100),
            last_check: Some(200),
            next_run: Some(260),
            retry_attempts: 0,
        },
    );

    let text = qddns::state::serialize_runtime_state(&runtime);
    let parsed = qddns::state::parse_runtime_state(&text).expect("state parses");
    assert_eq!(parsed.rules["home"].status, "success");
    assert_eq!(parsed.rules["home"].next_run, Some(260));
}

#[test]
fn state_store_atomic_write_and_recovery() {
    let temp = TempDir::new("qddns-state-store");
    let store = StateStore::new(temp.path());
    let mut runtime = RuntimeState::default();
    runtime.daemon_running = true;
    runtime.updated_at = Some(300);
    runtime.rules.insert(
        "home".into(),
        RuleState {
            status: "success".into(),
            current_ip: Some("198.51.100.10".into()),
            remote_ip: Some("198.51.100.10".into()),
            last_result: Some("updated".into()),
            last_error: None,
            last_update: Some(300),
            last_check: Some(300),
            next_run: Some(360),
            retry_attempts: 0,
        },
    );

    store.write_runtime(&runtime).expect("state write succeeds");
    let path = temp.path().join("runtime.state");
    let text = fs::read_to_string(&path).expect("state file readable");
    assert!(
        text.ends_with('}'),
        "state file should contain complete JSON: {text}"
    );

    let recovered = store.read_runtime().expect("state reads back");
    assert_eq!(recovered.rules["home"].last_update, Some(300));
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[test]
fn state_store_rejects_corrupt_json() {
    let temp = TempDir::new("qddns-state-store");
    fs::create_dir_all(temp.path()).unwrap();
    fs::write(
        temp.path().join("runtime.state"),
        "{\"daemon_running\":true,\"rules\":",
    )
    .unwrap();

    let err = StateStore::new(temp.path())
        .read_runtime()
        .expect_err("corrupt state must not silently become default");
    assert!(
        err.to_string().contains("corrupt runtime state"),
        "unexpected error: {err}"
    );
    let still_corrupt = fs::read_to_string(temp.path().join("runtime.state")).unwrap();
    assert!(still_corrupt.ends_with("\"rules\":"));
}

#[test]
fn state_store_concurrent_writers() {
    let temp = TempDir::new("qddns-state-store");
    let state_dir = temp.path().to_path_buf();
    let mut handles = Vec::new();

    for idx in 0..8 {
        let dir = state_dir.clone();
        handles.push(std::thread::spawn(move || {
            let store = StateStore::new(&dir);
            let mut runtime = RuntimeState::default();
            runtime.updated_at = Some(idx);
            runtime.rules.insert(
                format!("rule{idx}"),
                RuleState {
                    status: "success".into(),
                    current_ip: Some(format!("198.51.100.{idx}")),
                    remote_ip: Some(format!("198.51.100.{idx}")),
                    last_result: Some("updated".into()),
                    last_error: None,
                    last_update: Some(idx),
                    last_check: Some(idx),
                    next_run: Some(idx + 60),
                    retry_attempts: 0,
                },
            );
            store
                .write_runtime(&runtime)
                .expect("concurrent state write");
        }));
    }

    for handle in handles {
        handle.join().expect("writer thread");
    }

    let text = fs::read_to_string(state_dir.join("runtime.state")).expect("state file readable");
    let parsed = qddns::state::parse_runtime_state(&text).expect("state must be complete JSON");
    assert_eq!(parsed.rules.len(), 1);
}

#[test]
fn recent_results_are_sorted_by_time_desc() {
    let (config, runtime) = recent_fixture(10);
    let json = runtime_status_json(&config, &runtime);
    let ids = recent_ids(&json);

    assert_eq!(ids[0], "rule9");
    assert_eq!(ids[1], "rule8");
    assert_eq!(ids[2], "rule7");
}

#[test]
fn recent_results_limits_to_eight_entries() {
    let (config, runtime) = recent_fixture(10);
    let json = runtime_status_json(&config, &runtime);
    let ids = recent_ids(&json);

    assert_eq!(ids.len(), 8);
    assert_eq!(ids.last().map(String::as_str), Some("rule2"));
}

#[test]
fn recent_results_missing_time_sorts_last() {
    let (config, mut runtime) = recent_fixture(2);
    runtime
        .rules
        .insert("z_missing".into(), RuleState::default());
    runtime
        .rules
        .insert("a_missing".into(), RuleState::default());

    let json = runtime_status_json(&config, &runtime);
    let ids = recent_ids(&json);

    assert_eq!(&ids[0..2], ["rule1".to_string(), "rule0".to_string()]);
    assert_eq!(
        &ids[2..4],
        ["a_missing".to_string(), "z_missing".to_string()],
        "missing-time entries should sort last by id"
    );
}

fn recent_fixture(count: u64) -> (Config, RuntimeState) {
    let mut config = Config::default();
    let mut runtime = RuntimeState::default();
    for idx in 0..count {
        let id = format!("rule{idx}");
        config.rules.insert(
            id.clone(),
            RuleConfig {
                name: id.clone(),
                enabled: true,
                provider: "custom".into(),
                source: "wan4".into(),
                record_type: "A".into(),
                zone: "example.com".into(),
                record_name: id.clone(),
                ttl: 300,
                proxied: false,
                check_interval: 60,
                force_interval: 3600,
                retry_count: 3,
                retry_backoff: 30,
            },
        );
        runtime.rules.insert(
            id.clone(),
            RuleState {
                status: "success".into(),
                current_ip: Some(format!("198.51.100.{idx}")),
                remote_ip: Some(format!("198.51.100.{idx}")),
                last_result: Some("updated".into()),
                last_error: None,
                last_update: Some(100 + idx),
                last_check: Some(50 + idx),
                next_run: Some(200 + idx),
                retry_attempts: 0,
            },
        );
    }
    (config, runtime)
}

fn recent_ids(value: &Value) -> Vec<String> {
    let recent = value
        .pointer("/recent_results")
        .and_then(Value::as_array)
        .expect("recent results array");
    recent
        .iter()
        .map(|item| {
            item.pointer("/id")
                .and_then(Value::as_str)
                .expect("recent id")
                .to_string()
        })
        .collect()
}

#[test]
fn fixture_config_shape_covers_runtime_dependencies() {
    let config = Config {
        main: Default::default(),
        sources: BTreeMap::from([(
            "wan4".into(),
            SourceConfig {
                name: "wan4".into(),
                kind: SourceKind::LocalAddr {
                    family: Some(AddressFamily::Ipv4),
                    address: Some("198.51.100.10".into()),
                },
            },
        )]),
        providers: BTreeMap::from([(
            "custom".into(),
            ProviderConfig {
                name: "custom".into(),
                kind: ProviderKind::CustomHttp(CustomHttpConfig {
                    url: Some("https://example.com/update".into()),
                    method: Some("POST".into()),
                    headers_json: Some("{\"Authorization\":\"Bearer token\"}".into()),
                    body_template: Some("{\"ip\":\"{{ip}}\"}".into()),
                    lookup_url: None,
                    lookup_method: None,
                    lookup_headers_json: None,
                    lookup_json_pointer: None,
                    success_contains: Some("ok".into()),
                }),
            },
        )]),
        rules: BTreeMap::from([(
            "home".into(),
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
            },
        )]),
    };

    assert_eq!(config.providers["custom"].provider_type(), "custom_http");
}
