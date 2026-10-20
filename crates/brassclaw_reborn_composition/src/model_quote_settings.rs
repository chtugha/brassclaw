//! Bind the actual production accountant to the acknowledged settings source
//! before ingress. This slot never owns the VM or reads desired database rows.
use brassclaw_loop_support::{ModelQuoteSettings, ModelQuoteSettingsSource};
use std::sync::{Arc, OnceLock};

pub(crate) struct AcknowledgedModelQuoteSettings {
    source: OnceLock<Arc<dyn ModelQuoteSettingsSource>>,
}
impl AcknowledgedModelQuoteSettings {
    pub(crate) fn new() -> Self {
        Self {
            source: OnceLock::new(),
        }
    }
    pub(crate) fn bind(
        &self,
        source: Arc<dyn ModelQuoteSettingsSource>,
    ) -> Result<(), &'static str> {
        source.snapshot()?.validate()?;
        self.source
            .set(source)
            .map_err(|_| "model quote settings already bound")
    }
}
impl ModelQuoteSettingsSource for AcknowledgedModelQuoteSettings {
    fn snapshot(&self) -> Result<ModelQuoteSettings, &'static str> {
        self.source
            .get()
            .ok_or("model quote settings not bound")?
            .snapshot()?
            .validate()
    }
}
