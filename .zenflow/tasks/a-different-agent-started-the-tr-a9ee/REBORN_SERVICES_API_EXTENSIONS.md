# RebornServicesApi Extensions Complete

## Summary
Successfully added capability management endpoints to the RebornServicesApi trait and provided stub implementations in RebornServices.

## Changes Made

### 1. New DTOs in types.rs ✅

Added to `/Volumes/SSDE/brassclaw/crates/brassclaw_product_workflow/src/reborn_services/types.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebornCapabilityInfo {
    pub id: String,
    pub description: String,
    pub provider: String,
    pub effects: Vec<String>,
    pub permission_mode: String,
    pub default_permission: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebornListCapabilitiesResponse {
    pub capabilities: Vec<RebornCapabilityInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebornUpdateCapabilityPermissionRequest {
    pub capability_id: String,
    pub permission_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebornUpdateCapabilityPermissionResponse {
    pub capability_id: String,
    pub permission_mode: String,
    pub updated: bool,
}
```

### 2. Trait Methods Added ✅

Added to `RebornServicesApi` trait in `/Volumes/SSDE/brassclaw/crates/brassclaw_product_workflow/src/reborn_services.rs`:

```rust
/// List all available capabilities (built-in and extension-provided) with their
/// current permission modes and default permissions.
async fn list_capabilities(
    &self,
    caller: WebUiAuthenticatedCaller,
) -> Result<RebornListCapabilitiesResponse, RebornServicesError>;

/// Update the permission mode for a specific capability. The permission override
/// is scoped to the caller's tenant and persisted in the database.
async fn update_capability_permission(
    &self,
    caller: WebUiAuthenticatedCaller,
    request: RebornUpdateCapabilityPermissionRequest,
) -> Result<RebornUpdateCapabilityPermissionResponse, RebornServicesError>;
```

### 3. Stub Implementations ✅

Added stub implementations in `RebornServices`:

```rust
async fn list_capabilities(
    &self,
    _caller: WebUiAuthenticatedCaller,
) -> Result<RebornListCapabilitiesResponse, RebornServicesError> {
    // TODO: Wire up ExtensionRegistry and PermissionResolver during bridge layer rewrite
    // For now, return empty list until the capability infrastructure is fully wired
    Ok(RebornListCapabilitiesResponse {
        capabilities: Vec::new(),
    })
}

async fn update_capability_permission(
    &self,
    _caller: WebUiAuthenticatedCaller,
    _request: RebornUpdateCapabilityPermissionRequest,
) -> Result<RebornUpdateCapabilityPermissionResponse, RebornServicesError> {
    // TODO: Wire up CapabilityPermissionStore during bridge layer rewrite
    // For now, return service unavailable until the capability infrastructure is fully wired
    Err(RebornServicesError::from_status_kind(
        RebornServicesErrorCode::ServiceUnavailable,
        RebornServicesErrorKind::ServiceUnavailable,
        503,
        false,
    ))
}
```

### 4. Public Exports Updated ✅

Updated the `pub use types::` statement to include:
- `RebornCapabilityInfo`
- `RebornListCapabilitiesResponse`
- `RebornUpdateCapabilityPermissionRequest`
- `RebornUpdateCapabilityPermissionResponse`

## Implementation Strategy

The stub implementations follow the same pattern as the LLM config methods:
- `list_capabilities()` returns an empty list (safe default)
- `update_capability_permission()` returns 503 Service Unavailable (fail-closed)

This allows:
1. The API surface to be complete and type-safe
2. WebUI routes to be implemented without compilation errors
3. Actual functionality to be wired up during the bridge layer rewrite

## Next Steps

During the bridge layer rewrite (Step 7, remaining work item 3/9):

1. Add fields to `RebornServices`:
   ```rust
   extension_registry: Arc<SharedExtensionRegistry>,
   permission_resolver: Arc<PermissionResolver>,
   permission_store: Arc<dyn CapabilityPermissionStore>,
   ```

2. Implement `list_capabilities()`:
   ```rust
   async fn list_capabilities(&self, caller: WebUiAuthenticatedCaller) -> Result<...> {
       let registry = self.extension_registry.snapshot();
       let tenant_id = TenantId::new(caller.tenant_id.as_str())?;
       
       let mut capabilities = Vec::new();
       for descriptor in registry.capabilities() {
           let current_mode = self.permission_resolver
               .resolve_permission(&tenant_id, &descriptor.id)
               .await?;
           
           capabilities.push(RebornCapabilityInfo {
               id: descriptor.id.to_string(),
               description: descriptor.description.clone(),
               provider: descriptor.provider.to_string(),
               effects: descriptor.effects.iter().map(|e| e.to_string()).collect(),
               permission_mode: current_mode.to_string(),
               default_permission: descriptor.default_permission.to_string(),
           });
       }
       
       Ok(RebornListCapabilitiesResponse { capabilities })
   }
   ```

3. Implement `update_capability_permission()`:
   ```rust
   async fn update_capability_permission(&self, caller: WebUiAuthenticatedCaller, request: ...) -> Result<...> {
       let tenant_id = TenantId::new(caller.tenant_id.as_str())?;
       let capability_id = CapabilityId::new(&request.capability_id)?;
       let mode = PermissionMode::from_str(&request.permission_mode)?;
       
       self.permission_store.set(&tenant_id, &capability_id, mode).await?;
       
       Ok(RebornUpdateCapabilityPermissionResponse {
           capability_id: request.capability_id,
           permission_mode: request.permission_mode,
           updated: true,
       })
   }
   ```

## Files Modified

1. `/Volumes/SSDE/brassclaw/crates/brassclaw_product_workflow/src/reborn_services/types.rs` - Added 4 new DTOs
2. `/Volumes/SSDE/brassclaw/crates/brassclaw_product_workflow/src/reborn_services.rs` - Added 2 trait methods, 2 stub implementations, updated exports

## Status

✅ **RebornServicesApi Extensions Complete** (6/9 major components - 67%)

The API surface is now complete and ready for the bridge layer rewrite to wire up the actual functionality.