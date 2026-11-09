//! Read-only projection of the existing catalogue owner's exact sources.
//! No row scanning, latest lookup, authoring verdict, publication or Tool call.
use std::collections::{BTreeMap, BTreeSet};

use brassclaw_product_workflow::{
    CatalogueSourceComponent, CatalogueSourceExport, ComponentRevisionSelection, RecipeStoreError,
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef, component_revision::RetainedComponentRevision,
    revision_store::PgComponentRevisionStore,
};
use sha2::{Digest, Sha256};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

use crate::installed_monty_catalogue::InstalledMontyCatalogue;

// Complete serialized source-transfer capacity uses the existing acknowledged
// catalogue limit. This is independent of model/discretionary token budgets.
// Count without allocating a second complete byte buffer, and reject the whole
// export on overflow; never return a clipped record or a shortened inventory.
struct ExportSize {
    maximum: usize,
    total: usize,
    exceeded: bool,
}
impl std::io::Write for ExportSize {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let Some(total) = self
            .total
            .checked_add(bytes.len())
            .filter(|total| *total <= self.maximum)
        else {
            self.exceeded = true;
            return Err(std::io::Error::other("catalogue source capacity exceeded"));
        };
        self.total = total;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn ensure_export_capacity(
    export: &CatalogueSourceExport,
    maximum: usize,
) -> Result<(), RecipeStoreError> {
    let mut size = ExportSize {
        maximum,
        total: 0,
        exceeded: false,
    };
    match serde_json::to_writer(&mut size, export) {
        Ok(()) => Ok(()),
        Err(_) if size.exceeded => Err(RecipeStoreError::Unavailable(
            "complete catalogue export exceeds supported transport capacity".into(),
        )),
        Err(_) => Err(integrity()),
    }
}
fn integrity() -> RecipeStoreError {
    RecipeStoreError::Internal("catalogue source integrity verification failed".into())
}
fn reference(r: ComponentRevisionRef) -> ComponentRevisionSelection {
    ComponentRevisionSelection {
        uuid: r.uuid.to_string(),
        class_code: r.class_code,
        version: r.version,
        checksum: hex::encode(r.checksum),
    }
}
fn collect(
    records: &mut BTreeMap<(Uuid, u64), CatalogueSourceComponent>,
    revision: &RetainedComponentRevision,
    allow_versions: bool,
) -> Result<(), RecipeStoreError> {
    let r = revision.reference();
    if records
        .range((r.uuid, 0)..=(r.uuid, u64::MAX))
        .any(|(_, old)| {
            old.reference.class_code != r.class_code
                || (!allow_versions && old.reference.version != r.version)
        })
    {
        return Err(integrity());
    }
    if let Some(old) = records.get(&(r.uuid, r.version)) {
        if old.reference.version != r.version
            || old.reference.checksum != hex::encode(r.checksum)
            || old.reference.class_code != r.class_code
            || old.revision_bytes != revision.draft().exact_bytes()
        {
            return Err(integrity());
        }
    } else {
        records.insert(
            (r.uuid, r.version),
            CatalogueSourceComponent {
                reference: reference(r),
                revision_bytes: revision.draft().exact_bytes().to_owned(),
            },
        );
    }
    Ok(())
}
impl InstalledMontyCatalogue {
    pub(crate) async fn export_sources(
        &self,
        generation: &str,
    ) -> Result<CatalogueSourceExport, RecipeStoreError> {
        self.export_source_scope(generation, false).await
    }

    /// The two qualified private prefix workflows are source references, not
    /// newly advertised routing entries or execution authority. Different pinned
    /// revisions may coexist across independent approved workflow selections.
    pub(crate) async fn export_prefix_workflow_sources(
        &self,
        generation: &str,
    ) -> Result<CatalogueSourceExport, RecipeStoreError> {
        self.export_source_scope(generation, true).await
    }

