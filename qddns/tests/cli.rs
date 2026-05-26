use std::fs;
use std::process::Command;

use qddns::state::{serialize_runtime_state, RuleState, RuntimeState};
use serde_json::Value;

mod support;
use support::{MockHttpServer, MockResponse, TempDir};

fn parse_output_json(output: &[u8]) -> Value {
    let stdout = String::from_utf8_lossy(output);
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|err| panic!("stdout was not valid json: {stdout}\nerror: {err}"))
}

fn json_bool(value: &Value, path: &str) -> bool {
    value
        .pointer(path)
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("missing bool at {path}: {value:?}"))
}

fn json_str<'a>(value: &'a Value, path: &str) -> &'a str {
    value
        .pointer(path)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string at {path}: {value:?}"))
}

fn json_u64(value: &Value, path: &str) -> u64 {
    value
        .pointer(path)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing number at {path}: {value:?}"))
}

#[test]
fn qddnsctl_validate_reports_success_for_valid_config() {
    let temp = TempDir::new("qddns-cli-test");
    let config_path = temp.path().join("qddns.conf");
    fs::write(
        &config_path,
        r#"
config qddns 'main'

config source 'ipv4_local'
    option type 'local_addr'
    option address '1.2.3.4'

config provider 'cf'
    option type 'cloudflare'
    option api_token 'token'

config rule 'ok'
    option enabled '1'
    option provider 'cf'
    option source 'ipv4_local'
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
    )
    .unwrap();

    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .arg("validate")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":true"), "stdout was: {stdout}");
}

#[test]
fn qddnsctl_status_reports_runtime_rule_states() {
    let server = match MockHttpServer::try_responses(vec![
        MockResponse::new(200, "198.51.100.33\n"),
        MockResponse::new(200, "{\"ip\":\"198.51.100.44\",\"result\":\"updated\"}"),
    ]) {
        Ok(server) => server,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("local TCP bind unavailable in this sandbox; skipping HTTP-backed CLI run");
            return;
        }
        Err(err) => panic!("bind mock server: {err}"),
    };
    let temp = TempDir::new("qddns-cli-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    fs::write(
        &config_path,
        format!(
            r#"
config qddns 'main'
    option enabled '1'
    option state_dir '{}'
    option log_dir '{}'

config source 'wan4'
    option type 'local_addr'
    option address '198.51.100.44'

config provider 'custom'
    option type 'custom_http'
    option lookup_url '{}'
    option url '{}'
    option method 'POST'
    option body_template '{{"ip":"{{{{ip}}}}","result":"updated"}}'
    option success_contains 'updated'

config rule 'home'
    option enabled '1'
    option provider 'custom'
    option source 'wan4'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'home'
    option ttl '300'
    option check_interval '60'
    option force_interval '3600'
    option retry_count '3'
    option retry_backoff '30'
"#,
            state_dir.display(),
            log_dir.display(),
            server.url("/lookup"),
            server.url("/update")
        ),
    )
    .unwrap();

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let run_output = Command::new(&cargo)
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .args(["rules", "run", "home"])
        .output()
        .unwrap();
    assert!(run_output.status.success(), "{run_output:?}");

    let status_output = Command::new(&cargo)
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .arg("status")
        .output()
        .unwrap();

    assert!(status_output.status.success(), "{status_output:?}");
    let json = parse_output_json(&status_output.stdout);
    assert!(json_bool(&json, "/ok"));
    assert!(!json_bool(&json, "/running"));
    assert_eq!(json_str(&json, "/rule_states/home/status"), "success");
    assert_eq!(json_str(&json, "/rule_states/home/last_result"), "updated");
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/lookup");
    assert_eq!(requests[1].method, "POST");
    assert_eq!(requests[1].path, "/update");
    assert!(requests[1].body.contains("198.51.100.44"));
}

