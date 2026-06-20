//! Built-in capability dispatcher for v2 capabilities.
//!
//! This module implements the `CapabilityDispatcher` trait for all built-in
//! capabilities, routing capability IDs to their corresponding execute functions.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use brassclaw_host_api::{
    CapabilityDispatchRequest, CapabilityDispatchResult, CapabilityDispatcher, DispatchError,
    ExtensionId, ResourceReceipt, ResourceUsage, RuntimeDispatchErrorKind, RuntimeKind,
};
use rust_decimal::Decimal;
use serde_json::Value;

use super::extensions::ExtensionsContext;
use super::filesystem::FilesystemContext;
use super::images::ImagesContext;
use super::jobs::JobsContext;
use super::memory::MemoryContext;
use super::messaging::MessagingContext;
use super::network::NetworkContext;
use super::pairing::PairingContext;
use super::routines::RoutinesContext;
use super::secrets::SecretsContext;
use super::shell::ShellContext;
use super::skills::SkillsContext;
use super::system::SystemContext;

/// Built-in capability dispatcher that routes capability IDs to v2 execute functions.
pub struct BuiltinCapabilityDispatcher {
    filesystem_ctx: Arc<FilesystemContext>,
    shell_ctx: Arc<ShellContext>,
    network_ctx: Arc<NetworkContext>,
    memory_ctx: Arc<MemoryContext>,
    messaging_ctx: Arc<MessagingContext>,
    jobs_ctx: Arc<JobsContext>,
    routines_ctx: Arc<RoutinesContext>,
    skills_ctx: Arc<SkillsContext>,
    extensions_ctx: Arc<ExtensionsContext>,
    secrets_ctx: Arc<SecretsContext>,
    images_ctx: Arc<ImagesContext>,
    system_ctx: Arc<SystemContext>,
    pairing_ctx: Arc<PairingContext>,
}

