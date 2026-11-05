//! Original operator mutation -> existing immutable prefix-job metadata. Only
//! the provider transaction commits; dispatch follows after that commit.
use super::*;
impl RebornRuntime {
    /// Only before the provider mutation/locks. The existing role-CAS writer
    /// requires RC; the qualification reader separately requires coherent RR.
    /// Effective input limits are now captured inside the qualification's own
    /// coherent RR transaction; do not pre-read the preparation policy here.
    pub(crate) async fn prepare_provider_bootstrap(
        &self,
    ) -> Result<
        crate::bootstrap_setup_approval::Prepared,
        brassclaw_product_workflow::LlmConfigServiceError,
    > {
        self.installed_catalogue
            .setup_qualification
            .prepare(&self.installed_catalogue.pool)
            .await
            .map_err(|_| brassclaw_product_workflow::LlmConfigServiceError::Unavailable)
    }
    pub(crate) async fn reserve_provider_bootstrap(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        prepared: &crate::bootstrap_setup_approval::Prepared,
        origin: Uuid,
        caller: &brassclaw_product_workflow::WebUiAuthenticatedCaller,
        retained: &crate::pg_provider_repo::ProviderDefinitionRevision,
        model: &str,
    ) -> Result<Uuid, brassclaw_product_workflow::LlmConfigServiceError> {
        use brassclaw_product_workflow::LlmConfigServiceError as E;
        if caller.tenant_id.as_str() != self.webui_tenant_id() || model.is_empty() {
            return Err(E::Unavailable);
        }
        prepared
            .require_owner(&self.installed_catalogue.setup_qualification)
            .map_err(|_| E::Unavailable)?;
        let program = prepared.program();
        let store = crate::prefix_model_designs::PgPrefixModelDesigns::new(
            self.installed_catalogue.pool.clone(),
            caller.tenant_id.as_str().into(),
        );
        let specification = store
            .capture_original(tx, retained, model)
            .await
            .map_err(|_| E::Unavailable)?;
        // This is the existing specification-row lock used by manual CAS. A
        // concurrent manual edit is entirely before or after this exact capture.
        let setup = store
            .setup_draft_in_transaction(tx, &specification.id, None)
            .await
            .map_err(|_| E::Unavailable)?;
        let id = Uuid::new_v5(
            &origin,
            format!(
                "brassclaw-provider-bootstrap/1:{}",
                caller.tenant_id.as_str()
            )
            .as_bytes(),
        );
        let (scope, thread_scope) = self
            .prefix_definition_scope(id, caller)
            .map_err(|_| E::Unavailable)?;
        let target = crate::prefix_job_definitions::Target::ModelSetup {
            specification: specification.id.clone(),
            setup_revision: setup.revision,
        };
        let request = serde_json::json!({"operation":"bootstrap_setup","origin_mutation":origin,"specification":specification.id,"setup_revision":setup.revision,"executing_specification":specification.id});
        let input = crate::prefix_bootstrap_setup::inputs(
            id,
            origin,
            &specification,
            &setup,
            prepared.original_input_limits(),
        )
        .map_err(|_| E::Unavailable)?;
        crate::prefix_job_definitions::Store::reserve_in_transaction(
            tx,
            crate::prefix_job_definitions::Reservation {
                id,
                target: &target,
                operation: "bootstrap_setup",
                mode: "target",
                request: &request,
                program: &program,
                input: &input,
                original_input_limits: prepared.original_input_limits(),
                origin: &crate::prefix_job_definitions::Origin::Chat {
                    scope: scope.clone(),
                    thread_scope: thread_scope.clone(),
                },
                created_by: caller.user_id.as_str(),
            },
        )
        .await
        .map_err(|_| E::Unavailable)?;
        Ok(id)
    }
    pub(crate) async fn admit_provider_bootstrap(
        &self,
        id: Uuid,
    ) -> Result<(), brassclaw_product_workflow::LlmConfigServiceError> {
        use brassclaw_product_workflow::LlmConfigServiceError as E;
        let definition =
            crate::prefix_job_definitions::Store::new(self.installed_catalogue.pool.clone())
                .get(id)
                .await
                .map_err(|_| E::Unavailable)?
                .ok_or(E::Unavailable)?;
        if definition.origin.tenant_id().as_str() != self.webui_tenant_id()
            || !matches!(
                definition.target,
                crate::prefix_job_definitions::Target::ModelSetup { .. }
            )
        {
            return Err(E::Unavailable);
        }
        self.admit_prefix_definition(definition)
            .await
            .map_err(|_| E::Unavailable)?;
        Ok(())
    }
}
