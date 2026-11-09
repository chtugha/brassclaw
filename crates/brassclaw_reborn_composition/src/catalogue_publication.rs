//! Existing catalogue publication's source invalidation and uptake projection.
//! This records references, not a queue, approval or generated-prefix selection.
use super::*;
use crate::pg_review_submission::selection;
use brassclaw_product_workflow::{
    CatalogueActivationStatus, CataloguePublicationStatus, ComponentRevisionSelection,
};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileRevision {
    name: String,
    revision: u64,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PrefixInvalidation {
    format: String,
    catalogue_generation: Uuid,
    catalogue_checksum: String,
    inline_prefix_checksum: String,
    root: ComponentRevisionSelection,
    source_documents: Vec<ComponentRevisionSelection>,
    profiles: Vec<ProfileRevision>,
}
fn unavailable() -> RecipeStoreError {
    RecipeStoreError::Unavailable("catalogue publication reconciliation unavailable".into())
}
fn integrity() -> RecipeStoreError {
    RecipeStoreError::Internal("catalogue publication integrity failed".into())
}
fn checksum(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
struct Publication {
    generation: Uuid,
    catalogue_checksum: String,
    inline_prefix_checksum: String,
    activation: Option<Uuid>,
    invalidation: Option<PrefixInvalidation>,
}
impl InstalledMontyCatalogue {
    /// Resume publication from an existing immutable receipt, including after a
    /// browser reconnect. No new request, actor claim or catalogue is accepted.
    pub(crate) async fn reconcile_activation(
        &self,
        activation: &str,
    ) -> Result<CatalogueActivationReceipt, RecipeStoreError> {
        let activation = identifier(activation)?;
        let _serial = self.activation.lock().await;
        let client = self.pool.get().await.map_err(storage)?;
        let row = client.query_opt(
            "SELECT actor,request_bytes FROM brassclaw_monty_catalogue_activations WHERE activation_id=$1",
            &[&activation],
        ).await.map_err(storage)?.ok_or_else(|| RecipeStoreError::NotFound("catalogue activation not found".into()))?;
        let actor: String = row.get(0);
        let request: String = row.get(1);
        drop(client);
        self.finish_activation(&actor, activation, &request, None)
            .await
    }
    pub(super) async fn prefix_invalidation(
        &self,
        tx: &Transaction<'_>,
        view: &CatalogueGeneration,
    ) -> Result<Value, RecipeStoreError> {
        // Retain actual registered profile revisions, including unsupported
        // profiles. Recording a rebuild dependency does not enable its compiler.
        let profiles = tx.query(
            "SELECT name,revision FROM reborn_prefix_profiles ORDER BY name COLLATE \"C\" FOR SHARE", &[]
        ).await.map_err(storage)?.into_iter().map(|row| {
            Ok(ProfileRevision {
                name: row.get(0),
                revision: u64::try_from(row.get::<_, i64>(1)).map_err(invalid)?,
            })
        }).collect::<Result<Vec<_>, RecipeStoreError>>()?;
        serde_json::to_value(PrefixInvalidation {
            format: "catalogue-prefix-invalidation/1".into(),
            catalogue_generation: view.generation,
            catalogue_checksum: digest(&view.bytes),
            inline_prefix_checksum: digest(&view.prefix),
            root: selection(self.root_reference),
            source_documents: self
                .reference_sources
                .snapshot
                .revisions()
                .values()
                .map(|r| selection(r.reference()))
                .collect(),
            profiles,
        })
        .map_err(invalid)
    }
    fn checked_invalidation(
        &self,
        generation: Uuid,
        catalogue_checksum: &str,
        inline_prefix_checksum: &str,
        value: Option<Value>,
    ) -> Result<Option<PrefixInvalidation>, RecipeStoreError> {
        let Some(value) = value else { return Ok(None) };
        let retained: PrefixInvalidation =
            serde::Deserialize::deserialize(&value).map_err(|_| integrity())?;
        if serde_json::to_value(&retained).map_err(|_| integrity())? != value {
            return Err(integrity());
        }
        let root =
            crate::pg_review_submission::reference(&retained.root).map_err(|_| integrity())?;
        let documents = retained
            .source_documents
            .iter()
            .map(crate::pg_review_submission::reference)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| integrity())?;
        let expected = self
            .reference_sources
            .snapshot
            .revisions()
            .values()
            .map(|r| r.reference())
            .collect::<Vec<_>>();
        if retained.format != "catalogue-prefix-invalidation/1"
            || retained.catalogue_generation != generation
            || retained.catalogue_checksum != catalogue_checksum
            || retained.inline_prefix_checksum != inline_prefix_checksum
            || root != self.root_reference
            || documents != expected
            || retained
                .profiles
                .iter()
                .any(|p| p.name.is_empty() || p.revision == 0)
            || retained.profiles.windows(2).any(|p| p[0].name >= p[1].name)
        {
            return Err(integrity());
        }
        Ok(Some(retained))
    }
    pub(super) fn verify_prefix_invalidation(
        &self,
        generation: Uuid,
        catalogue: &str,
        value: Option<Value>,
    ) -> Result<(), RecipeStoreError> {
        let document = catalogue_document(catalogue)?;
        let prefix = document["prefix_checksum"]
            .as_str()
            .filter(|s| checksum(s))
            .ok_or_else(integrity)?;
        self.checked_invalidation(generation, &digest(catalogue), prefix, value)?;
        Ok(())
    }
    async fn committed_publication(&self) -> Result<Publication, RecipeStoreError> {
        let client = self.pool.get().await.map_err(storage)?;
        let row = client
            .query_one(
                "SELECT a.catalogue_id,c.checksum,a.activation_id,p.prefix_invalidation,
                    COALESCE(p.prefix_invalidation->>'inline_prefix_checksum',
                             c.catalogue_bytes::jsonb->>'prefix_checksum')
             FROM brassclaw_monty_active_catalogue a
             JOIN brassclaw_monty_boot_catalogues c USING(catalogue_id)
             LEFT JOIN brassclaw_monty_catalogue_activations p
               ON p.activation_id=a.activation_id AND p.catalogue_id=a.catalogue_id
             WHERE a.singleton",
                &[],
            )
            .await
            .map_err(storage)?;
        let generation = row.get(0);
        let catalogue_checksum: String = row.get(1);
        let inline_prefix_checksum: String = row
            .get::<_, Option<String>>(4)
            .filter(|v| checksum(v))
            .ok_or_else(unavailable)?;
        let invalidation = self.checked_invalidation(
            generation,
            &catalogue_checksum,
            &inline_prefix_checksum,
            row.get(3),
        )?;
        Ok(Publication {
            generation,
            catalogue_checksum,
            inline_prefix_checksum,
            activation: row.get(2),
            invalidation,
        })
    }
    /// Caller already holds activation serialization. Fresh task capture and
    /// source export reconcile the durable target instead of trusting a lost
    /// notification or republishing a superseded original activation.
    pub(crate) async fn reconcile_committed_runtime_locked(&self) -> Result<(), RecipeStoreError> {
        let desired = self.committed_publication().await?;
        // End the read guard before restoration takes the write side.
        let stale = self.active.read().await.generation != desired.generation;
        if stale && let Err(error) = self.restore_authored_locked().await {
            if self.active.read().await.generation != desired.generation {
                return Err(error);
            }
            // Core publication succeeded, discovery uptake remains pending.
            // Provider-free ordinary work can use the verified new view;
            // MCP validates its own advertised contract before chat ingress.
            tracing::warn!("catalogue runtime reconciled; discovery uptake remains pending");
        }
        if self.active.read().await.generation != desired.generation {
            return Err(unavailable());
        }
        Ok(())
    }

    /// Publish only a view already held by a committed active-pointer read.
    /// The caller retains its row lock through this short in-memory handoff.
    /// Command qualification runs afterwards; an empty advertised list is not
    /// an acknowledgement of that qualification attempt.
    pub(super) async fn publish_committed_view_locked(
        &self,
        view: CatalogueGeneration,
        _publication: &tokio::sync::MutexGuard<'_, ()>,
    ) -> Result<(), RecipeStoreError> {
        let expected = self.mcp_discovery.snapshot().ok().map(|v| v.generation());
        let view = Arc::new(view);
        let programs = std::iter::once(self.reply.as_ref())
            .chain(std::iter::once(self.history.as_ref()))
            .chain(view.programs.iter().map(Arc::as_ref))
            .filter(|p| view.routing_enabled(p))
            .collect::<Vec<_>>();
        *self.discovery_effective.write().await = None;
        *self.active.write().await = view.clone();
        self.mcp_discovery
            .publish_unadvertised_installed(expected, view.generation, &programs)
            .map_err(|_| unavailable())
    }

    /// A lost COMMIT is resolved only by the original immutable activation.
    /// A superseding activation reconciles its current target; this never
    /// republishes an old receipt or generates another activation identity.
    pub(super) async fn finish_activation(
        &self,
        actor: &str,
        activation: Uuid,
        request: &str,
        prepared: Option<CatalogueGeneration>,
    ) -> Result<CatalogueActivationReceipt, RecipeStoreError> {
        // Same order as command qualification: publication lock, then pool.
        // Never hold a pool lease while waiting for that publication lock.
        let publication = self.discovery_publication.lock().await;
        let mut client = self.pool.get().await.map_err(storage)?;
        let tx = client.transaction().await.map_err(storage)?;
        let row = tx
            .query_opt(
                "SELECT a.actor,a.request_bytes,a.request_checksum,a.catalogue_id,
                    a.prefix_invalidation,c.checksum,c.catalogue_bytes::jsonb->>'prefix_checksum'
             FROM brassclaw_monty_catalogue_activations a
             JOIN brassclaw_monty_boot_catalogues c USING(catalogue_id)
             WHERE a.activation_id=$1",
                &[&activation],
            )
            .await
            .map_err(storage)?
            .ok_or_else(unavailable)?;
        if row.get::<_, String>(0) != actor
            || row.get::<_, String>(1) != request
            || row.get::<_, String>(2) != digest(request)
        {
            return Err(RecipeStoreError::Conflict(
                "activation identifier belongs to different content".into(),
            ));
        }
        let original: Uuid = row.get(3);
        let original_checksum: String = row.get(5);
        let inline: String = row
            .get::<_, Option<String>>(6)
            .filter(|s| checksum(s))
            .ok_or_else(integrity)?;
        self.checked_invalidation(original, &original_checksum, &inline, row.get(4))?;
        let current: Uuid = tx.query_one(
            "SELECT catalogue_id FROM brassclaw_monty_active_catalogue WHERE singleton FOR SHARE", &[]
        ).await.map_err(storage)?.get(0);
        if let Some(view) = prepared.filter(|v| v.generation == current) {
            if view.generation != original
                || digest(&view.bytes) != original_checksum
                || digest(&view.prefix) != inline
            {
                return Err(integrity());
            }
            self.publish_committed_view_locked(view, &publication)
                .await?;
        }
        tx.commit().await.map_err(storage)?;
        // Do not retain an administration lease while restoration/discovery
        // acquire their own connection; a one-connection pool must also work.
        drop(client);
        drop(publication);
        self.reconcile_committed_runtime_locked().await?;
        self.refresh_active_discovery()
            .await
            .map_err(|_| unavailable())?;
        let current = self.committed_publication().await?;
        if self.active.read().await.generation != current.generation {
            return Err(unavailable());
        }
        Ok(CatalogueActivationReceipt {
            activation_id: activation.to_string(),
            generation: original.to_string(),
            catalogue_activated: current.generation == original,
        })
    }
    pub(crate) async fn activation_status(
        &self,
    ) -> Result<CatalogueActivationStatus, RecipeStoreError> {
        // Observation is not publication authority. Do not wait behind a stalled
        // activation/discovery attempt merely to show that its uptake is pending.
        // Every effective value carries its own generation; edits still use CAS.
        let desired = self.committed_publication().await?;
        let view = self.active.read().await.clone();
        let discovery = *self.discovery_effective.read().await;
        let inline_ready = view.generation == desired.generation
            && digest(&view.bytes) == desired.catalogue_checksum
            && digest(&view.prefix) == desired.inline_prefix_checksum;
        let reference = inline_ready.then(|| {
            format!(
                "catalogue:{}:sha256:{}",
                view.generation, desired.inline_prefix_checksum
            )
        });
        Ok(CatalogueActivationStatus {
            original_pure_case: Some(match &self.case_unready {
                Some(unready) => unready.view(),
                None => brassclaw_product_workflow::OriginalPureCaseReadiness {
                    ready: self.case_harness.is_some(),
                    evidence_pending: false,
                    phase: None,
                    reason_kind: None,
                },
            }),
            generation: view.generation.to_string(),
            recipes: view.recipes.iter().map(|r| r.recipe_id.clone()).collect(),
            selections: view.recipes.clone(),
            disabled_recipe_ids: view
                .disabled_recipes
                .iter()
                .map(ToString::to_string)
                .collect(),
            packaged_recipe_ids: vec![
                self.reply
                    .program()
                    .inputs()
                    .instruction()
                    .recipe()
                    .uuid
                    .to_string(),
            ],
            publication: CataloguePublicationStatus {
                desired_generation: desired.generation.to_string(),
                effective_generation: view.generation.to_string(),
                discovery_effective_generation: discovery.map(|g| g.to_string()),
                pending: !inline_ready || discovery != Some(desired.generation),
                prefix_invalidation_id: desired
                    .activation
                    .filter(|_| desired.invalidation.is_some())
                    .map(|id| id.to_string()),
                invalidated_profiles: desired.invalidation.map_or_else(Vec::new, |r| {
                    r.profiles.into_iter().map(|p| p.name).collect()
                }),
                prefix_rebuild_pending: true,
                inline_prefix_ready: inline_ready,
                inline_prefix_reference: reference,
                model_context_support:
                    "qualified-retained-inline; generated-prefix-serving-unavailable".into(),
            },
        })
    }
}
