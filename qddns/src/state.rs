use std::collections::BTreeMap;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::json::{self, JsonValue};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuleState {
    pub status: String,
    pub current_ip: Option<String>,
    pub remote_ip: Option<String>,
    pub last_result: Option<String>,
    pub last_error: Option<String>,
    pub last_update: Option<u64>,
    pub last_check: Option<u64>,
    pub next_run: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeState {
    pub daemon_running: bool,
    pub updated_at: Option<u64>,
    pub rules: BTreeMap<String, RuleState>,
}

pub fn default_rule_state() -> RuleState {
    RuleState {
        status: "idle".into(),
        current_ip: None,
        remote_ip: None,
        last_result: None,
        last_error: None,
        last_update: None,
        last_check: None,
        next_run: None,
    }
}

pub fn rule_state_json(rule: &RuleState) -> JsonValue {
    let mut map = BTreeMap::new();
    map.insert("status".into(), JsonValue::String(rule.status.clone()));
    map.insert(
        "current_ip".into(),
        option_string_value(rule.current_ip.as_deref()),
    );
    map.insert(
        "remote_ip".into(),
        option_string_value(rule.remote_ip.as_deref()),
    );
    map.insert(
        "last_result".into(),
        option_string_value(rule.last_result.as_deref()),
    );
    map.insert(
        "last_error".into(),
        option_string_value(rule.last_error.as_deref()),
    );
    map.insert("last_update".into(), option_u64_value(rule.last_update));
    map.insert("last_check".into(), option_u64_value(rule.last_check));
    map.insert("next_run".into(), option_u64_value(rule.next_run));
    JsonValue::Object(map)
}

pub fn rule_state_with_id_json(id: &str, rule: &RuleState) -> JsonValue {
    let mut map = BTreeMap::new();
    map.insert("id".into(), JsonValue::String(id.to_string()));

    if let JsonValue::Object(fields) = rule_state_json(rule) {
        for (key, value) in fields {
            map.insert(key, value);
        }
    }

    JsonValue::Object(map)
}

pub fn rule_status_json(id: &str, rule: &RuleState) -> JsonValue {
    rule_status_with_runtime_json(id, false, rule)
}

pub fn rule_status_with_runtime_json(id: &str, running: bool, rule: &RuleState) -> JsonValue {
    let mut map = BTreeMap::new();
    map.insert("ok".into(), JsonValue::Bool(true));
    map.insert("id".into(), JsonValue::String(id.to_string()));
    map.insert("running".into(), JsonValue::Bool(running));

    if let JsonValue::Object(fields) = rule_state_json(rule) {
        for (key, value) in fields {
            map.insert(key, value);
        }
    }

    JsonValue::Object(map)
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

pub fn runtime_rule_status_json(config: &Config, runtime: &RuntimeState, id: &str) -> Result<JsonValue> {
    let rule = runtime_rule_state(config, runtime, id)?;
    Ok(rule_status_with_runtime_json(
        id,
        runtime.daemon_running,
        &rule,
    ))
}

pub fn runtime_status_json(config: &Config, runtime: &RuntimeState) -> JsonValue {
    let enabled_rules = config.rules.values().filter(|rule| rule.enabled).count();
    let recent_results = runtime
        .rules
        .iter()
        .take(8)
        .map(|(name, state)| rule_state_with_id_json(name, state))
        .collect::<Vec<_>>();
    let rule_states = JsonValue::Object(
        runtime
            .rules
            .iter()
            .map(|(name, state)| (name.clone(), rule_state_json(state)))
            .collect::<BTreeMap<_, _>>(),
    );

    JsonValue::Object(BTreeMap::from([
        ("ok".into(), JsonValue::Bool(true)),
        ("enabled".into(), JsonValue::Bool(config.main.enabled)),
        ("running".into(), JsonValue::Bool(runtime.daemon_running)),
        (
            "sources".into(),
            JsonValue::Number(config.sources.len().to_string()),
        ),
        (
            "providers".into(),
            JsonValue::Number(config.providers.len().to_string()),
        ),
        ("rules".into(), JsonValue::Number(config.rules.len().to_string())),
        (
            "enabled_rules".into(),
            JsonValue::Number(enabled_rules.to_string()),
        ),
        ("updated_at".into(), option_u64_value(runtime.updated_at)),
        ("recent_results".into(), JsonValue::Array(recent_results)),
        ("rule_states".into(), rule_states),
    ]))
}

pub fn serialize_runtime_state(state: &RuntimeState) -> String {
    let mut root = BTreeMap::new();
    root.insert("daemon_running".into(), JsonValue::Bool(state.daemon_running));
    root.insert(
        "updated_at".into(),
        option_u64_value(state.updated_at),
    );

    let mut rules = BTreeMap::new();
    for (name, rule) in &state.rules {
        rules.insert(name.clone(), rule_state_json(rule));
    }
    root.insert("rules".into(), JsonValue::Object(rules));

    json::stringify(&JsonValue::Object(root))
}

pub fn parse_runtime_state(input: &str) -> Result<RuntimeState> {
    let root = json::parse(input)?;
    let obj = root
        .as_object()
        .ok_or_else(|| Error::new("runtime state root must be object"))?;

    let mut runtime = RuntimeState {
        daemon_running: obj
            .get("daemon_running")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false),
        updated_at: obj.get("updated_at").and_then(json_to_opt_u64),
        rules: BTreeMap::new(),
    };

    if let Some(rule_obj) = obj.get("rules").and_then(JsonValue::as_object) {
        for (name, value) in rule_obj {
            runtime.rules.insert(name.clone(), json_to_rule_state(value)?);
        }
    }

    Ok(runtime)
}

fn json_to_rule_state(value: &JsonValue) -> Result<RuleState> {
    let obj = value
        .as_object()
        .ok_or_else(|| Error::new("rule state must be object"))?;
    Ok(RuleState {
        status: obj
            .get("status")
            .and_then(JsonValue::as_str)
            .unwrap_or("idle")
            .to_string(),
        current_ip: obj.get("current_ip").and_then(json_to_opt_string),
        remote_ip: obj.get("remote_ip").and_then(json_to_opt_string),
        last_result: obj.get("last_result").and_then(json_to_opt_string),
        last_error: obj.get("last_error").and_then(json_to_opt_string),
        last_update: obj.get("last_update").and_then(json_to_opt_u64),
        last_check: obj.get("last_check").and_then(json_to_opt_u64),
        next_run: obj.get("next_run").and_then(json_to_opt_u64),
    })
}

fn json_to_opt_string(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::Null => None,
        JsonValue::String(text) => Some(text.clone()),
        _ => None,
    }
}

fn json_to_opt_u64(value: &JsonValue) -> Option<u64> {
    match value {
        JsonValue::Null => None,
        JsonValue::Number(number) => number.parse::<u64>().ok(),
        _ => None,
    }
}

fn option_string_value(value: Option<&str>) -> JsonValue {
    if let Some(text) = value {
        JsonValue::String(text.to_string())
    } else {
        JsonValue::Null
    }
}

fn option_u64_value(value: Option<u64>) -> JsonValue {
    if let Some(number) = value {
        JsonValue::Number(number.to_string())
    } else {
        JsonValue::Null
    }
}
