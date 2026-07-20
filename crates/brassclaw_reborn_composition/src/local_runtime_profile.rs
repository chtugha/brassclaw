use std::path::PathBuf;

use brassclaw_host_api::runtime_policy::{DeploymentMode, RuntimeProfile};
use brassclaw_runtime_policy::{EffectiveRuntimePolicy as ResolvedRuntimePolicy, ResolveError};
use thiserror::Error;

use crate::{RebornBuildInput, RebornCompositionProfile};

#[derive(Debug, Error)]
pub enum RebornLocalRuntimeProfileError {
    #[error("failed to resolve local runtime policy: {0}")]
    Policy(#[from] ResolveError),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RebornLocalRuntimeProfileOptions {
    pub confirm_host_access: bool,
}

/// Build the local runtime substrate input and its matching runtime policy from
/// one profile mapping, so yolo policy and process behavior cannot drift.
pub fn local_runtime_build_input(
    profile: RebornCompositionProfile,
    owner_id: impl Into<String>,
    root: PathBuf,
) -> Result<RebornBuildInput, RebornLocalRuntimeProfileError> {
    local_runtime_build_input_with_options(
        profile,
        owner_id,
        root,
        RebornLocalRuntimeProfileOptions::default(),
    )
}

/// Build the local runtime substrate input while applying local-only operator
/// confirmations such as trusted host access.
pub fn local_runtime_build_input_with_options(
    profile: RebornCompositionProfile,
    owner_id: impl Into<String>,
    root: PathBuf,
    options: RebornLocalRuntimeProfileOptions,
) -> Result<RebornBuildInput, RebornLocalRuntimeProfileError> {
    let runtime_profile = composition_to_runtime_profile(profile);
    let policy = local_runtime_policy(runtime_profile, options)?;
    Ok(
        RebornBuildInput::local_dev_with_profile(profile, owner_id, root)
            .with_runtime_policy(policy),
    )
}

/// Resolved policy for the standalone local development runtime profile.
pub fn local_dev_runtime_policy() -> Result<ResolvedRuntimePolicy, ResolveError> {
    local_runtime_policy(
        RuntimeProfile::LocalDev,
        RebornLocalRuntimeProfileOptions::default(),
    )
    .map_err(|e| match e {
        RebornLocalRuntimeProfileError::Policy(re) => re,
    })
}

/// Resolved policy for trusted single-user local development with inherited
/// host environment access.
pub fn local_dev_yolo_runtime_policy(
    confirm_host_access: bool,
) -> Result<ResolvedRuntimePolicy, ResolveError> {
    local_runtime_policy(
        RuntimeProfile::LocalYolo,
        RebornLocalRuntimeProfileOptions {
            confirm_host_access,
        },
    )
    .map_err(|e| match e {
        RebornLocalRuntimeProfileError::Policy(re) => re,
    })
}

/// Map a local `RebornCompositionProfile` to the matching `RuntimeProfile`.
fn composition_to_runtime_profile(profile: RebornCompositionProfile) -> RuntimeProfile {
    match profile {
        RebornCompositionProfile::LocalDev | RebornCompositionProfile::Disabled => {
            RuntimeProfile::LocalDev
        }
        RebornCompositionProfile::LocalDevYolo => RuntimeProfile::LocalYolo,
    }
}

fn local_runtime_policy(
    runtime_profile: RuntimeProfile,
    options: RebornLocalRuntimeProfileOptions,
) -> Result<ResolvedRuntimePolicy, RebornLocalRuntimeProfileError> {
    let request = brassclaw_runtime_policy::ResolveRequest {
        yolo_disclosure_acknowledged: options.confirm_host_access,
        ..brassclaw_runtime_policy::ResolveRequest::new(
            DeploymentMode::LocalSingleUser,
            runtime_profile,
        )
    };
    Ok(brassclaw_runtime_policy::resolve(request)?)
}
