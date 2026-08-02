use brassclaw_reborn_config::RebornDoctorReport;
use clap::Args;

use crate::context::RebornCliContext;

#[derive(Debug, Args)]
pub(crate) struct DoctorCommand;

impl DoctorCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let report = RebornDoctorReport::from_config(context.boot_config().clone());

        // Surface profile mis-configuration early — same fail-closed guard as
        // `run` so operators can diagnose deployment issues before starting.
        crate::runtime::runtime_profile_from_env()?;

        println!("BrassClaw Reborn doctor");
        println!("reborn_home: {}", report.home_path().display());
        println!("home_source: {}", report.home_source_label());
        println!("v1_state: {}", report.v1_state());
        println!("driver_registry: initialized");
        Ok(())
    }
}
