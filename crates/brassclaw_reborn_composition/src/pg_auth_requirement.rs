//! Exact requirement head publication joins the existing Auth transaction.
//! The immutable selection remains in its original flow custody after a later
//! head replacement. Neither this record nor its content reference is a grant.
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use brassclaw_auth::{CredentialRequirementPublication, CredentialRequirementSelection};

impl PgAuthProductServices {
    pub(super) async fn publish_requirement_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        document: &mut StoredAuthFlow,
        account: &CredentialAccount,
    ) -> Result<(), AuthProductError> {
        let Some(setup) = &document.auth_custody.requirement_setup else {
            if document.auth_custody.requirement_selection.is_some()
                || document.auth_custody.requirement_source.is_some()
            {
                return Err(AuthProductError::BackendConflict);
            }
            return Ok(());
        };
        if document.auth_custody.requirement_selection.is_some()
            || document.auth_custody.requirement_source.is_some()
        {
            // The caller recovers actual completed originals before entering
            // this new-publication path. A copied receipt is not that proof.
            return Err(AuthProductError::BackendConflict);
        }
        let selected = CredentialRequirementSelection::from_completed_flow(
            &document.flow.scope,
            setup,
            &document.flow,
            account.clone(),
        )?;
        let key = selected.key()?;
        // Read through this transaction, never another pooled connection while
        // holding Auth locks. Missing historical material stays missing proof.
        let source = if let Some(handle) = &account.access_secret {
            self.secret_store
                .inspect_record_reference_in_transaction(
                    transaction,
                    &account.scope.resource,
                    handle,
                )
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .map(|reference| {
                    brassclaw_auth::CredentialRequirementSource::from_selection(
                        &selected,
                        brassclaw_auth::CredentialSecretGenerationRef::new(reference)
                            .map_err(|_| AuthProductError::BackendUnavailable)?,
                    )
                })
                .transpose()?
        } else {
            None
        };
        let tenant = document.flow.scope.resource.tenant_id.as_str();
        let row = transaction
            .query_opt(
                "SELECT data,revision FROM brassclaw_product_auth_requirement_selections
                 WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
                &[&tenant, &key.as_str()],
            )
            .await
            .map_err(db_err)?;
        let current: Option<(CredentialRequirementSelection, i64)> = row
            .map(|row| {
                serde_json::from_value(row.get(0))
                    .map(|selection| (selection, row.get(1)))
                    .map_err(|_| AuthProductError::BackendUnavailable)
            })
            .transpose()?;
        if let Some((current, _)) = &current {
            current.verify_requested(&document.flow.scope, setup.requirement())?;
            let origin = transaction
                .query_opt(
                    "SELECT data FROM brassclaw_product_auth_flows
                     WHERE tenant_id=$1 AND id=$2",
                    &[&tenant, &current.original_flow().to_string()],
                )
                .await
                .map_err(db_err)?
                .ok_or(AuthProductError::BackendConflict)?;
            let origin: StoredAuthFlow = serde_json::from_value(origin.get(0))
                .map_err(|_| AuthProductError::BackendUnavailable)?;
            let original_setup = origin
                .auth_custody
                .requirement_setup
                .as_deref()
                .ok_or(AuthProductError::BackendConflict)?;
            if origin.auth_custody.requirement_selection.as_deref() != Some(current) {
                return Err(AuthProductError::BackendConflict);
            }
            current.verify_receipt(&origin.flow, original_setup)?;
            if let Some(source) = &origin.auth_custody.requirement_source {
                source.verify_selection(current)?;
            }
        }
        if setup.publication(current.as_ref().map(|(selection, _)| selection), &selected)?
            != CredentialRequirementPublication::Publish
        {
            return Err(AuthProductError::BackendConflict);
        }
        let data =
            serde_json::to_value(&selected).map_err(|_| AuthProductError::BackendUnavailable)?;
        let changed = if let Some((current, revision)) = current {
            let expected =
                serde_json::to_value(current).map_err(|_| AuthProductError::BackendUnavailable)?;
            transaction
                .execute(
                    "UPDATE brassclaw_product_auth_requirement_selections
                     SET data=$3,revision=revision+1,updated_at=NOW()
                     WHERE tenant_id=$1 AND id=$2 AND revision=$4 AND data=$5",
                    &[&tenant, &key.as_str(), &data, &revision, &expected],
                )
                .await
        } else {
            // The unique key serializes two absent-head publishers. Losing the
            // insert rolls back the entire caller transaction, including any
            // account/material changes; no advisory lock or fallback write.
            transaction
                .execute(
                    "INSERT INTO brassclaw_product_auth_requirement_selections
                     (tenant_id,id,user_id,data) VALUES($1,$2,$3,$4)
                     ON CONFLICT(tenant_id,id) DO NOTHING",
                    &[
                        &tenant,
                        &key.as_str(),
                        &document.flow.scope.resource.user_id.as_str(),
                        &data,
                    ],
                )
                .await
        }
        .map_err(db_err)?;
        if changed != 1 {
            return Err(AuthProductError::BackendConflict);
        }
        document.auth_custody.requirement_selection = Some(Box::new(selected));
        document.auth_custody.requirement_source = source.map(Box::new);
        Ok(())
    }
}
