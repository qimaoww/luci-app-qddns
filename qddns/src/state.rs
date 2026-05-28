use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::config::Config;
use crate::error::{Error, Result};
use serde_json::{json, Map, Value};

pub type StateText = String;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuleState {
    pub status: RuleStatus,
    pub current_ip: Option<StateText>,
    pub remote_ip: Option<StateText>,
    pub last_result: Option<RuleResult>,
    pub last_error: Option<StateText>,
    pub last_update: Option<u64>,
    pub last_check: Option<u64>,
    pub next_run: Option<u64>,
    pub retry_attempts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleStatus {
    #[default]
    Pending,
    Success,
    Failed,
    Skipped,
    Noop,
}

impl RuleStatus {
    pub fn parse(value: &str) -> Self {
        match value {
            "success" => RuleStatus::Success,
            "error" | "failed" => RuleStatus::Failed,
            "skipped" => RuleStatus::Skipped,
            "noop" => RuleStatus::Noop,
            _ => RuleStatus::Pending,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RuleStatus::Pending => "idle",
            RuleStatus::Success => "success",
            RuleStatus::Failed => "error",
            RuleStatus::Skipped => "skipped",
            RuleStatus::Noop => "noop",
        }
    }
}

impl std::fmt::Display for RuleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<&str> for RuleStatus {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl From<&str> for RuleStatus {
    fn from(value: &str) -> Self {
        RuleStatus::parse(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleResult {
    Updated,
    Unchanged,
    Error,
    Skipped,
    Noop,
}

impl RuleResult {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "updated" => Some(RuleResult::Updated),
            "unchanged" => Some(RuleResult::Unchanged),
            "error" => Some(RuleResult::Error),
            "skipped" => Some(RuleResult::Skipped),
            "noop" => Some(RuleResult::Noop),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RuleResult::Updated => "updated",
            RuleResult::Unchanged => "unchanged",
            RuleResult::Error => "error",
            RuleResult::Skipped => "skipped",
            RuleResult::Noop => "noop",
        }
    }
}

impl std::fmt::Display for RuleResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<&str> for RuleResult {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl From<&str> for RuleResult {
    fn from(value: &str) -> Self {
        RuleResult::parse(value).expect("valid rule result")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeState {
    pub daemon_running: bool,
    pub updated_at: Option<u64>,
    pub rules: BTreeMap<String, RuleState>,
}

const STATE_DIR_MODE: u32 = 0o750;
const STATE_FILE_MODE: u32 = 0o640;

#[derive(Debug, Clone)]
pub struct StateStore {
    state_dir: PathBuf,
}

impl StateStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            state_dir: path.as_ref().to_path_buf(),
        }
    }

    pub fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.state_dir)?;
        set_dir_mode(&self.state_dir, STATE_DIR_MODE)
    }

    pub fn read_runtime(&self) -> Result<RuntimeState> {
        let path = self.runtime_path();
        if !path.exists() {
            return Ok(RuntimeState::default());
        }
        let text = fs::read_to_string(&path)?;
        parse_runtime_state(&text)
            .map_err(|err| Error::new(format!("corrupt runtime state '{}': {err}", path.display())))
    }

    pub fn write_runtime(&self, runtime: &RuntimeState) -> Result<()> {
        self.ensure_dir()?;
        let _lock = self.acquire_lock()?;
        let target = self.runtime_path();
        let tmp = self.temp_path();
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        set_file_mode(&file, STATE_FILE_MODE)?;
        file.write_all(serialize_runtime_state(runtime).as_bytes())
            .map_err(|err| Error::new(err.to_string()))?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, &target)?;
        sync_dir(&self.state_dir)?;
        Ok(())
    }

    pub fn write_daemon_marker(&self, enabled: bool) -> Result<()> {
        self.ensure_dir()?;
        let _lock = self.acquire_lock()?;
        let target = self.marker_path();
        let tmp = self.temp_marker_path();
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        set_file_mode(&file, STATE_FILE_MODE)?;
        writeln!(file, "running={}", if enabled { 1 } else { 0 })
            .map_err(|err| Error::new(err.to_string()))?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, &target)?;
        sync_dir(&self.state_dir)?;
        Ok(())
    }

    pub fn remove_daemon_marker(&self) -> Result<()> {
        match fs::remove_file(self.marker_path()) {
            Ok(()) => {
                sync_dir(&self.state_dir)?;
                Ok(())
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(Error::new(err.to_string())),
        }
    }

    fn runtime_path(&self) -> PathBuf {
        self.state_dir.join("runtime.state")
    }

    fn marker_path(&self) -> PathBuf {
        self.state_dir.join("daemon.status")
    }

    fn lock_path(&self) -> PathBuf {
        self.state_dir.join(".runtime.state.lock")
    }

    fn temp_path(&self) -> PathBuf {
        self.state_dir
            .join(format!(".runtime.state.{}.tmp", unique_suffix()))
    }

    fn temp_marker_path(&self) -> PathBuf {
        self.state_dir
            .join(format!(".daemon.status.{}.tmp", unique_suffix()))
    }

    fn acquire_lock(&self) -> Result<StateLock> {
        let path = self.lock_path();
        for _ in 0..500 {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok(StateLock { path, _file: file }),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(err) => return Err(Error::new(err.to_string())),
            }
        }
        Err(Error::new(format!(
            "timed out waiting for state lock '{}'",
            path.display()
        )))
    }
}

