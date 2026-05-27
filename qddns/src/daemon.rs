use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::config::{Config, ProviderConfig, RuleConfig, SourceKind};
use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::logstore::{append_log, ensure_valid_log_scope, read_logs, LogEntry};
use crate::provider::{ProviderAdapter, RemoteRecord, ShellProviderAdapter, SyncOutcome};
use crate::runner::{run_rule, SourceAdapter};
use crate::source::{resolve_source_with_http, SourceResolution};
use crate::state::{
    runtime_rule_state, RuleResult, RuleState, RuleStatus, RuntimeState, StateStore,
};
use serde_json::json;

const STATE_DIR_MODE: u32 = 0o750;

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
        let provider_adapter = ShellProviderAdapter::new(config.main.timeout);
        let mut errors = Vec::new();
        for rule_id in enabled_rule_ids(&config) {
            if let Err(err) =
                run_rule_once_with_runtime(&config, &mut runtime, &rule_id, &provider_adapter)
            {
                errors.push(format!("{rule_id}: {err}"));
            }
        }
        runtime.updated_at = Some(unix_now());
        write_runtime_state(&config.main.state_dir, &runtime)?;
        remove_daemon_marker(&config)?;
        return if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::new(format!(
                "one or more rules failed: {}",
                errors.join("; ")
            )))
        };
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
        let provider_adapter = ShellProviderAdapter::new(current.main.timeout);

        for rule_id in enabled_rule_ids(&current) {
            let should_run = runtime
                .rules
                .get(&rule_id)
                .and_then(|state| state.next_run)
                .map(|next| next <= now)
                .unwrap_or(true);
            if should_run {
                let _ =
                    run_rule_once_with_runtime(&current, &mut runtime, &rule_id, &provider_adapter);
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
        println!("{id}\t{}", source.source_type());
    }
    Ok(())
}

pub fn probe_source(config_path: &str, source_id: &str) -> Result<()> {
    let config = Config::load_from_path(Path::new(config_path))?;
    let source = config
        .sources
        .get(source_id)
        .ok_or_else(|| Error::new(format!("missing source '{source_id}'")))?;
    if matches!(
        &source.kind,
        SourceKind::Script { .. } | SourceKind::PublicProbe { .. }
    ) {
        return Err(Error::new(format!(
            "probe not allowed for source type '{}'",
            source.source_type()
        )));
    }
    let http = HttpClient::from_timeout_secs(config.main.timeout);
    let resolved = resolve_source_with_http(source, &http)?;
    println!(
        "{}",
        json!({
            "ok": true,
            "source": source.name,
            "family": resolved.family,
            "address": resolved.address.to_string(),
            "detail": resolved.detail,
        })
    );
    Ok(())
}

pub fn list_rules(config_path: &str) -> Result<()> {
    let config = Config::load_from_path(Path::new(config_path))?;
    for (id, rule) in config.rules {
        println!(
            "{id}\t{}\t{}\t{}",
            rule.record_name, rule.record_type, rule.source
        );
    }
    Ok(())
}

pub fn run_rule_once(config_path: &str, rule_id: &str) -> Result<()> {
    let config = Config::load_from_path(Path::new(config_path))?;
    config.validate()?;
    let mut runtime = load_runtime_state(&config.main.state_dir).unwrap_or_default();
    let provider_adapter = ShellProviderAdapter::new(config.main.timeout);
    let result = run_rule_once_with_runtime(&config, &mut runtime, rule_id, &provider_adapter);
    runtime.updated_at = Some(unix_now());
    write_runtime_state(&config.main.state_dir, &runtime)?;
    let report = result?;
    println!(
        "{}",
        json!({
            "ok": true,
            "status": report.status.as_str(),
            "changed": report.changed,
            "current_ip": report.current_ip,
            "remote_ip": report.remote_ip,
            "detail": report.detail,
        })
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
        .map(|entry| {
            format!(
                "{}\t{}\t{}\t{}",
                entry.timestamp, entry.level, entry.scope, entry.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    println!(
        "{}",
        json!({
            "ok": true,
            "scope": scope,
            "content": content,
            "entries": entries.iter().map(|entry| {
                json!({
                    "timestamp": entry.timestamp,
                    "level": entry.level,
                    "scope": entry.scope,
                    "message": entry.message,
                })
            }).collect::<Vec<_>>(),
        })
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
    let source_adapter = DefaultSourceAdapter::new(config.main.timeout);

    let result = run_rule(
        config,
        rule_id,
        &source_adapter,
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
                        state
                            .last_result
                            .map(|result| result.as_str())
                            .unwrap_or("checked"),
                        state.current_ip.as_deref().unwrap_or("-"),
                        state.remote_ip.as_deref().unwrap_or("-"),
                        report.detail
                    ),
                },
            )?;
            Ok(report)
        }
        Err(err) => {
            let prior_retry_attempts = if prior.last_result == Some(RuleResult::Error) {
                prior.retry_attempts
            } else {
                0
            };
            let retry_attempts = prior_retry_attempts.saturating_add(1);
            let next_run = config.rules.get(rule_id).map(|rule| {
                let delay = if retry_attempts <= rule.retry_count {
                    rule.retry_backoff
                } else {
                    rule.check_interval
                };
                now + delay
            });
            let failed = RuleState {
                status: RuleStatus::Failed,
                current_ip: None,
                remote_ip: None,
                last_result: Some(RuleResult::Error),
                last_error: Some(err.to_string()),
                last_update: prior.last_update,
                last_check: Some(now),
                next_run: next_run.or(Some(now + config.main.poll_interval)),
                retry_attempts,
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

struct DefaultSourceAdapter {
    http: HttpClient,
}

impl DefaultSourceAdapter {
    fn new(timeout_secs: u64) -> Self {
        Self {
            http: HttpClient::from_timeout_secs(timeout_secs),
        }
    }
}

impl SourceAdapter for DefaultSourceAdapter {
    fn resolve(&self, source: &crate::config::SourceConfig) -> Result<SourceResolution> {
        resolve_source_with_http(source, &self.http)
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
    StateStore::new(&config.main.state_dir).write_daemon_marker(config.main.enabled)
}

fn remove_daemon_marker(config: &Config) -> Result<()> {
    StateStore::new(&config.main.state_dir).remove_daemon_marker()
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
    StateStore::new(state_dir).read_runtime()
}

fn write_runtime_state(state_dir: &str, runtime: &RuntimeState) -> Result<()> {
    StateStore::new(state_dir).write_runtime(runtime)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
