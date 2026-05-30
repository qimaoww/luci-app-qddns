use std::env;
use std::path::Path;

use crate::config::Config;
use crate::daemon;
use crate::error::{Error, Result};
use crate::state::{runtime_rule_status_json, runtime_status_json};
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cli {
    pub config: String,
    pub command: Commands,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Commands {
    Status,
    Validate,
    Logs { scope: String },
    Sources(SourceCommands),
    Rules(RuleCommands),
    Interfaces,
    Leases { mode: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCommands {
    List,
    Probe { id: String },
    Discover,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleCommands {
    List,
    Run { id: String },
    Test { id: String },
    Status { id: String },
}

impl Cli {
    pub fn parse_from<I, S>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into).collect::<Vec<_>>();
        if !args.is_empty() {
            args.remove(0);
        }

        let mut config = "/etc/config/qddns".to_string();
        let mut cleaned = Vec::new();
        let mut idx = 0;
        while idx < args.len() {
            if args[idx] == "--config" {
                let value = args
                    .get(idx + 1)
                    .cloned()
                    .ok_or_else(|| Error::new("missing value for --config"))?;
                config = value;
                idx += 2;
                continue;
            }
            cleaned.push(args[idx].clone());
            idx += 1;
        }

        if cleaned.is_empty() {
            return Err(Error::new("missing command"));
        }

        let command = match cleaned[0].as_str() {
            "status" => Commands::Status,
            "validate" => Commands::Validate,
            "logs" => Commands::Logs {
                scope: cleaned.get(1).cloned().unwrap_or_else(|| "system".into()),
            },
            "sources" => match cleaned.get(1).map(String::as_str) {
                Some("list") => Commands::Sources(SourceCommands::List),
                Some("probe") => Commands::Sources(SourceCommands::Probe {
                    id: cleaned
                        .get(2)
                        .cloned()
                        .ok_or_else(|| Error::new("missing source id"))?,
                }),
                Some("discover") => Commands::Sources(SourceCommands::Discover),
                _ => return Err(Error::new("unsupported sources subcommand")),
            },
            "interfaces" => Commands::Interfaces,
            "leases" => Commands::Leases {
                mode: cleaned.get(1).cloned().unwrap_or_else(|| "duid".into()),
            },
            "rules" => match cleaned.get(1).map(String::as_str) {
                Some("list") => Commands::Rules(RuleCommands::List),
                Some("run") => Commands::Rules(RuleCommands::Run {
                    id: cleaned
                        .get(2)
                        .cloned()
                        .ok_or_else(|| Error::new("missing rule id"))?,
                }),
                Some("test") => Commands::Rules(RuleCommands::Test {
                    id: cleaned
                        .get(2)
                        .cloned()
                        .ok_or_else(|| Error::new("missing rule id"))?,
                }),
                Some("status") => Commands::Rules(RuleCommands::Status {
                    id: cleaned
                        .get(2)
                        .cloned()
                        .ok_or_else(|| Error::new("missing rule id"))?,
                }),
                _ => return Err(Error::new("unsupported rules subcommand")),
            },
            _ => return Err(Error::new("unsupported command")),
        };

        Ok(Self { config, command })
    }
}

pub fn parse_from_env() -> Result<Cli> {
    Cli::parse_from(env::args())
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Status => {
            let config = Config::load_from_path(Path::new(&cli.config))?;
            let runtime = daemon::read_runtime_status(&cli.config).unwrap_or_default();
            println!("{}", runtime_status_json(&config, &runtime));
            Ok(())
        }
        Commands::Validate => {
            let config = Config::load_from_path(Path::new(&cli.config))?;
            config.validate()?;
            println!(
                "{}",
                json!({
                    "ok": true,
                    "sources": config.sources.len(),
                    "providers": config.providers.len(),
                    "rules": config.rules.len(),
                })
            );
            Ok(())
        }
        Commands::Sources(SourceCommands::List) => daemon::list_sources(&cli.config),
        Commands::Logs { scope } => daemon::print_logs(&cli.config, &scope),
        Commands::Sources(SourceCommands::Probe { id }) => daemon::probe_source(&cli.config, &id),
        Commands::Sources(SourceCommands::Discover) => daemon::discover_slaac(&cli.config),
        Commands::Interfaces => {
            let interfaces = crate::network::list_interfaces();
            println!("{}", json!({"ok": true, "interfaces": interfaces}));
            Ok(())
        }
        Commands::Leases { mode } => daemon::list_leases_cmd(&cli.config, &mode),
        Commands::Rules(RuleCommands::List) => daemon::list_rules(&cli.config),
        Commands::Rules(RuleCommands::Run { id }) | Commands::Rules(RuleCommands::Test { id }) => {
            daemon::run_rule_once(&cli.config, &id)
        }
        Commands::Rules(RuleCommands::Status { id }) => {
            let config = Config::load_from_path(Path::new(&cli.config))?;
            let runtime = daemon::read_runtime_status(&cli.config)?;
            println!("{}", runtime_rule_status_json(&config, &runtime, &id)?);
            Ok(())
        }
    }
}
