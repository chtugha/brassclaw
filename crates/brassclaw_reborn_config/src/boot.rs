use crate::{RebornConfigError, RebornHome};

/// Fully resolved boot configuration for the standalone Reborn binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebornBootConfig {
    home: RebornHome,
}

impl RebornBootConfig {
    pub fn new(home: RebornHome) -> Self {
        Self { home }
    }

    pub fn resolve_from_env() -> Result<Self, RebornConfigError> {
        let home = RebornHome::resolve_from_env()?;
        Ok(Self { home })
    }

    pub fn resolve_from_env_parts(
        reborn_home: Option<std::ffi::OsString>,
        home: Option<std::ffi::OsString>,
        userprofile: Option<std::ffi::OsString>,
    ) -> Result<Self, RebornConfigError> {
        let home = RebornHome::resolve_from_env_parts(reborn_home, home, userprofile)?;
        Ok(Self { home })
    }

    pub fn home(&self) -> &RebornHome {
        &self.home
    }

    pub fn into_parts(self) -> RebornHome {
        self.home
    }
}
