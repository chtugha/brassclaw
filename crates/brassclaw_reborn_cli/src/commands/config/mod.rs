use clap::{Args, Subcommand};

use crate::context::RebornCliContext;

mod crud;
mod init;
pub(crate) mod pg_lifecycle;

#[derive(Debug, Args)]
pub(crate) struct ConfigCommand {
    #[command(subcommand)]
    command: ConfigSubcommand,
}

#[derive(Debug, Subcommand)]
enum ConfigSubcommand {
    /// Show resolved Reborn configuration paths without creating state.
    Path(ConfigPathCommand),
    /// Run the first-run wizard, writing config to PostgreSQL.
    ///
    /// Detects `boot.initialized` in `brassclaw_config`; if already set,
    /// exits early (use --yes to overwrite). Non-interactive when stdin is
    /// not a TTY and `boot.initialized` is absent.
    Init(init::ConfigInitCommand),
    /// Read a single config key from the database.
    Get(crud::ConfigGetCommand),
    /// Write a config key→value pair to the database.
    Set(crud::ConfigSetCommand),
    /// Remove a config key from the database.
    Unset(crud::ConfigUnsetCommand),
    /// List config keys for a tenant (optional --section filter).
    List(crud::ConfigListCommand),
    /// Print all config as TOML (reconstructed from DB rows).
    ShowAll(crud::ConfigShowAllCommand),
    /// Export all config as TOML to stdout (for backup).
    Export(crud::ConfigExportCommand),
    /// Import config from TOML on stdin into the database.
    Import(crud::ConfigImportCommand),
}

#[derive(Debug, Args)]
struct ConfigPathCommand;

impl ConfigCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        match self.command {
            ConfigSubcommand::Path(command) => command.execute(context),
            ConfigSubcommand::Init(command) => command.execute(context),
            ConfigSubcommand::Get(command) => command.execute(context),
            ConfigSubcommand::Set(command) => command.execute(context),
            ConfigSubcommand::Unset(command) => command.execute(context),
            ConfigSubcommand::List(command) => command.execute(context),
            ConfigSubcommand::ShowAll(command) => command.execute(context),
            ConfigSubcommand::Export(command) => command.execute(context),
            ConfigSubcommand::Import(command) => command.execute(context),
        }
    }
}

impl ConfigPathCommand {
    fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        use brassclaw_reborn_config::RebornDoctorReport;

        let report = RebornDoctorReport::from_config(context.boot_config().clone());
        let home = context.boot_config().home();

        let config_path = home.config_file_path();
        let providers_path = home.providers_file_path();
        let exists = |path: &std::path::Path| {
            if path.exists() {
                "present"
            } else {
                "absent (optional; falls back to defaults)"
            }
        };

        println!("BrassClaw Reborn config path");
        println!("reborn_home: {}", report.home_path().display());
        println!("home_source: {}", report.home_source_label());
        println!("profile: {}", report.profile());
        println!(
            "config_file: {} ({})",
            config_path.display(),
            exists(&config_path)
        );
        println!(
            "providers: {} ({})",
            providers_path.display(),
            exists(&providers_path)
        );
        println!("v1_state: {}", report.v1_state());
        Ok(())
    }
}
