use brassclaw_reborn_config::RebornBootConfig;

/// Per-invocation context shared by Reborn CLI commands.
#[derive(Debug, Clone)]
pub(crate) struct RebornCliContext {
    boot_config: RebornBootConfig,
}

impl RebornCliContext {
    pub(crate) fn resolve_from_env() -> anyhow::Result<Self> {
        Ok(Self {
            boot_config: RebornBootConfig::resolve_from_env()?,
        })
    }

    pub(crate) fn boot_config(&self) -> &RebornBootConfig {
        &self.boot_config
    }

    /// Match the trusted installation namespace used by serve/run. A legacy
    /// namespace is accessible only when the operator names it explicitly;
    /// malformed configuration never silently selects a different inventory.
    pub(crate) fn storage_namespace(&self, explicit: Option<&str>) -> anyhow::Result<String> {
        let config = crate::runtime::read_config_file(&self.boot_config)?;
        let namespace = explicit
            .map(str::to_owned)
            .unwrap_or_else(|| crate::runtime::runtime_identity(config.as_ref()).tenant_id);
        brassclaw_reborn_composition::host_api::TenantId::new(&namespace)
            .map_err(|_| anyhow::anyhow!("invalid installation storage namespace"))?;
        Ok(namespace)
    }
}