    async fn export_source_scope(
        &self,
        generation: &str,
        prefix_workflows: bool,
    ) -> Result<CatalogueSourceExport, RecipeStoreError> {
        let expected = Uuid::parse_str(generation)
            .ok()
            .filter(|g| !g.is_nil() && g.to_string() == generation)
            .ok_or_else(|| {
                RecipeStoreError::Invalid("canonical non-nil catalogue generation required".into())
            })?;
        // Capture once per new export, before holding catalogue activation/SQL
        // locks. The source is the same acknowledged settings owner as the UI;
        // a desired but failed/unpublished edit must not change this operation.
        let limits = self.preparation_policy().await.map_err(|_| {
            RecipeStoreError::Unavailable("catalogue source settings unavailable".into())
        })?;
        let maximum = usize::try_from(limits.max_catalogue_bytes).map_err(|_| integrity())?;
        let _serial = self.activation.lock().await;
        self.reconcile_committed_runtime_locked().await?;
        let view = self.active.read().await.clone();
        if expected != view.generation {
            return Err(RecipeStoreError::Conflict(
                "catalogue generation changed; select and export again".into(),
            ));
        }
        let mut client =
            self.pool.get().await.map_err(|_| {
                RecipeStoreError::Unavailable("catalogue database unavailable".into())
            })?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| integrity())?;
        self.verify_source_selection(&tx, &view)
            .await
            .map_err(|_| integrity())?;
        // The root is retained separately from usage approvals. Exporting its
        // pinned source never promotes its pending review into an approval.
        let root = PgComponentRevisionStore::read_exact_in_transaction(
            &tx,
            &[self.root_reference.uuid],
            &[self.root_reference],
            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
        )
        .await
        .map_err(|_| integrity())?;
        let mut records = BTreeMap::new();
        for revision in root.revisions().values() {
            collect(&mut records, revision, prefix_workflows)?;
        }
        let mut selections = BTreeSet::new();
        let mut private = Vec::new();
        if prefix_workflows {
            let compiler = self._compiler_program.as_ref().ok_or_else(|| {
                RecipeStoreError::Unavailable(
                    "source compiler qualification unavailable for export".into(),
                )
            })?;
            self.compiler_approval
                .as_ref()
                .ok_or_else(integrity)?
                .require_selected_program(compiler)
                .map_err(|_| integrity())?;
            private.push(compiler.clone());
            private.push(
                self.measurement_qualification
                    .as_ref()
                    .ok_or_else(|| {
                        RecipeStoreError::Unavailable(
                            "measurement qualification unavailable for export".into(),
                        )
                    })?
                    .program(),
            );
        }
        for selected in std::iter::once(&self.reply)
            .chain(std::iter::once(&self.history))
            .chain(view.programs.iter())
            .chain(private.iter())
        {
            let instruction = selected.program().inputs().instruction();
            for revision in instruction.snapshot().revisions().values() {
                collect(&mut records, revision, prefix_workflows)?;
            }
            selections.insert(
                instruction
                    .retained_selection()
                    .map_err(|_| integrity())?
                    .exact_bytes()
                    .to_owned(),
            );
        }
        let export = CatalogueSourceExport {
            format: if prefix_workflows {
                "brassclaw-catalogue-source-export/2"
            } else {
                "brassclaw-catalogue-source-export/1"
            }
            .into(),
            generation: view.generation.to_string(),
            coverage: if prefix_workflows {
                "active-ordinary-plus-source-compiler-and-measurement-with-retained-root"
            } else {
                "active-ordinary-workflows-and-retained-root"
            }
            .into(),
            catalogue_bytes: view.bytes.clone(),
            catalogue_checksum: hex::encode(Sha256::digest(view.bytes.as_bytes())),
            root: reference(self.root_reference),
            components: records.into_values().collect(),
            selections: selections.into_iter().collect(),
        };
        // Serde escaping is part of the actual transfer size, unlike raw source
        // sums. This limit is deliberately checked on the complete projection.
        ensure_export_capacity(&export, maximum)?;
        tx.commit().await.map_err(|_| integrity())?;
        Ok(export)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_skills::component_revision::ComponentRevisionDraft;
    use serde_json::json;

    #[test]
    fn versioned_source_records_preserve_each_revision_and_reject_conflicts() {
        // Pure source-transfer records, not qualification or execution evidence.
        let uuid = Uuid::from_u128(911);
        let record = |class, version, text| {
            ComponentRevisionDraft::from_json(
                &json!({"format":"component-revision/1",
                "uuid":uuid,"class_code":class,"document":{"content":text},
                "dependencies":[],"association":null})
                .to_string(),
            )
            .unwrap()
            .at_version(version)
            .unwrap()
        };
        let first = record(17, 1, "first exact source\n");
        let next = record(17, 2, "second exact source\n");
        let mut expanded = BTreeMap::new();
        collect(&mut expanded, &first, true).unwrap();
        collect(&mut expanded, &next, true).unwrap();
        collect(&mut expanded, &first, true).unwrap();
        assert_eq!(expanded.len(), 2);
        assert_eq!(
            expanded[&(uuid, 1)].revision_bytes,
            first.draft().exact_bytes()
        );
        assert_eq!(
            expanded[&(uuid, 2)].revision_bytes,
            next.draft().exact_bytes()
        );
        assert!(
            collect(
                &mut expanded,
                &record(17, 1, "conflicting first source"),
                true
            )
            .is_err()
        );
        assert!(collect(&mut expanded, &record(18, 3, "class changed"), true).is_err());
        let mut ordinary = BTreeMap::new();
        collect(&mut ordinary, &first, false).unwrap();
        assert!(collect(&mut ordinary, &next, false).is_err());
        assert_eq!(ordinary.len(), 1);
    }

