use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use qddns::config::{Config, ProviderConfig, RuleConfig, SourceConfig};
use qddns::daemon::{self, DaemonOptions};
use qddns::logstore::{append_log, read_logs, LogEntry};
use qddns::state::{serialize_runtime_state, RuleState, RuntimeState};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("qddns-runtime-test-{unique}"));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_config(path: &Path, state_dir: &Path, log_dir: &Path) {
    let lookup_path = state_dir.join("lookup.txt");
    let update_path = state_dir.join("update.txt");
    fs::write(&lookup_path, "198.51.100.99\n").unwrap();
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
    option lookup_url 'file://{}'
    option url 'file://{}'
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
            lookup_path.display(),
            update_path.display()
        ),
    )
    .unwrap();
}

#[test]
fn append_and_read_logs_roundtrip() {
    let temp = TempDir::new();
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
    let temp = TempDir::new();
    let err = read_logs(temp.path().to_str().unwrap(), Some("../system"), 10).unwrap_err();
    assert!(err.to_string().contains("invalid log scope"), "error was: {err}");
}

#[test]
fn run_rule_once_writes_rule_log_and_state_file() {
    let temp = TempDir::new();
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    write_config(&config_path, &state_dir, &log_dir);

    daemon::run_rule_once(config_path.to_str().unwrap(), "home").unwrap();

    let log_path = log_dir.join("home.log");
    assert!(log_path.exists(), "expected rule log at {}", log_path.display());

    let state_path = state_dir.join("runtime.state");
    assert!(state_path.exists(), "expected runtime state at {}", state_path.display());

    let state_text = fs::read_to_string(&state_path).unwrap();
    assert!(state_text.contains("home"), "state file was: {state_text}");
    assert!(state_text.contains("\"daemon_running\":false"), "state file was: {state_text}");

    let marker_path = state_dir.join("daemon.status");
    assert!(!marker_path.exists(), "daemon marker should be absent after one-shot run");

    let update_path = state_dir.join("update.txt");
    let update_text = fs::read_to_string(update_path).unwrap();
    assert!(update_text.contains("198.51.100.10"), "update file was: {update_text}");

    #[cfg(unix)]
    {
        assert_eq!(fs::metadata(&log_dir).unwrap().permissions().mode() & 0o777, 0o750);
        assert_eq!(fs::metadata(&state_dir).unwrap().permissions().mode() & 0o777, 0o750);
        assert_eq!(fs::metadata(&log_path).unwrap().permissions().mode() & 0o777, 0o640);
        assert_eq!(fs::metadata(state_path).unwrap().permissions().mode() & 0o777, 0o640);
    }
}

#[test]
fn daemon_once_batch_keeps_runtime_marked_not_running() {
    let temp = TempDir::new();
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    write_config(&config_path, &state_dir, &log_dir);

    daemon::run(DaemonOptions {
        config: config_path.display().to_string(),
        once: true,
    })
    .unwrap();

    let state_text = fs::read_to_string(state_dir.join("runtime.state")).unwrap();
    assert!(state_text.contains("\"daemon_running\":false"), "state file was: {state_text}");
    assert!(!state_dir.join("daemon.status").exists());
}

#[test]
fn failed_one_shot_run_clears_stale_daemon_running_state() {
    let temp = TempDir::new();
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    write_config(&config_path, &state_dir, &log_dir);
    let broken_config = fs::read_to_string(&config_path)
        .unwrap()
        .replace(
            &format!("    option lookup_url 'file://{}'\n", state_dir.join("lookup.txt").display()),
            "",
        )
        .replace(
            &format!("    option url 'file://{}'\n", state_dir.join("update.txt").display()),
            "",
        );
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

    let err = daemon::run_rule_once(config_path.to_str().unwrap(), "home").unwrap_err();
    assert!(
        err.to_string().contains("missing lookup_url or url"),
        "unexpected error: {err}"
    );

    let state_text = fs::read_to_string(state_dir.join("runtime.state")).unwrap();
    assert!(state_text.contains("\"daemon_running\":false"), "state file was: {state_text}");
    assert!(state_text.contains("\"status\":\"error\""), "state file was: {state_text}");
    assert!(!state_dir.join("daemon.status").exists(), "daemon marker should be removed after failure");
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
        },
    );

    let text = qddns::state::serialize_runtime_state(&runtime);
    let parsed = qddns::state::parse_runtime_state(&text).expect("state parses");
    assert_eq!(parsed.rules["home"].status, "success");
    assert_eq!(parsed.rules["home"].next_run, Some(260));
}

#[test]
fn fixture_config_shape_covers_runtime_dependencies() {
    let config = Config {
        main: Default::default(),
        sources: BTreeMap::from([(
            "wan4".into(),
            SourceConfig {
                name: "wan4".into(),
                source_type: "local_addr".into(),
                family: Some("ipv4".into()),
                interface: None,
                address: Some("198.51.100.10".into()),
                probe_url: None,
                script: None,
                command: None,
                duid: None,
                iaid: None,
                lease_file: None,
                prefix_filter: None,
                hostname_hint: None,
            },
        )]),
        providers: BTreeMap::from([(
            "custom".into(),
            ProviderConfig {
                name: "custom".into(),
                provider_type: "custom_http".into(),
                api_token: None,
                secret_id: None,
                secret_key: None,
                access_key_id: None,
                access_key_secret: None,
                url: Some("https://example.com/update".into()),
                method: Some("POST".into()),
                headers_json: Some("{\"Authorization\":\"Bearer token\"}".into()),
                body_template: Some("{\"ip\":\"{{ip}}\"}".into()),
                lookup_url: None,
                lookup_method: None,
                lookup_headers_json: None,
                lookup_json_pointer: None,
                success_contains: Some("ok".into()),
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

    assert_eq!(config.providers["custom"].provider_type, "custom_http");
}
