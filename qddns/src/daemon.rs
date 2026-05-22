use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::config::{Config, ProviderConfig, RuleConfig};
use crate::error::{Error, Result};
use crate::json::{self, JsonValue};
use crate::logstore::{append_log, ensure_valid_log_scope, read_logs, LogEntry};
use crate::provider::{ProviderAdapter, RemoteRecord, ShellProviderAdapter, SyncOutcome};
use crate::runner::{run_rule, SourceAdapter};
use crate::source::{resolve_source, SourceResolution};
use crate::state::{parse_runtime_state, runtime_rule_state, serialize_runtime_state, RuleState, RuntimeState};

const STATE_DIR_MODE: u32 = 0o750;
const STATE_FILE_MODE: u32 = 0o640;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonOptions {
    pub config: String,
    pub once: bool,
}

pub fn run(options: DaemonOptions) -> Result<()> {
    let config = Config::load_from_path(Path::new(&options.config))?;
    config.validate()?;
    ensure_runtime_dir(&config.main.state_dir)?;
    ensure_runtime_dir(&config.main.log_dir)?;

    if options.once {
        let mut runtime = load_runtime_state(&config.main.state_dir).unwrap_or_default();
        runtime.daemon_running = false;
        runtime.updated_at = Some(unix_now());
        for rule_id in enabled_rule_ids(&config) {
            let _ = run_rule_once_with_runtime(&config, &mut runtime, &rule_id, &ShellProviderAdapter);
        }
        runtime.updated_at = Some(unix_now());
        write_runtime_state(&config.main.state_dir, &runtime)?;
        remove_daemon_marker(&config)?;
        return Ok(());
    }

    let mut runtime = load_runtime_state(&config.main.state_dir).unwrap_or_default();
    runtime.daemon_running = true;
    runtime.updated_at = Some(unix_now());
    write_runtime_state(&config.main.state_dir, &runtime)?;
    write_daemon_marker(&config)?;
    append_system_log(&config.main.log_dir, "daemon started")?;

    loop {
        let now = unix_now();
        let current = Config::load_from_path(Path::new(&options.config))?;
        current.validate()?;
        runtime.daemon_running = true;

        for rule_id in enabled_rule_ids(&current) {
            let should_run = runtime
                .rules
                .get(&rule_id)
                .and_then(|state| state.next_run)
                .map(|next| next <= now)
                .unwrap_or(true);
            if should_run {
                let _ = run_rule_once_with_runtime(&current, &mut runtime, &rule_id, &ShellProviderAdapter);
            }
        }

        runtime.updated_at = Some(now);
        write_runtime_state(&current.main.state_dir, &runtime)?;
        write_daemon_marker(&current)?;
        thread::sleep(Duration::from_secs(current.main.poll_interval.max(5)));
    }
}

pub fn list_sources(config_path: &str) -> Result<()> {
    let config = Config::load_from_path(Path::new(config_path))?;
    for (id, source) in config.sources {
        println!("{id}\t{}", source.source_type);
    }
    Ok(())
}

pub fn probe_source(config_path: &str, source_id: &str) -> Result<()> {
    let config = Config::load_from_path(Path::new(config_path))?;
    let source = config
        .sources
        .get(source_id)
        .ok_or_else(|| Error::new(format!("missing source '{source_id}'")))?;
    let resolved = resolve_source(source)?;
    println!(
        "{{\"ok\":true,\"source\":\"{}\",\"family\":\"{}\",\"address\":\"{}\",\"detail\":\"{}\"}}",
        source.name,
        resolved.family,
        resolved.address,
        escape_json(&resolved.detail)
    );
    Ok(())
}

pub fn list_rules(config_path: &str) -> Result<()> {
    let config = Config::load_from_path(Path::new(config_path))?;
    for (id, rule) in config.rules {
        println!("{id}\t{}\t{}\t{}", rule.record_name, rule.record_type, rule.source);
    }
    Ok(())
}

