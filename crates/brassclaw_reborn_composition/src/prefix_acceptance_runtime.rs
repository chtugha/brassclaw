//! Authenticated operator review composition. The caller identity is native
//! host context, never a field in the design/model's submitted JSON.
use super::*;
use brassclaw_product_workflow::{
    InterceptorConfigServiceError as Error, PrefixAcceptanceDecision, PrefixAcceptanceReceipt,
    PrefixAcceptanceSelection, PrefixAcceptanceView, WebUiAuthenticatedCaller,
};

impl RebornRuntime {
    async fn prepare_prefix_review(
        &self,
        caller: &WebUiAuthenticatedCaller,
        selection: &PrefixAcceptanceSelection,
    ) -> Result<crate::prefix_acceptances::PreparedPrefixReview, Error> {
        if caller.tenant_id.as_str() != self.webui_tenant_id() {
            return Err(Error::Conflict);
        }
        let pool = self.services().pg_pool.clone().ok_or(Error::Unavailable)?;
        let worker = self
            .services()
            .source_inspection
            .clone()
            .ok_or(Error::Unavailable)?;
        let backend = crate::model_encoding::ContainedModelEncoding::new(pool.clone(), worker);
        let scope = brassclaw_host_api::ResourceScope {
            tenant_id: caller.tenant_id.clone(),
            user_id: caller.user_id.clone(),
            agent_id: None,
            project_id: None,
            thread_id: None,
            invocation_id: brassclaw_host_api::InvocationId::new(),
        };
        crate::prefix_acceptances::PgPrefixAcceptances::new(pool, caller.tenant_id.to_string())
            .prepare(
                &selection.profile,
                &selection.generation,
                &scope,
                &backend,
                self.resource_governor.clone(),
            )
            .await
    }
    pub(crate) async fn prepare_prefix_pair_acceptance(
        &self,
        caller: WebUiAuthenticatedCaller,
        selection: brassclaw_product_workflow::PrefixPairAcceptanceSelection,
    ) -> Result<brassclaw_product_workflow::PrefixPairAcceptanceView, Error> {
        // Same actual encoder/preparation as the old generation review. The
        // resulting Pair summary explicitly cannot supply a design seal.
        self.prepare_prefix_review(&caller, &selection.evaluation)
            .await?
            .pair_view(selection)
    }
    pub(crate) async fn prepare_prefix_acceptance(
        &self,
        caller: WebUiAuthenticatedCaller,
        selection: PrefixAcceptanceSelection,
    ) -> Result<PrefixAcceptanceView, Error> {
        let prepared = self.prepare_prefix_review(&caller, &selection).await?;
        serde_json::from_value(prepared.view()).map_err(|_| Error::Unavailable)
    }
    pub(crate) async fn approve_prefix_acceptance(
        &self,
        caller: WebUiAuthenticatedCaller,
        decision: PrefixAcceptanceDecision,
    ) -> Result<PrefixAcceptanceReceipt, Error> {
        if caller.tenant_id.as_str() != self.webui_tenant_id() || decision.acceptance_id.is_nil() {
            return Err(Error::Conflict);
        }
        let owner = crate::prefix_acceptances::PgPrefixAcceptances::new(
            self.services().pg_pool.clone().ok_or(Error::Unavailable)?,
            caller.tenant_id.to_string(),
        );
        if !owner
            .approved_retry(&decision, caller.user_id.as_str())
            .await?
        {
            let prepared = self
                .prepare_prefix_review(&caller, &decision.selection)
                .await?;
            owner
                .approve(
                    prepared,
                    decision.acceptance_id,
                    &decision.review_checksum,
                    caller.user_id.as_str(),
                    &decision.semantic_review,
                )
                .await?;
        }
        Ok(PrefixAcceptanceReceipt {
            generation_qualification: decision.acceptance_id,
            serving_activated: false,
        })
    }
    pub(crate) async fn activate_prefix_acceptance(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: brassclaw_product_workflow::ActivatePrefixAcceptanceRequest,
    ) -> Result<brassclaw_product_workflow::PrefixTargetBindingReceipt, Error> {
        if caller.tenant_id.as_str() != self.webui_tenant_id() {
            return Err(Error::Conflict);
        }
        let prepared = self
            .services()
            .source_inspection
            .as_ref()
            .ok_or(Error::Unavailable)?
            .prepare()
            .await
            .map_err(|_| Error::Unavailable)?;
        let owner = crate::prefix_acceptances::PgPrefixAcceptances::new(
            self.services().pg_pool.clone().ok_or(Error::Unavailable)?,
            caller.tenant_id.to_string(),
        );
        let selected = owner
            .resolve(request.generation_qualification, prepared.revision)
            .await?;
        if selected.worker_sha256 != hex::encode(prepared.worker.checksum()) {
            return Err(Error::Conflict);
        }
        self.prepare_prefix_review(
            &caller,
            &PrefixAcceptanceSelection {
                profile: selected.profile.clone(),
                generation: selected.generation.clone(),
            },
        )
        .await?;
        let revision = owner
            .activate(
                request.generation_qualification,
                request.expected_revision,
                prepared.revision,
            )
            .await?;
        Ok(brassclaw_product_workflow::PrefixTargetBindingReceipt {
            generation_qualification: request.generation_qualification,
            revision,
        })
    }
}
