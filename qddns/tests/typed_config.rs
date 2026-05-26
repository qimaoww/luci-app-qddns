use qddns::config::Config;

#[test]
fn typed_config_rejects_core_string_dispatch() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source = std::fs::read_to_string(root.join("source.rs")).expect("read source.rs");
    let provider = read_provider_sources(&root.join("provider"));
    let runner = std::fs::read_to_string(root.join("runner.rs")).expect("read runner.rs");
    let state = std::fs::read_to_string(root.join("state.rs")).expect("read state.rs");

    let offenders = [
        (
            "source.rs",
            "source_type.as_str()",
            source.contains("source_type.as_str()"),
        ),
        (
            "provider.rs",
            "provider_type.as_str()",
            provider.contains("provider_type.as_str()"),
        ),
        (
            "runner.rs",
            "record_type.as_str()",
            runner.contains("record_type.as_str()"),
        ),
        (
            "state.rs",
            "status: String",
            state.contains("status: String"),
        ),
    ];
    let found = offenders
        .iter()
        .filter_map(|(file, pattern, matched)| matched.then_some(format!("{file}: {pattern}")))
        .collect::<Vec<_>>();

    assert!(
        found.is_empty(),
        "core still dispatches on strings: {}",
        found.join(", ")
    );
}

fn read_provider_sources(path: &std::path::Path) -> String {
    let mut source = String::new();
    for entry in std::fs::read_dir(path).expect("read provider module dir") {
        let entry = entry.expect("read provider module entry");
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            source.push_str(&std::fs::read_to_string(&path).expect("read provider module"));
            source.push('\n');
        }
    }
    source
}

#[test]
fn typed_config_accepts_all_supported_variants() {
    let config = Config::parse_uci(
        r#"
config qddns 'main'

config source 'local'
    option type 'local_addr'
    option address '192.0.2.10'

config source 'iface'
    option type 'interface'
    option family 'ipv6'
    option interface 'br-lan'

config source 'probe'
    option type 'public_probe'
    option family 'ipv4'
    option probe_url 'http://127.0.0.1/probe'

config source 'scripted'
    option type 'script'
    option family 'ipv6'
    option script '/usr/lib/qddns/source.sh'

config source 'duid'
    option type 'dhcpv6_duid'
    option duid '0001000130555374bcfce78c41cb'
    option iaid '6bcfce7'

config provider 'cf'
    option type 'cloudflare'
    option api_token 'token'

config provider 'dnspod'
    option type 'dnspod'
    option secret_id 'id'
    option secret_key 'key'

config provider 'aliyun'
    option type 'aliyun'
    option access_key_id 'id'
    option access_key_secret 'secret'

config provider 'custom'
    option type 'custom_http'
    option lookup_url 'http://127.0.0.1/lookup'
    option url 'http://127.0.0.1/update'

config rule 'a_record'
    option provider 'custom'
    option source 'local'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'www'

config rule 'aaaa_record'
    option provider 'cf'
    option source 'duid'
    option record_type 'AAAA'
    option zone 'example.com'
    option record_name 'host'
"#,
    )
    .expect("all variants parse");

    config.validate().expect("all variants validate");
    assert_eq!(config.sources.len(), 5);
    assert_eq!(config.providers.len(), 4);
    assert_eq!(config.rules.len(), 2);
}
