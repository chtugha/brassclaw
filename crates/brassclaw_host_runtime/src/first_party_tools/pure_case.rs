//! One native host-free original invocation; sequencing remains Recipe-owned.
use brassclaw_extensions::{CapabilityManifest, CapabilityVisibility, ExtensionError};
use brassclaw_host_api::{EffectKind, PermissionMode};

pub(super) fn manifest() -> Result<CapabilityManifest, ExtensionError> {
    let mut manifest = super::first_party_capability_manifest(
        crate::PURE_CASE_CAPABILITY_ID,
        "Invoke one sealed original Pure export in its existing task and return observed data after confirmed context release; no approval or pass verdict.",
        vec![EffectKind::ExecuteCode],
        PermissionMode::Allow,
        None,
    )?;
    manifest.visibility = CapabilityVisibility::HostInternal;
    Ok(manifest)
}

pub(super) async fn dispatch(
    request: &crate::FirstPartyCapabilityRequest,
) -> Result<crate::FirstPartyCapabilityResult, crate::FirstPartyCapabilityError> {
    if request
        .input
        .as_object()
        .is_none_or(|object| !object.is_empty())
    {
        return Err(super::operation_error());
    }
    let view = request
        .services
        .pure_case
        .as_ref()
        .ok_or_else(super::operation_error)?;
    let result = view.invoke(&request.scope).await?;
    if !result.output.is_string() || result.display_preview.is_some() {
        return Err(super::operation_error());
    }
    Ok(result)
}
