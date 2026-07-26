use brassclaw_reborn_composition::host_api::RuntimeProfile;
use clap::{Args, Subcommand};

use crate::runtime::RUNTIME_PROFILE_ENV;

#[derive(Debug, Args)]
pub(crate) struct RuntimeProfileCommand {
    #[command(subcommand)]
    command: RuntimeProfileSubcommand,
}

#[derive(Debug, Subcommand)]
enum RuntimeProfileSubcommand {
    /// List supported runtime profiles.
    List(RuntimeProfileListCommand),
}

#[derive(Debug, Args)]
struct RuntimeProfileListCommand {
    /// Output profiles as JSON.
    #[arg(long)]
    json: bool,
}

/// All 12 `RuntimeProfile` variants in display order.
const ALL_RUNTIME_PROFILES: &[RuntimeProfile] = &[
    RuntimeProfile::SecureDefault,
    RuntimeProfile::LocalSafe,
    RuntimeProfile::LocalDev,
    RuntimeProfile::LocalYolo,
    RuntimeProfile::HostedSafe,
    RuntimeProfile::HostedDev,
    RuntimeProfile::HostedYoloTenantScoped,
    RuntimeProfile::EnterpriseSafe,
    RuntimeProfile::EnterpriseDev,
    RuntimeProfile::EnterpriseYoloDedicated,
    RuntimeProfile::Sandboxed,
    RuntimeProfile::Experiment,
];

impl RuntimeProfileCommand {
    pub(crate) fn execute(self) -> anyhow::Result<()> {
        match self.command {
            RuntimeProfileSubcommand::List(command) => command.execute(),
        }
    }
}

impl RuntimeProfileListCommand {
    fn execute(self) -> anyhow::Result<()> {
        let profiles = ALL_RUNTIME_PROFILES;
        let default_profile = RuntimeProfile::LocalDev;

        if self.json {
            let profiles = profiles.iter().map(|profile| {
                serde_json::json!({
                    "name": profile.as_str(),
                    "local": profile.is_local(),
                    "default": *profile == default_profile,
                })
            });
            println!(
                "{}",
                serde_json::json!({
                    "profiles": profiles.collect::<Vec<_>>(),
                    "selector": RUNTIME_PROFILE_ENV,
                })
            );
        } else {
            println!("BrassClaw runtime profiles");
            for profile in profiles {
                if *profile == default_profile {
                    println!("- {} (default)", profile.as_str());
                } else {
                    println!("- {}", profile.as_str());
                }
            }
            println!("Select with {}=<profile>", RUNTIME_PROFILE_ENV);
        }

        Ok(())
    }
}