#[test]
fn qddnsctl_rule_status_matches_runtime_status_contract() {
    let temp = TempDir::new("qddns-cli-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    fs::write(
        &config_path,
        format!(
            r#"
config qddns 'main'
    option enabled '1'
    option state_dir '{}'
    option log_dir '{}'

config source 'wan4'
    option type 'local_addr'
    option address '198.51.100.44'

config provider 'custom'
    option type 'custom_http'
    option url 'http://127.0.0.1/unused'
    option method 'POST'
    option body_template '{{"ip":"{{{{ip}}}}","result":"updated"}}'
    option success_contains 'updated'

config rule 'home'
    option enabled '1'
    option provider 'custom'
    option source 'wan4'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'home'
    option ttl '300'
    option check_interval '60'
    option force_interval '3600'
    option retry_count '3'
    option retry_backoff '30'
"#,
            state_dir.display(),
            log_dir.display()
        ),
    )
    .unwrap();

    let runtime = RuntimeState {
        daemon_running: true,
        updated_at: Some(200),
        rules: std::collections::BTreeMap::from([(
            "home".into(),
            RuleState {
                status: "success".into(),
                current_ip: Some("198.51.100.44".into()),
                remote_ip: Some("198.51.100.33".into()),
                last_result: Some("updated".into()),
                last_error: None,
                last_update: Some(190),
                last_check: Some(200),
                next_run: Some(260),
                retry_attempts: 0,
            },
        )]),
    };
    fs::write(
        state_dir.join("runtime.state"),
        serialize_runtime_state(&runtime),
    )
    .unwrap();

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status_output = Command::new(&cargo)
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .arg("status")
        .output()
        .unwrap();
    let rule_output = Command::new(&cargo)
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .args(["rules", "status", "home"])
        .output()
        .unwrap();

    assert!(status_output.status.success(), "{status_output:?}");
    assert!(rule_output.status.success(), "{rule_output:?}");

    let status_json = parse_output_json(&status_output.stdout);
    let rule_json = parse_output_json(&rule_output.stdout);
    assert!(json_bool(&status_json, "/running"));
    assert!(json_bool(&rule_json, "/running"));
    for field in [
        "status",
        "current_ip",
        "remote_ip",
        "last_result",
        "last_update",
        "last_check",
        "next_run",
    ] {
        let runtime_path = format!("/rule_states/home/{field}");
        let rule_path = format!("/{field}");
        assert_eq!(
            status_json.pointer(&runtime_path),
            rule_json.pointer(&rule_path)
        );
    }
    assert_eq!(json_str(&rule_json, "/id"), "home");
    assert_eq!(json_str(&status_json, "/recent_results/0/id"), "home");
    assert_eq!(json_u64(&status_json, "/updated_at"), 200);
}

#[test]
fn cli_json_contracts_parse_with_serde() {
    let temp = TempDir::new("qddns-cli-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    fs::write(
        &config_path,
        format!(
            r#"
config qddns 'main'
    option enabled '1'
    option state_dir '{}'
    option log_dir '{}'

config source 'wan4'
    option type 'local_addr'
    option address '198.51.100.44'

config provider 'custom'
    option type 'custom_http'
    option url 'http://127.0.0.1/unused'
    option method 'POST'
    option body_template '{{"ip":"{{{{ip}}}}","result":"updated"}}'
    option success_contains 'updated'

config rule 'home'
    option enabled '1'
    option provider 'custom'
    option source 'wan4'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'home'
    option ttl '300'
    option check_interval '60'
    option force_interval '3600'
    option retry_count '3'
    option retry_backoff '30'
"#,
            state_dir.display(),
            log_dir.display()
        ),
    )
    .unwrap();

    let runtime = RuntimeState {
        daemon_running: true,
        updated_at: Some(200),
        rules: std::collections::BTreeMap::from([(
            "home".into(),
            RuleState {
                status: "success".into(),
                current_ip: Some("198.51.100.44".into()),
                remote_ip: Some("198.51.100.33".into()),
                last_result: Some("updated".into()),
                last_error: None,
                last_update: Some(190),
                last_check: Some(200),
                next_run: Some(260),
                retry_attempts: 0,
            },
        )]),
    };
    fs::write(
        state_dir.join("runtime.state"),
        serialize_runtime_state(&runtime),
    )
    .unwrap();
    fs::write(log_dir.join("home.log"), "200\tinfo\thome\tupdated\n").unwrap();

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status_output = Command::new(&cargo)
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .arg("status")
        .output()
        .unwrap();
    let rule_output = Command::new(&cargo)
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .args(["rules", "status", "home"])
        .output()
        .unwrap();
    let logs_output = Command::new(&cargo)
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .args(["logs", "home"])
        .output()
        .unwrap();

    assert!(status_output.status.success(), "{status_output:?}");
    assert!(rule_output.status.success(), "{rule_output:?}");
    assert!(logs_output.status.success(), "{logs_output:?}");

    let status_json = parse_output_json(&status_output.stdout);
    let rule_json = parse_output_json(&rule_output.stdout);
    let logs_json = parse_output_json(&logs_output.stdout);

    assert!(json_bool(&status_json, "/ok"));
    assert!(json_bool(&rule_json, "/ok"));
    assert!(json_bool(&logs_json, "/ok"));
    assert_eq!(
        json_str(&status_json, "/rule_states/home/status"),
        "success"
    );
    assert_eq!(json_str(&rule_json, "/id"), "home");
    assert_eq!(json_str(&logs_json, "/scope"), "home");
    assert_eq!(json_str(&logs_json, "/entries/0/message"), "updated");
}