#[derive(Debug)]
struct StateLock {
    path: PathBuf,
    _file: File,
}

impl Drop for StateLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}", std::process::id())
}

#[cfg(unix)]
fn set_dir_mode(path: &Path, mode: u32) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_dir_mode(_path: &Path, _mode: u32) -> Result<()> {
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

fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

pub fn default_rule_state() -> RuleState {
    RuleState {
        status: RuleStatus::Pending,
        current_ip: None,
        remote_ip: None,
        last_result: None,
        last_error: None,
        last_update: None,
        last_check: None,
        next_run: None,
        retry_attempts: 0,
    }
}

pub fn rule_state_json(rule: &RuleState) -> Value {
    json!({
        "status": rule.status.as_str(),
        "current_ip": rule.current_ip.as_deref(),
        "remote_ip": rule.remote_ip.as_deref(),
        "last_result": rule.last_result.map(|result| result.as_str()),
        "last_error": rule.last_error.as_deref(),
        "last_update": rule.last_update,
        "last_check": rule.last_check,
        "next_run": rule.next_run,
        "retry_attempts": rule.retry_attempts,
    })
}

pub fn rule_state_with_id_json(id: &str, rule: &RuleState) -> Value {
    let mut map = Map::new();
    map.insert("id".into(), json!(id));

    if let Value::Object(fields) = rule_state_json(rule) {
        for (key, value) in fields {
            map.insert(key, value);
        }
    }

    Value::Object(map)
}

pub fn rule_status_json(id: &str, rule: &RuleState) -> Value {
    rule_status_with_runtime_json(id, false, rule)
}

pub fn rule_status_with_runtime_json(id: &str, running: bool, rule: &RuleState) -> Value {
    let mut map = Map::new();
    map.insert("ok".into(), json!(true));
    map.insert("id".into(), json!(id));
    map.insert("running".into(), json!(running));

    if let Value::Object(fields) = rule_state_json(rule) {
        for (key, value) in fields {
            map.insert(key, value);
        }
    }

    Value::Object(map)
}

pub fn runtime_rule_state(config: &Config, runtime: &RuntimeState, id: &str) -> Result<RuleState> {
    if !config.rules.contains_key(id) {
        return Err(Error::new(format!("missing rule '{id}'")));
    }

    Ok(runtime
        .rules
        .get(id)
        .cloned()
        .unwrap_or_else(default_rule_state))
}

pub fn runtime_rule_status_json(
    config: &Config,
    runtime: &RuntimeState,
    id: &str,
) -> Result<Value> {
    let rule = runtime_rule_state(config, runtime, id)?;
    Ok(rule_status_with_runtime_json(
        id,
        runtime.daemon_running,
        &rule,
    ))
}

pub fn prune_runtime_rules(config: &Config, runtime: &mut RuntimeState) -> bool {
    let before = runtime.rules.len();
    runtime
        .rules
        .retain(|name, _| config.rules.contains_key(name));
    runtime.rules.len() != before
}

pub fn runtime_status_json(config: &Config, runtime: &RuntimeState) -> Value {
    let enabled_rules = config.rules.values().filter(|rule| rule.enabled).count();
    let mut recent_items = runtime
        .rules
        .iter()
        .filter(|(name, _)| config.rules.contains_key(*name))
        .collect::<Vec<_>>();
    recent_items.sort_by(|(left_name, left_state), (right_name, right_state)| {
        match (rule_recent_time(left_state), rule_recent_time(right_state)) {
            (Some(left_time), Some(right_time)) => right_time
                .cmp(&left_time)
                .then_with(|| left_name.cmp(right_name)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left_name.cmp(right_name),
        }
    });
    let recent_results = recent_items
        .into_iter()
        .take(8)
        .map(|(name, state)| rule_state_with_id_json(name, state))
        .collect::<Vec<_>>();
    let rule_states = Value::Object(
        runtime
            .rules
            .iter()
            .filter(|(name, _)| config.rules.contains_key(*name))
            .map(|(name, state)| (name.clone(), rule_state_json(state)))
            .collect::<Map<_, _>>(),
    );

    json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "enabled": config.main.enabled,
        "running": runtime.daemon_running,
        "sources": config.sources.len(),
        "providers": config.providers.len(),
        "rules": config.rules.len(),
        "enabled_rules": enabled_rules,
        "updated_at": runtime.updated_at,
        "recent_results": recent_results,
        "rule_states": rule_states,
    })
}

