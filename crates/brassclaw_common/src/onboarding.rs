use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelOnboardingState {
    SetupRequired,
    AuthRequired,
    PairingRequired,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelOnboardingInfo {
    pub state: ChannelOnboardingState,
    #[serde(default)]
    pub requires_pairing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_next_step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pairing_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pairing_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_instructions: Option<String>,
}
