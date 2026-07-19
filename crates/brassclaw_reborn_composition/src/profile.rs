use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RebornCompositionProfile {
    #[default]
    Disabled,
    LocalDev,
    LocalDevYolo,
}

impl RebornCompositionProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::LocalDev => "local-dev",
            Self::LocalDevYolo => "local-dev-yolo",
        }
    }

    pub fn is_active(self) -> bool {
        self != Self::Disabled
    }

    pub fn to_event_store_profile(self) -> brassclaw_reborn_event_store::RebornProfile {
        // All active local-dev profiles use the LocalDev event store path.
        brassclaw_reborn_event_store::RebornProfile::LocalDev
    }
}

impl FromStr for RebornCompositionProfile {
    type Err = RebornCompositionProfileParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "disabled" => Ok(Self::Disabled),
            "local-dev" => Ok(Self::LocalDev),
            "local-dev-yolo" => Ok(Self::LocalDevYolo),
            _ => Err(RebornCompositionProfileParseError { value: normalized }),
        }
    }
}

impl std::fmt::Display for RebornCompositionProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid reborn composition profile '{value}'")]
pub struct RebornCompositionProfileParseError {
    value: String,
}