fn rule_recent_time(rule: &RuleState) -> Option<u64> {
    match (rule.last_update, rule.last_check) {
        (Some(update), Some(check)) => Some(update.max(check)),
        (Some(update), None) => Some(update),
        (None, Some(check)) => Some(check),
        (None, None) => None,
    }
}

pub fn serialize_runtime_state(state: &RuntimeState) -> String {
    let mut root = Map::new();
    root.insert("daemon_running".into(), json!(state.daemon_running));
    root.insert("updated_at".into(), json!(state.updated_at));

    let mut rules = Map::new();
    for (name, rule) in &state.rules {
        rules.insert(name.clone(), rule_state_json(rule));
    }
    root.insert("rules".into(), Value::Object(rules));

    serde_json::to_string(&Value::Object(root)).expect("runtime state is valid JSON")
}

pub fn parse_runtime_state(input: &str) -> Result<RuntimeState> {
    let root: Value = serde_json::from_str(input)
        .map_err(|_| Error::new("runtime state root must be valid JSON"))?;
    let obj = root
        .as_object()
        .ok_or_else(|| Error::new("runtime state root must be object"))?;

    let mut runtime = RuntimeState {
        daemon_running: obj
            .get("daemon_running")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        updated_at: obj.get("updated_at").and_then(json_to_opt_u64),
        rules: BTreeMap::new(),
    };

    if let Some(rule_obj) = obj.get("rules").and_then(Value::as_object) {
        for (name, value) in rule_obj {
            runtime
                .rules
                .insert(name.clone(), json_to_rule_state(value)?);
        }
    }

    Ok(runtime)
}

fn json_to_rule_state(value: &Value) -> Result<RuleState> {
    let obj = value
        .as_object()
        .ok_or_else(|| Error::new("rule state must be object"))?;
    Ok(RuleState {
        status: obj
            .get("status")
            .and_then(Value::as_str)
            .map(RuleStatus::parse)
            .unwrap_or_default(),
        current_ip: obj.get("current_ip").and_then(json_to_opt_string),
        remote_ip: obj.get("remote_ip").and_then(json_to_opt_string),
        last_result: obj
            .get("last_result")
            .and_then(json_to_opt_string)
            .and_then(|value| RuleResult::parse(&value)),
        last_error: obj.get("last_error").and_then(json_to_opt_string),
        last_update: obj.get("last_update").and_then(json_to_opt_u64),
        last_check: obj.get("last_check").and_then(json_to_opt_u64),
        next_run: obj.get("next_run").and_then(json_to_opt_u64),
        retry_attempts: obj
            .get("retry_attempts")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0),
    })
}

fn json_to_opt_string(value: &Value) -> Option<StateText> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        _ => None,
    }
}

fn json_to_opt_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Null => None,
        Value::Number(number) => number.as_u64(),
        _ => None,
    }
}