impl BuiltinCapabilityDispatcher {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        filesystem_ctx: Arc<FilesystemContext>,
        shell_ctx: Arc<ShellContext>,
        network_ctx: Arc<NetworkContext>,
        memory_ctx: Arc<MemoryContext>,
        messaging_ctx: Arc<MessagingContext>,
        jobs_ctx: Arc<JobsContext>,
        routines_ctx: Arc<RoutinesContext>,
        skills_ctx: Arc<SkillsContext>,
        extensions_ctx: Arc<ExtensionsContext>,
        secrets_ctx: Arc<SecretsContext>,
        images_ctx: Arc<ImagesContext>,
        system_ctx: Arc<SystemContext>,
        pairing_ctx: Arc<PairingContext>,
    ) -> Self {
        Self {
            filesystem_ctx,
            shell_ctx,
            network_ctx,
            memory_ctx,
            messaging_ctx,
            jobs_ctx,
            routines_ctx,
            skills_ctx,
            extensions_ctx,
            secrets_ctx,
            images_ctx,
            system_ctx,
            pairing_ctx,
        }
    }

    async fn dispatch_internal(
        &self,
        request: &CapabilityDispatchRequest,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let capability_id = request.capability_id.as_str();
        let params = &request.input;

        match capability_id {
            // Filesystem capabilities
            super::filesystem::READ_FILE_CAPABILITY_ID => {
                super::filesystem::execute_read_file(params, &self.filesystem_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::filesystem::WRITE_FILE_CAPABILITY_ID => {
                super::filesystem::execute_write_file(params, &self.filesystem_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::filesystem::LIST_DIR_CAPABILITY_ID => {
                super::filesystem::execute_list_dir(params, &self.filesystem_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::filesystem::APPLY_PATCH_CAPABILITY_ID => {
                super::filesystem::execute_apply_patch(params, &self.filesystem_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::filesystem::GLOB_CAPABILITY_ID => {
                super::filesystem::execute_glob(params, &self.filesystem_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::filesystem::GREP_CAPABILITY_ID => {
                super::filesystem::execute_grep(params, &self.filesystem_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::filesystem::FILE_UNDO_CAPABILITY_ID => {
                super::filesystem::execute_file_undo(params, &self.filesystem_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // Shell capabilities
            super::shell::SHELL_CAPABILITY_ID => {
                super::shell::execute_shell(params, &self.shell_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // Network capabilities
            super::network::HTTP_CAPABILITY_ID => {
                super::network::execute_http(params, &self.network_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // Memory capabilities
            super::memory::MEMORY_READ_CAPABILITY_ID => {
                super::memory::execute_memory_read(params, &self.memory_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::memory::MEMORY_WRITE_CAPABILITY_ID => {
                super::memory::execute_memory_write(params, &self.memory_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::memory::MEMORY_SEARCH_CAPABILITY_ID => {
                super::memory::execute_memory_search(params, &self.memory_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::memory::MEMORY_TREE_CAPABILITY_ID => {
                super::memory::execute_memory_tree(params, &self.memory_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // Messaging capabilities
            super::messaging::MESSAGE_CAPABILITY_ID => {
                super::messaging::execute_message(params, &self.messaging_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // Jobs capabilities
            super::jobs::CREATE_JOB_CAPABILITY_ID => {
                super::jobs::execute_create_job(params, &self.jobs_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::jobs::CANCEL_JOB_CAPABILITY_ID => {
                super::jobs::execute_cancel_job(params, &self.jobs_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::jobs::LIST_JOBS_CAPABILITY_ID => {
                super::jobs::execute_list_jobs(params, &self.jobs_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::jobs::JOB_STATUS_CAPABILITY_ID => {
                super::jobs::execute_job_status(params, &self.jobs_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::jobs::JOB_EVENTS_CAPABILITY_ID => {
                super::jobs::execute_job_events(params, &self.jobs_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::jobs::JOB_PROMPT_CAPABILITY_ID => {
                super::jobs::execute_job_prompt(params, &self.jobs_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // Routines capabilities
            super::routines::ROUTINE_CREATE_CAPABILITY_ID => {
                super::routines::execute_routine_create(params, &self.routines_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::routines::ROUTINE_UPDATE_CAPABILITY_ID => {
                super::routines::execute_routine_update(params, &self.routines_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::routines::ROUTINE_DELETE_CAPABILITY_ID => {
                super::routines::execute_routine_delete(params, &self.routines_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::routines::ROUTINE_LIST_CAPABILITY_ID => {
                super::routines::execute_routine_list(params, &self.routines_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::routines::ROUTINE_HISTORY_CAPABILITY_ID => {
                super::routines::execute_routine_history(params, &self.routines_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::routines::ROUTINE_FIRE_CAPABILITY_ID => {
                super::routines::execute_routine_fire(params, &self.routines_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::routines::EVENT_EMIT_CAPABILITY_ID => {
                super::routines::execute_event_emit(params, &self.routines_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // Skills capabilities
            super::skills::SKILL_INSTALL_CAPABILITY_ID => {
                super::skills::execute_skill_install(params, &self.skills_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::skills::SKILL_REMOVE_CAPABILITY_ID => {
                super::skills::execute_skill_remove(params, &self.skills_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::skills::SKILL_LIST_CAPABILITY_ID => {
                super::skills::execute_skill_list(params, &self.skills_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::skills::SKILL_SEARCH_CAPABILITY_ID => {
                super::skills::execute_skill_search(params, &self.skills_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // Extensions capabilities
            super::extensions::TOOL_INSTALL_CAPABILITY_ID => {
                super::extensions::execute_tool_install(params, &self.extensions_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::extensions::TOOL_REMOVE_CAPABILITY_ID => {
                super::extensions::execute_tool_remove(params, &self.extensions_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::extensions::TOOL_LIST_CAPABILITY_ID => {
                super::extensions::execute_tool_list(params, &self.extensions_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::extensions::TOOL_SEARCH_CAPABILITY_ID => {
                super::extensions::execute_tool_search(params, &self.extensions_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::extensions::TOOL_UPGRADE_CAPABILITY_ID => {
                super::extensions::execute_tool_upgrade(params, &self.extensions_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::extensions::TOOL_AUTH_CAPABILITY_ID => {
                super::extensions::execute_tool_auth(params, &self.extensions_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::extensions::TOOL_INFO_CAPABILITY_ID => {
                super::extensions::execute_tool_info(params, &self.extensions_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::extensions::EXTENSION_INFO_CAPABILITY_ID => {
                super::extensions::execute_extension_info(params, &self.extensions_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::extensions::TOOL_PERMISSION_SET_CAPABILITY_ID => {
                super::extensions::execute_tool_permission_set(params, &self.extensions_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // Secrets capabilities
            super::secrets::SECRET_LIST_CAPABILITY_ID => {
                super::secrets::execute_secret_list(params, &self.secrets_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::secrets::SECRET_DELETE_CAPABILITY_ID => {
                super::secrets::execute_secret_delete(params, &self.secrets_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // Images capabilities
            super::images::IMAGE_GENERATE_CAPABILITY_ID => {
                super::images::execute_image_generate(params, &self.images_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::images::IMAGE_ANALYZE_CAPABILITY_ID => {
                super::images::execute_image_analyze(params, &self.images_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::images::IMAGE_EDIT_CAPABILITY_ID => {
                super::images::execute_image_edit(params, &self.images_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // System capabilities (mix of sync and async)
            super::system::ECHO_CAPABILITY_ID => {
                super::system::execute_echo(params)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::system::JSON_CAPABILITY_ID => {
                super::system::execute_json(params, &self.system_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::system::TIME_CAPABILITY_ID => {
                super::system::execute_time(params, &self.system_ctx.user_timezone)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::system::SYSTEM_VERSION_CAPABILITY_ID => {
                super::system::execute_system_version()
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::system::SYSTEM_TOOLS_LIST_CAPABILITY_ID => {
                super::system::execute_system_tools_list(&self.system_ctx)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::system::PLAN_UPDATE_CAPABILITY_ID => {
                super::system::execute_plan_update(params, &self.system_ctx)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
            super::system::RESTART_CAPABILITY_ID => {
                super::system::execute_restart(params)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            // Pairing capabilities
            super::pairing::PAIRING_APPROVE_CAPABILITY_ID => {
                super::pairing::execute_pairing_approve(params, &self.pairing_ctx)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }

            _ => Err(format!("unknown capability: {}", capability_id).into()),
        }
    }
}

#[async_trait]
impl CapabilityDispatcher for BuiltinCapabilityDispatcher {
    async fn dispatch_json(
        &self,
        request: CapabilityDispatchRequest,
    ) -> Result<CapabilityDispatchResult, DispatchError> {
        let start = Instant::now();
        let capability_id = request.capability_id.clone();

        // Execute the capability
        let output = self
            .dispatch_internal(&request)
            .await
            .map_err(|e| DispatchError::FirstParty {
                kind: RuntimeDispatchErrorKind::OperationFailed,
                safe_summary: Some(e.to_string()),
            })?;

        // Calculate resource usage
        let elapsed = start.elapsed();
        let wall_clock_ms = elapsed.as_millis() as u64;
        let output_bytes = serde_json::to_vec(&output)
            .map(|v| v.len() as u64)
            .unwrap_or(0);

        let usage = ResourceUsage {
            usd: Decimal::ZERO,
            input_tokens: 0,
            output_tokens: 0,
            wall_clock_ms,
            output_bytes,
            network_egress_bytes: 0,
            process_count: 0,
        };

        let receipt = ResourceReceipt {
            id: request.resource_reservation
                .as_ref()
                .map(|r| r.id)
                .unwrap_or_else(brassclaw_host_api::ResourceReservationId::new),
            scope: request.scope.clone(),
            status: brassclaw_host_api::ReservationStatus::Reconciled,
            estimate: request.estimate.clone(),
            actual: Some(usage.clone()),
        };

        Ok(CapabilityDispatchResult {
            capability_id,
            provider: ExtensionId::new("builtin").expect("valid provider id"),
            runtime: RuntimeKind::FirstParty,
            output,
            display_preview: None,
            usage,
            receipt,
        })
    }
}

