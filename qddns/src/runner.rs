use crate::config::{Config, ProviderConfig, RuleConfig, SourceConfig};
use crate::error::{Error, Result};
use crate::provider::{ProviderAdapter, SyncOutcome};
use crate::source::SourceResolution;
use crate::state::RuleState;

pub trait SourceAdapter: Send + Sync {
    fn resolve(&self, source: &SourceConfig) -> Result<SourceResolution>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub status: String,
    pub changed: bool,
    pub current_ip: String,
    pub remote_ip: Option<String>,
    pub detail: String,
}

pub fn validate_rule_family(rule: &RuleConfig, source: &SourceConfig) -> Result<()> {
    match (rule.record_type.as_str(), source.family.as_deref()) {
        ("AAAA", Some("ipv4")) => Err(Error::new(format!(
            "rule '{}' cannot bind AAAA to IPv4 source '{}'",
            rule.name, source.name
        ))),
        ("A", Some("ipv6")) => Err(Error::new(format!(
            "rule '{}' cannot bind A to IPv6 source '{}'",
            rule.name, source.name
        ))),
        _ => Ok(()),
    }
}

pub fn run_rule(
    config: &Config,
    rule_id: &str,
    source_adapter: &dyn SourceAdapter,
    provider_adapter: &dyn ProviderAdapter,
    prior_state: Option<&RuleState>,
    now_epoch: u64,
) -> Result<(RunReport, RuleState)> {
    let (rule, source, provider) = get_provider_and_source(config, rule_id)?;
    validate_rule_family(rule, source)?;

    let resolved = source_adapter.resolve(source)?;
    let current_ip = resolved.address.to_string();
    let remote = provider_adapter.fetch_record(provider, rule)?;
    let force = should_force_update(rule, prior_state, now_epoch);
    let matches = remote.address.as_deref() == Some(current_ip.as_str());

    let mut report = RunReport {
        status: "success".into(),
        changed: false,
        current_ip: current_ip.clone(),
        remote_ip: remote.address.clone(),
        detail: "checked".into(),
    };

    let mut state = RuleState {
        status: "success".into(),
        current_ip: Some(current_ip.clone()),
        remote_ip: remote.address.clone(),
        last_result: Some("unchanged".into()),
        last_error: None,
        last_update: prior_state.and_then(|s| s.last_update),
        last_check: Some(now_epoch),
        next_run: Some(now_epoch + rule.check_interval),
    };

    if matches && !force {
        report.detail = "remote record already matches source".into();
        return Ok((report, state));
    }

    let outcome = provider_adapter.update_record(provider, rule, &remote, &current_ip)?;
    report.changed = outcome.changed;
    apply_sync_outcome(&mut report, &mut state, &outcome, now_epoch);
    Ok((report, state))
}

pub fn should_force_update(rule: &RuleConfig, prior_state: Option<&RuleState>, now_epoch: u64) -> bool {
    match prior_state.and_then(|s| s.last_update) {
        Some(last) => now_epoch.saturating_sub(last) >= rule.force_interval,
        None => true,
    }
}

pub fn apply_sync_outcome(
    report: &mut RunReport,
    state: &mut RuleState,
    outcome: &SyncOutcome,
    now_epoch: u64,
) {
    report.status = "success".into();
    report.remote_ip = Some(outcome.remote_after.clone());
    report.detail = outcome.detail.clone();
    state.status = "success".into();
    state.remote_ip = Some(outcome.remote_after.clone());
    state.last_result = Some(if outcome.changed {
        "updated".into()
    } else {
        "unchanged".into()
    });
    state.last_error = None;
    state.last_update = Some(now_epoch);
}

pub fn get_provider_and_source<'a>(
    config: &'a Config,
    rule_id: &str,
) -> Result<(&'a RuleConfig, &'a SourceConfig, &'a ProviderConfig)> {
    let rule = config
        .rules
        .get(rule_id)
        .ok_or_else(|| Error::new(format!("missing rule '{rule_id}'")))?;
    let source = config.sources.get(&rule.source).ok_or_else(|| {
        Error::new(format!(
            "rule '{}' references missing source '{}'",
            rule.name, rule.source
        ))
    })?;
    let provider = config.providers.get(&rule.provider).ok_or_else(|| {
        Error::new(format!(
            "rule '{}' references missing provider '{}'",
            rule.name, rule.provider
        ))
    })?;
    Ok((rule, source, provider))
}