#[test]
fn qddnsctl_rule_status_returns_idle_for_never_run_rule() {
    let temp = TempDir::new("qddns-cli-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    fs::write(
        &config_path,
        format!(
            r#"
config qddns 'main'
    option enabled '1'
    option state_dir '{}'
    option log_dir '{}'

config source 'wan4'
    option type 'local_addr'
    option address '198.51.100.44'

config provider 'custom'
    option type 'custom_http'
    option url 'http://127.0.0.1/unused'
    option method 'POST'
    option body_template '{{"ip":"{{{{ip}}}}","result":"updated"}}'
    option success_contains 'updated'

config rule 'home'
    option enabled '1'
    option provider 'custom'
    option source 'wan4'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'home'
    option ttl '300'
    option check_interval '60'
    option force_interval '3600'
    option retry_count '3'
    option retry_backoff '30'
"#,
            state_dir.display(),
            log_dir.display()
        ),
    )
    .unwrap();

    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .args(["rules", "status", "home"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json = parse_output_json(&output.stdout);
    assert_eq!(json_str(&json, "/status"), "idle");
    assert!(!json_bool(&json, "/running"));
}

#[test]
fn qddnsctl_logs_returns_structured_json() {
    let server = match MockHttpServer::try_responses(vec![
        MockResponse::new(200, "198.51.100.33\n"),
        MockResponse::new(200, "{\"ip\":\"198.51.100.44\",\"result\":\"updated\"}"),
    ]) {
        Ok(server) => server,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("local TCP bind unavailable in this sandbox; skipping HTTP-backed log run");
            return;
        }
        Err(err) => panic!("bind mock server: {err}"),
    };
    let temp = TempDir::new("qddns-cli-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    fs::write(
        &config_path,
        format!(
            r#"
config qddns 'main'
    option enabled '1'
    option state_dir '{}'
    option log_dir '{}'

config source 'wan4'
    option type 'local_addr'
    option address '198.51.100.44'

config provider 'custom'
    option type 'custom_http'
    option lookup_url '{}'
    option url '{}'
    option method 'POST'
    option body_template '{{"ip":"{{{{ip}}}}","result":"updated"}}'
    option success_contains 'updated'

config rule 'home'
    option enabled '1'
    option provider 'custom'
    option source 'wan4'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'home'
    option ttl '300'
    option check_interval '60'
    option force_interval '3600'
    option retry_count '3'
    option retry_backoff '30'
"#,
            state_dir.display(),
            log_dir.display(),
            server.url("/lookup"),
            server.url("/update")
        ),
    )
    .unwrap();

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let run_output = Command::new(&cargo)
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .args(["rules", "run", "home"])
        .output()
        .unwrap();
    assert!(run_output.status.success(), "{run_output:?}");

    let logs_output = Command::new(&cargo)
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .args(["logs", "home"])
        .output()
        .unwrap();

    assert!(logs_output.status.success(), "{logs_output:?}");
    let stdout = String::from_utf8_lossy(&logs_output.stdout);
    assert!(stdout.contains("\"ok\":true"), "stdout was: {stdout}");
    assert!(stdout.contains("\"entries\":"), "stdout was: {stdout}");
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].path, "/update");
}

#[test]
fn qddnsctl_logs_rejects_invalid_scope_with_dynamic_log_dir() {
    let temp = TempDir::new("qddns-cli-test");
    let config_path = temp.path().join("qddns.conf");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("custom-logs");
    fs::create_dir_all(&state_dir).unwrap();
    fs::create_dir_all(&log_dir).unwrap();
    fs::write(
        &config_path,
        format!(
            r#"
config qddns 'main'
    option enabled '1'
    option state_dir '{}'
    option log_dir '{}'

config source 'wan4'
    option type 'local_addr'
    option address '198.51.100.44'

config provider 'custom'
    option type 'custom_http'
    option lookup_url 'http://127.0.0.1/unused-lookup'
    option url 'http://127.0.0.1/unused-update'
    option method 'POST'
    option body_template '{{"ip":"{{{{ip}}}}","result":"updated"}}'
    option success_contains 'updated'

config rule 'home'
    option enabled '1'
    option provider 'custom'
    option source 'wan4'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'home'
"#,
            state_dir.display(),
            log_dir.display()
        ),
    )
    .unwrap();

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let logs_output = Command::new(&cargo)
        .args(["run", "--quiet", "--bin", "qddnsctl", "--", "--config"])
        .arg(config_path.as_os_str())
        .args(["logs", "../system"])
        .output()
        .unwrap();

    assert!(!logs_output.status.success(), "{logs_output:?}");
    let stderr = String::from_utf8_lossy(&logs_output.stderr);
    assert!(stderr.contains("invalid log scope"), "stderr was: {stderr}");
}
