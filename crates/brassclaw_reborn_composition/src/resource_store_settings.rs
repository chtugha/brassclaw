//! The actual PostgreSQL governor and gate store share this host-owned source.
//! Bootstrap uses the selected startup revision. Live reads use only the last
//! acknowledged watch value, including when failed-VM accounting must settle.
use brassclaw_resources::{PgResourceStoreSettings, PgResourceStoreSettingsSource};
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

pub(crate) struct AcknowledgedResourceStoreSettings {
    state: RwLock<State>,
}
enum State {
    Unbound,
    Bootstrap(PgResourceStoreSettings),
    Live(Arc<dyn PgResourceStoreSettingsSource>),
}
impl AcknowledgedResourceStoreSettings {
    pub(crate) fn new() -> Self {
        Self {
            state: RwLock::new(State::Unbound),
        }
    }
    pub(crate) fn prepare_bootstrap(
        &self,
        revision: u64,
        timeout_millis: u64,
    ) -> Result<(), &'static str> {
        let settings = PgResourceStoreSettings {
            revision,
            timeout: Duration::from_millis(timeout_millis),
        }
        .validate()?;
        let mut state = self
            .state
            .write()
            .map_err(|_| "resource-store settings unavailable")?;
        if !matches!(*state, State::Unbound) {
            return Err("resource-store bootstrap already bound");
        }
        *state = State::Bootstrap(settings);
        Ok(())
    }
    pub(crate) fn bind(
        &self,
        source: Arc<dyn PgResourceStoreSettingsSource>,
    ) -> Result<(), &'static str> {
        let settings = source.snapshot()?.validate()?;
        let mut state = self
            .state
            .write()
            .map_err(|_| "resource-store settings unavailable")?;
        let State::Bootstrap(bootstrap) = &*state else {
            return Err("resource-store bootstrap missing or already bound");
        };
        if settings.revision < bootstrap.revision
            || (settings.revision == bootstrap.revision && settings != *bootstrap)
        {
            return Err("resource-store settings revision conflict");
        }
        *state = State::Live(source);
        Ok(())
    }
}
impl PgResourceStoreSettingsSource for AcknowledgedResourceStoreSettings {
    fn snapshot(&self) -> Result<PgResourceStoreSettings, &'static str> {
        let source = {
            let state = self
                .state
                .read()
                .map_err(|_| "resource-store settings unavailable")?;
            match &*state {
                State::Unbound => return Err("resource-store settings not bound"),
                State::Bootstrap(settings) => return settings.validate(),
                State::Live(source) => source.clone(),
            }
        };
        source.snapshot()?.validate()
    }
}