pub fn run_rule_once(config_path: &str, rule_id: &str) -> Result<()> {
    let config = Config::load_from_path(Path::new(config_path))?;
    config.validate()?;
    let mut runtime = load_runtime_state(&config.main.state_dir).unwrap_or_default();
    runtime.daemon_running = false;
    let result = run_rule_once_with_runtime(&config, &mut runtime, rule_id, &ShellProviderAdapter);
    runtime.daemon_running = false;
    runtime.updated_at = Some(unix_now());
    write_runtime_state(&config.main.state_dir, &runtime)?;
    remove_daemon_marker(&config)?;
    let report = result?;
    println!(
        "{{\"ok\":true,\"status\":\"{}\",\"changed\":{},\"current_ip\":\"{}\",\"remote_ip\":{},\"detail\":\"{}\"}}",
        report.status,
        if report.changed { "true" } else { "false" },
        escape_json(&report.current_ip),
        report
            .remote_ip
            .map(|value| format!("\"{}\"", escape_json(&value)))
            .unwrap_or_else(|| "null".into()),
        escape_json(&report.detail)
    );
    Ok(())
}

pub fn read_runtime_status(config_path: &str) -> Result<RuntimeState> {
    let config = Config::load_from_path(Path::new(config_path))?;
    config.validate()?;
    let runtime = load_runtime_state(&config.main.state_dir).unwrap_or_default();
    Ok(runtime)
}

pub fn read_rule_status(config_path: &str, rule_id: &str) -> Result<RuleState> {
    let config = Config::load_from_path(Path::new(config_path))?;
    config.validate()?;
    let runtime = load_runtime_state(&config.main.state_dir).unwrap_or_default();
    runtime_rule_state(&config, &runtime, rule_id)
}

pub fn read_rule_logs(config_path: &str, rule_id: &str, limit: usize) -> Result<Vec<LogEntry>> {
    let config = Config::load_from_path(Path::new(config_path))?;
    if !config.rules.contains_key(rule_id) {
        return Err(Error::new(format!("missing rule '{rule_id}'")));
    }
    read_logs(&config.main.log_dir, Some(rule_id), limit)
}

pub fn print_logs(config_path: &str, scope: &str) -> Result<()> {
    let config = Config::load_from_path(Path::new(config_path))?;
    ensure_valid_log_scope(scope)?;
    if scope != "system" && !config.rules.contains_key(scope) {
        return Err(Error::new(format!("missing log scope '{scope}'")));
    }
    let entries = read_logs(&config.main.log_dir, Some(scope), 200)?;
    let content = entries
        .iter()
        .map(|entry| format!("{}\t{}\t{}\t{}", entry.timestamp, entry.level, entry.scope, entry.message))
        .collect::<Vec<_>>()
        .join("\n");
    let items = entries
        .iter()
        .map(|entry| {
            JsonValue::Object(std::collections::BTreeMap::from([
                ("timestamp".into(), JsonValue::Number(entry.timestamp.to_string())),
                ("level".into(), JsonValue::String(entry.level.clone())),
                ("scope".into(), JsonValue::String(entry.scope.clone())),
                ("message".into(), JsonValue::String(entry.message.clone())),
            ]))
        })
        .collect::<Vec<_>>();

    println!(
        "{}",
        json::stringify(&JsonValue::Object(std::collections::BTreeMap::from([
            ("ok".into(), JsonValue::Bool(true)),
            ("scope".into(), JsonValue::String(scope.to_string())),
            ("content".into(), JsonValue::String(content)),
            ("entries".into(), JsonValue::Array(items)),
        ])))
    );
    Ok(())
}

fn run_rule_once_with_runtime(
    config: &Config,
    runtime: &mut RuntimeState,
    rule_id: &str,
    provider_adapter: &dyn ProviderAdapter,
) -> Result<crate::runner::RunReport> {
    let now = unix_now();
    let prior = runtime.rules.get(rule_id).cloned().unwrap_or_default();

    let result = run_rule(
        config,
        rule_id,
        &DefaultSourceAdapter,
        provider_adapter,
        Some(&prior),
        now,
    );

    match result {
        Ok((report, state)) => {
            runtime.rules.insert(rule_id.to_string(), state.clone());
            runtime.updated_at = Some(now);
            append_log(
                &config.main.log_dir,
                rule_id,
                &LogEntry {
                    timestamp: now,
                    level: "info".into(),
                    scope: rule_id.into(),
                    message: format!(
                        "{} current={} remote={} detail={}",
                        state.last_result.as_deref().unwrap_or("checked"),
                        state.current_ip.as_deref().unwrap_or("-"),
                        state.remote_ip.as_deref().unwrap_or("-"),
                        report.detail
                    ),
                },
            )?;
            Ok(report)
        }
        Err(err) => {
            let failed = RuleState {
                status: "error".into(),
                current_ip: prior.current_ip.clone(),
                remote_ip: prior.remote_ip.clone(),
                last_result: Some("error".into()),
                last_error: Some(err.to_string()),
                last_update: prior.last_update,
                last_check: Some(now),
                next_run: config
                    .rules
                    .get(rule_id)
                    .map(|rule| now + rule.retry_backoff)
                    .or(Some(now + config.main.poll_interval)),
            };
            runtime.rules.insert(rule_id.to_string(), failed);
            runtime.updated_at = Some(now);
            append_log(
                &config.main.log_dir,
                rule_id,
                &LogEntry {
                    timestamp: now,
                    level: "error".into(),
                    scope: rule_id.into(),
                    message: err.to_string(),
                },
            )?;
            Err(err)
        }
    }
}

