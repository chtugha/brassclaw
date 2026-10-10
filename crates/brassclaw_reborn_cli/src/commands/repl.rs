use clap::Args;

use crate::context::RebornCliContext;
use crate::runtime::RuntimeInputOptions;

/// Start an interactive Reborn CLI session backed by the composed runtime.
#[derive(Debug, Args)]
pub(crate) struct ReplCommand {
    /// Confirm trusted-laptop host filesystem access for local-dev-yolo.
    #[arg(long = "confirm-host-access")]
    confirm_host_access: bool,

    /// Loopback MCP listener port. Zero lets the OS choose an instance-local port.
    #[arg(long = "mcp-port", default_value_t = 0)]
    mcp_port: u16,
}

impl ReplCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        crate::runtime::init_tracing();
        crate::runtime::execute(
            context,
            None,
            self.mcp_port,
            RuntimeInputOptions {
                confirm_host_access: self.confirm_host_access,
            },
        )
    }
}
