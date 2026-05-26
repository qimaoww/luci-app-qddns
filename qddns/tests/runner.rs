use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};

use qddns::config::{
    AddressFamily, Config, ProviderConfig, ProviderKind, RuleConfig, SourceConfig, SourceKind,
};
use qddns::error::Result;
use qddns::provider::{ProviderAdapter, RemoteRecord, SyncOutcome};
use qddns::runner::{run_rule, should_force_update, SourceAdapter};
use qddns::source::SourceResolution;
use qddns::state::{RuleResult, RuleState};

struct StaticSource;

impl SourceAdapter for StaticSource {
    fn resolve(&self, _source: &SourceConfig) -> Result<SourceResolution> {
        Ok(SourceResolution {
            address: IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
            family: "ipv4".into(),
            detail: "fixture".into(),
        })
    }
}

#[derive(Default)]
struct MemoryProvider {
    remote: Option<String>,
    updates: std::sync::Mutex<Vec<String>>,
}

impl ProviderAdapter for MemoryProvider {
    fn fetch_record(&self, _provider: &ProviderConfig, _rule: &RuleConfig) -> Result<RemoteRecord> {
        Ok(RemoteRecord {
            address: self.remote.clone(),
            record_id: Some("record-1".into()),
            detail: "fixture".into(),
        })
    }

    fn update_record(
        &self,
        _provider: &ProviderConfig,
        _rule: &RuleConfig,
        _remote: &RemoteRecord,
        target_ip: &str,
    ) -> Result<SyncOutcome> {
        self.updates.lock().unwrap().push(target_ip.into());
        Ok(SyncOutcome {
            changed: true,
            remote_before: self.remote.clone(),
            remote_after: target_ip.into(),
            detail: "updated".into(),
        })
    }
}

fn fixture_config() -> Config {
    Config {
        main: Default::default(),
        sources: BTreeMap::from([(
            "wan4".into(),
            SourceConfig {
                name: "wan4".into(),
                kind: SourceKind::LocalAddr {
                    family: Some(AddressFamily::Ipv4),
                    address: Some("1.2.3.4".into()),
                },
            },
        )]),
        providers: BTreeMap::from([(
            "cf".into(),
            ProviderConfig {
                name: "cf".into(),
                kind: ProviderKind::Cloudflare {
                    api_token: Some("token".into()),
                },
            },
        )]),
        rules: BTreeMap::from([(
            "home".into(),
            RuleConfig {
                name: "home".into(),
                enabled: true,
                provider: "cf".into(),
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
    }
}

#[test]
fn initial_run_skips_update_when_remote_matches() {
    let config = fixture_config();
    let provider = MemoryProvider {
        remote: Some("1.2.3.4".into()),
        ..Default::default()
    };

    let (report, state) =
        run_rule(&config, "home", &StaticSource, &provider, None, 200).expect("run succeeds");

    assert!(!report.changed);
    assert_eq!(state.last_result, Some(RuleResult::Unchanged));
    assert!(provider.updates.lock().unwrap().is_empty());
}

#[test]
fn run_rule_skips_update_when_remote_matches_and_force_interval_not_reached() {
    let config = fixture_config();
    let provider = MemoryProvider {
        remote: Some("1.2.3.4".into()),
        ..Default::default()
    };
    let prior = RuleState {
        status: "success".into(),
        current_ip: Some("1.2.3.4".into()),
        remote_ip: Some("1.2.3.4".into()),
        last_result: Some("unchanged".into()),
        last_error: None,
        last_update: Some(100),
        last_check: Some(100),
        next_run: None,
        retry_attempts: 0,
    };

    let (report, state) = run_rule(&config, "home", &StaticSource, &provider, Some(&prior), 200)
        .expect("run succeeds");
    assert_eq!(report.status, "success");
    assert!(!report.changed);
    assert_eq!(state.last_result, Some(RuleResult::Unchanged));
    assert!(provider.updates.lock().unwrap().is_empty());
}

#[test]
fn run_rule_updates_when_remote_differs() {
    let config = fixture_config();
    let provider = MemoryProvider {
        remote: Some("8.8.8.8".into()),
        ..Default::default()
    };

    let (report, state) =
        run_rule(&config, "home", &StaticSource, &provider, None, 400).expect("run succeeds");
    assert_eq!(report.status, "success");
    assert!(report.changed);
    assert_eq!(state.remote_ip.as_deref(), Some("1.2.3.4"));
    assert_eq!(provider.updates.lock().unwrap().as_slice(), ["1.2.3.4"]);
}

#[test]
fn force_update_becomes_true_when_last_update_exceeds_force_interval() {
    let config = fixture_config();
    let rule = &config.rules["home"];
    let state = RuleState {
        last_update: Some(10),
        retry_attempts: 0,
        ..Default::default()
    };

    assert!(should_force_update(rule, Some(&state), 4000));
    assert!(!should_force_update(rule, Some(&state), 100));
}
