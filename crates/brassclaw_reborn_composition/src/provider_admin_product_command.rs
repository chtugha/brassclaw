use async_trait::async_trait;
use brassclaw_product_adapters::{
    ProductCommandResultPayload, ProductInboundAck, ProductRejection, ProductRejectionKind,
};
use brassclaw_product_workflow::{
    ProductCommand, ProductCommandContext, ProductCommandService, ProductModelCommand,
    ProductWorkflowError,
};
use serde::Serialize;

use crate::{
    RebornModelRoutesState, RebornProviderAdmin, RebornProviderAdminError, RebornProviderSelection,
    RebornProviderStatus, RebornV1State,
};

pub struct RebornProviderAdminProductCommandService {
    admin: RebornProviderAdmin,
}

impl RebornProviderAdminProductCommandService {
    pub fn new(admin: RebornProviderAdmin) -> Self {
        Self { admin }
    }
}

#[async_trait]
impl ProductCommandService for RebornProviderAdminProductCommandService {
    async fn execute(
        &self,
        _context: ProductCommandContext,
        command: ProductCommand,
    ) -> Result<ProductInboundAck, ProductWorkflowError> {
        let ProductCommand::Model { action } = command else {
            return Ok(ProductInboundAck::Rejected(ProductRejection::permanent(
                ProductRejectionKind::PolicyDenied,
                format!("command routing unavailable: {}", command.name()),
            )));
        };

        let admin = self.admin.clone();
        let payload = tokio::task::spawn_blocking(move || provider_admin_payload(admin, action))
            .await
            .map_err(|error| ProductWorkflowError::Transient {
                reason: format!("provider-admin task failed: {error}"),
            })??;

        Ok(ProductInboundAck::CommandResult {
            command: "model".to_string(),
            payload: ProductCommandResultPayload::new(payload),
        })
    }
}

fn provider_admin_payload(
    admin: RebornProviderAdmin,
    action: ProductModelCommand,
) -> Result<serde_json::Value, ProductWorkflowError> {
    let payload = match action {
        ProductModelCommand::Status => {
            ProductSafeProviderStatus::from(admin.status().map_err(provider_admin_workflow_error)?)
                .to_value()
        }
        // Phase 8: set_model and set_provider are no longer file-based.
        // Provider/model writes now go through LlmConfigService::set_active
        // (DB-backed). Return a transient error to surface this at runtime.
        ProductModelCommand::Set { model: _ } | ProductModelCommand::SetProvider { .. } => {
            return Err(ProductWorkflowError::Transient {
                reason:
                    "model/provider writes via product command are not supported in this build; \
                         use the WebUI settings or `brassclaw config set` instead"
                        .to_string(),
            });
        }
    };
    payload.map_err(|error| ProductWorkflowError::Transient {
        reason: format!("provider-admin response serialization failed: {error}"),
    })
}

#[derive(Serialize)]
struct ProductSafeProviderStatus {
    routes: RebornModelRoutesState,
    default: Option<ProductSafeProviderSelection>,
    v1_state: RebornV1State,
}

impl From<RebornProviderStatus> for ProductSafeProviderStatus {
    fn from(status: RebornProviderStatus) -> Self {
        Self {
            routes: status.routes,
            default: status.default.map(ProductSafeProviderSelection::from),
            v1_state: status.v1_state,
        }
    }
}

impl ProductSafeProviderStatus {
    fn to_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

#[derive(Serialize)]
struct ProductSafeProviderSelection {
    provider_id: Option<String>,
    provider_known: bool,
    model: Option<String>,
}

impl From<RebornProviderSelection> for ProductSafeProviderSelection {
    fn from(selection: RebornProviderSelection) -> Self {
        Self {
            provider_id: selection.provider_id,
            provider_known: selection.provider_known,
            model: selection.model,
        }
    }
}

fn provider_admin_workflow_error(error: RebornProviderAdminError) -> ProductWorkflowError {
    match error {
        RebornProviderAdminError::UnknownProvider { provider, .. } => {
            ProductWorkflowError::InvalidBindingRequest {
                reason: format!("unknown Reborn LLM provider `{provider}`"),
            }
        }
        RebornProviderAdminError::InvalidRequest { reason } => {
            ProductWorkflowError::InvalidBindingRequest { reason }
        }
        RebornProviderAdminError::LoadRegistry { reason, .. } => ProductWorkflowError::Transient {
            reason: format!("load Reborn provider catalog failed: {reason}"),
        },
        RebornProviderAdminError::LoadConfig { source, .. } => ProductWorkflowError::Transient {
            reason: format!(
                "load Reborn config failed: {}",
                config_load_error_reason(source.as_ref())
            ),
        },
    }
}

fn config_load_error_reason(error: &brassclaw_reborn_config::RebornConfigFileError) -> String {
    match error {
        brassclaw_reborn_config::RebornConfigFileError::Io { source, .. } => {
            format!("read failed: {source}")
        }
        brassclaw_reborn_config::RebornConfigFileError::Toml { source, .. } => {
            format!("TOML parse failed: {source}")
        }
        brassclaw_reborn_config::RebornConfigFileError::IncompatibleApiVersion {
            found,
            expected,
            ..
        } => {
            format!("api_version `{found}` is incompatible with `{expected}`")
        }
        brassclaw_reborn_config::RebornConfigFileError::InlineSecret { source, .. } => {
            format!("field validation failed: {source}")
        }
        brassclaw_reborn_config::RebornConfigFileError::InvalidField { field, reason, .. } => {
            format!("field `{field}` validation failed: {reason}")
        }
        brassclaw_reborn_config::RebornConfigFileError::InvalidApiVersion {
            found, reason, ..
        } => {
            format!("api_version `{found}` could not be parsed: {reason}")
        }
    }
}