    #[cfg(feature = "root-llm-provider")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_source_export_uses_acknowledged_catalogue_limit_without_token_budget_bypass() {
        use brassclaw_host_api::runtime_policy::*;
        use brassclaw_product_workflow::{MontyMemoryMode, MontyVmSettingsStore};
        let rig = crate::runtime::test_pg::pg_rig().await;
        rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
        let directory = tempfile::tempdir().unwrap();
        let runtime = Box::pin(crate::build_reborn_runtime(
            crate::RebornRuntimeInput::from_services(
                rig.build_input("source-export-capacity-owner", directory.path())
                    .with_runtime_policy(EffectiveRuntimePolicy {
                        deployment: DeploymentMode::LocalSingleUser,
                        requested_profile: RuntimeProfile::LocalDev,
                        resolved_profile: RuntimeProfile::LocalDev,
                        filesystem_backend: FilesystemBackendKind::HostWorkspace,
                        process_backend: ProcessBackendKind::LocalHost,
                        network_mode: NetworkMode::DirectLogged,
                        secret_mode: SecretMode::ScrubbedEnv,
                        approval_policy: ApprovalPolicy::AskDestructive,
                        audit_mode: AuditMode::LocalMinimal,
                    }),
            ),
        ))
        .await
        .unwrap();
        let catalogue = runtime.installed_catalogue.clone();
        let generation = catalogue.active.read().await.generation.to_string();
        let settings = runtime.webui_monty_settings_store();
        let initial = settings.get("default", "default").await.unwrap();
        assert!(!initial.token_budgets_enabled);
        let baseline = catalogue.export_sources(&generation).await.unwrap();
        let bytes = serde_json::to_vec(&baseline).unwrap();
        assert!(!baseline.components.is_empty() && !baseline.selections.is_empty());
        assert!(
            bytes.len() > 1 && bytes.len() < initial.execution_limits.max_catalogue_bytes as usize
        );
        let mut limits = initial.execution_limits;
        limits.max_catalogue_bytes = bytes.len() as u64;
        let exact = settings
            .upsert(
                "default",
                "default",
                &serde_json::from_value(json!({
                    "expected_revision":initial.revision,"execution_limits":limits,
                }))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            catalogue
                .preparation_policy()
                .await
                .unwrap()
                .max_catalogue_bytes,
            bytes.len() as u64
        );
        assert_eq!(
            serde_json::to_vec(&catalogue.export_sources(&generation).await.unwrap()).unwrap(),
            bytes
        );
        limits.max_catalogue_bytes -= 1;
        let smaller = settings
            .upsert(
                "default",
                "default",
                &serde_json::from_value(json!({
                    "expected_revision":exact.revision,"execution_limits":limits,
                }))
                .unwrap(),
            )
            .await
            .unwrap();
        assert!(!smaller.token_budgets_enabled);
        assert!(
            matches!(catalogue.export_sources(&generation).await,Err(RecipeStoreError::Unavailable(reason)) if reason.contains("capacity"))
        );
        let enabled = settings
            .upsert(
                "default",
                "default",
                &serde_json::from_value(json!({
                    "expected_revision":smaller.revision,"token_budgets_enabled":true,
                }))
                .unwrap(),
            )
            .await
            .unwrap();
        assert!(enabled.token_budgets_enabled);
        assert!(
            matches!(catalogue.export_sources(&generation).await,Err(RecipeStoreError::Unavailable(reason)) if reason.contains("capacity"))
        );
        // Raising the existing value restores the entire identical projection;
        // no persisted second cap, truncation or content rewrite is involved.
        let restored = settings.upsert("default", "default", &serde_json::from_value(json!({
            "expected_revision":enabled.revision,"execution_limits":initial.execution_limits,
            "token_budgets_enabled":initial.token_budgets_enabled,
        })).unwrap()).await.unwrap();
        assert_eq!(
            serde_json::to_vec(&catalogue.export_sources(&generation).await.unwrap()).unwrap(),
            bytes
        );
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE FUNCTION reject_export_limit_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.execution_limits->'max_catalogue_bytes' IS DISTINCT FROM OLD.execution_limits->'max_catalogue_bytes' THEN RAISE EXCEPTION 'deliberate export limit commit rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_export_limit_fixture BEFORE UPDATE ON reborn_monty_vm_settings FOR EACH ROW EXECUTE FUNCTION reject_export_limit_fixture()").await.unwrap();
        drop(client);
        let rejected = serde_json::from_value(
            json!({"expected_revision":restored.revision,"execution_limits":limits}),
        )
        .unwrap();
        assert!(
            settings
                .upsert("default", "default", &rejected)
                .await
                .is_err()
        );
        assert_eq!(
            settings.get("default", "default").await.unwrap().revision,
            restored.revision
        );
        assert_eq!(
            catalogue
                .preparation_policy()
                .await
                .unwrap()
                .max_catalogue_bytes,
            initial.execution_limits.max_catalogue_bytes
        );
        assert_eq!(
            serde_json::to_vec(&catalogue.export_sources(&generation).await.unwrap()).unwrap(),
            bytes
        );
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("DROP TRIGGER reject_export_limit_fixture ON reborn_monty_vm_settings; DROP FUNCTION reject_export_limit_fixture()").await.unwrap();
        drop(client);
        let persisted = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
            rig.pool.clone(),
            "default",
            "default",
        )
        .get("default", "default")
        .await
        .unwrap();
        assert_eq!(persisted.revision, restored.revision);
        assert_eq!(
            persisted.execution_limits.max_catalogue_bytes,
            initial.execution_limits.max_catalogue_bytes
        );
        assert_eq!(
            catalogue.active.read().await.generation.to_string(),
            generation
        );
        drop(settings);
        drop(catalogue);
        runtime.shutdown().await.unwrap();
    }
}