struct DefaultSourceAdapter;

impl SourceAdapter for DefaultSourceAdapter {
    fn resolve(&self, source: &crate::config::SourceConfig) -> Result<SourceResolution> {
        resolve_source(source)
    }
}

#[derive(Debug)]
pub struct NoopProviderAdapter;

impl ProviderAdapter for NoopProviderAdapter {
    fn fetch_record(&self, _provider: &ProviderConfig, _rule: &RuleConfig) -> Result<RemoteRecord> {
        Ok(RemoteRecord {
            address: None,
            record_id: None,
            detail: "noop".into(),
        })
    }

    fn update_record(
        &self,
        _provider: &ProviderConfig,
        _rule: &RuleConfig,
        remote: &RemoteRecord,
        target_ip: &str,
    ) -> Result<SyncOutcome> {
        Ok(SyncOutcome {
            changed: remote.address.as_deref() != Some(target_ip),
            remote_before: remote.address.clone(),
            remote_after: target_ip.into(),
            detail: "noop update".into(),
        })
    }
}

fn enabled_rule_ids(config: &Config) -> Vec<String> {
    config
        .rules
        .iter()
        .filter_map(|(id, rule)| if rule.enabled { Some(id.clone()) } else { None })
        .collect()
}

fn write_daemon_marker(config: &Config) -> Result<()> {
    let marker = Path::new(&config.main.state_dir).join("daemon.status");
    let mut file = File::create(marker)?;
    set_file_mode(&file, STATE_FILE_MODE)?;
    writeln!(file, "running={}", if config.main.enabled { 1 } else { 0 })
        .map_err(|err| Error::new(err.to_string()))?;
    Ok(())
}

fn remove_daemon_marker(config: &Config) -> Result<()> {
    let marker = Path::new(&config.main.state_dir).join("daemon.status");
    match fs::remove_file(marker) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(Error::new(err.to_string())),
    }
}

fn append_system_log(log_dir: &str, message: &str) -> Result<()> {
    append_log(
        log_dir,
        "system",
        &LogEntry {
            timestamp: unix_now(),
            level: "info".into(),
            scope: "system".into(),
            message: message.into(),
        },
    )
}

fn load_runtime_state(state_dir: &str) -> Result<RuntimeState> {
    let path = state_path(state_dir);
    if !path.exists() {
        return Ok(RuntimeState::default());
    }
    let text = fs::read_to_string(path)?;
    parse_runtime_state(&text)
}

fn write_runtime_state(state_dir: &str, runtime: &RuntimeState) -> Result<()> {
    ensure_runtime_dir(state_dir)?;
    let path = state_path(state_dir);
    let mut file = File::create(path)?;
    set_file_mode(&file, STATE_FILE_MODE)?;
    file.write_all(serialize_runtime_state(runtime).as_bytes())
        .map_err(|err| Error::new(err.to_string()))?;
    Ok(())
}

fn state_path(state_dir: &str) -> PathBuf {
    Path::new(state_dir).join("runtime.state")
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn ensure_runtime_dir(path: &str) -> Result<()> {
    fs::create_dir_all(path)?;
    set_dir_mode(path, STATE_DIR_MODE)
}

#[cfg(unix)]
fn set_dir_mode(path: &str, mode: u32) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_dir_mode(_path: &str, _mode: u32) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_file_mode(file: &File, mode: u32) -> Result<()> {
    file.set_permissions(fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_file_mode(_file: &File, _mode: u32) -> Result<()> {
    Ok(())
}
