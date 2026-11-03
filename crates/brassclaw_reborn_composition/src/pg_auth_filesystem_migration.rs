//! One-time installation migration from the actual PostgreSQL filesystem owner.
//! Source records stay immutable and readable. No account/flow IDs, secret
//! handles, original requests, timestamps or historical bytes are regenerated.
use super::*;
use crate::auth::RebornAuthCustodyState;
use crate::product_auth_durable::paths;
use brassclaw_auth::{CredentialRefreshAction, CredentialRefreshId};
use brassclaw_host_api::ScopedPath;
use std::collections::BTreeMap;

fn conflict() -> AuthProductError {
    AuthProductError::BackendConflict
}
fn value<T: DeserializeOwned>(data: &serde_json::Value) -> Result<T, AuthProductError> {
    serde_json::from_value(data.clone()).map_err(|_| conflict())
}
fn source_json(bytes: &[u8]) -> Result<serde_json::Value, AuthProductError> {
    use brassclaw_common::json::{DuplicateKeys, JsonReadLimits, read_json};
    let text = std::str::from_utf8(bytes).map_err(|_| conflict())?;
    // Every JSON node/name/depth requires source bytes. Bound the existing
    // common reader by the actual admitted record, without a new size quota.
    let strict = read_json(
        text,
        JsonReadLimits {
            max_depth: bytes.len(),
            max_nodes: bytes.len(),
            max_bytes: bytes.len(),
        },
        DuplicateKeys::Reject,
    )
    .map_err(|_| conflict())?;
    // Keep the former Value reader's technical recursion/numeric admission
    // unchanged before existing Clone/SQL consumers take this tree. This pass
    // grants nothing: the only returned tree is the duplicate-rejecting owner's.
    let _: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| conflict())?;
    Ok(strict.into_value())
}
fn canonical(
    scope: &AuthProductScope,
    path: &ScopedPath,
    stored: &str,
    tenant: &str,
) -> Result<(), AuthProductError> {
    if scope.resource.tenant_id.as_str() != tenant
        || crate::invocation_mount_view(&scope.resource)
            .map_err(|_| conflict())?
            .resolve(path)
            .map_err(|_| conflict())?
            .as_str()
            != stored
    {
        return Err(conflict());
    }
    Ok(())
}
async fn import_record(
    tx: &tokio_postgres::Transaction<'_>,
    table: &str,
    scope: &AuthProductScope,
    id: &str,
    data: &serde_json::Value,
) -> Result<(), AuthProductError> {
    // Only this module's constant table names reach this query. Existing native
    // rows must agree exactly; migration never overwrites a newer native subject.
    let query = format!(
        "INSERT INTO {table} (tenant_id,user_id,id,data) VALUES ($1,$2,$3,$4) ON CONFLICT (tenant_id,id) DO NOTHING"
    );
    tx.execute(
        &query,
        &[
            &scope.resource.tenant_id.as_str(),
            &scope.resource.user_id.as_str(),
            &id,
            &data,
        ],
    )
    .await
    .map_err(db_err)?;
    let query = format!("SELECT user_id,data FROM {table} WHERE tenant_id=$1 AND id=$2 FOR UPDATE");
    let row = tx
        .query_one(&query, &[&scope.resource.tenant_id.as_str(), &id])
        .await
        .map_err(db_err)?;
    if row.get::<_, String>(0) != scope.resource.user_id.as_str()
        || row.get::<_, serde_json::Value>(1) != *data
    {
        return Err(conflict());
    }
    Ok(())
}

async fn import_refresh_record(
    tx: &tokio_postgres::Transaction<'_>,
    tenant: &str,
    action: &CredentialRefreshAction,
) -> Result<(), AuthProductError> {
    let json = serde_json::to_value(action).map_err(|_| conflict())?;
    let scope_ref = action.original.scope_ref()?;
    let id = action.original.id.to_string();
    let account_id = action.original.account_id.to_string();
    let refresh = action
        .expected_account
        .refresh_secret
        .as_ref()
        .map(|h| h.as_str());
    let blocks = action
        .expected_account
        .refresh_secret
        .as_ref()
        .is_some_and(|h| action.blocks_credential(h));
    tx.execute("INSERT INTO brassclaw_product_auth_refreshes (tenant_id,scope_ref,id,account_id,refresh_handle,blocks_refresh,data) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (tenant_id,scope_ref,id) DO NOTHING",
        &[&tenant,&scope_ref,&id,&account_id,&refresh,&blocks,&json]).await.map_err(db_err)?;
    let found=tx.query_one("SELECT account_id,refresh_handle,blocks_refresh,data FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1 AND scope_ref=$2 AND id=$3 FOR UPDATE",&[&tenant,&scope_ref,&id]).await.map_err(db_err)?;
    if found.get::<_, String>(0) != account_id
        || found.get::<_, Option<String>>(1).as_deref() != refresh
        || found.get::<_, bool>(2) != blocks
        || found.get::<_, serde_json::Value>(3) != json
    {
        return Err(conflict());
    }
    Ok(())
}

impl PgAuthProductServices {
    /// Before exposing the default-tenant native Auth owner. The table lock also
    /// fences old filesystem processes: their queued writes resume behind the
    /// installed trigger and fail, rather than publishing into an abandoned store.
    pub(crate) async fn migrate_filesystem_auth(
        &self,
        tenant: &str,
    ) -> Result<RebornAuthCustodyState, AuthProductError> {
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client.transaction().await.map_err(db_err)?;
        tx.batch_execute("LOCK TABLE root_filesystem_entries, brassclaw_product_auth_accounts, brassclaw_product_auth_flows, brassclaw_product_auth_interactions, brassclaw_product_auth_requirement_selections, brassclaw_product_auth_refreshes IN SHARE ROW EXCLUSIVE MODE")
            .await
            .map_err(db_err)?;
        tx.batch_execute("DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgrelid='root_filesystem_entries'::regclass AND tgname='brassclaw_retained_auth_custody') THEN CREATE TRIGGER brassclaw_retained_auth_custody BEFORE INSERT OR UPDATE OR DELETE ON root_filesystem_entries FOR EACH ROW EXECUTE FUNCTION brassclaw_fence_retained_auth_filesystem(); END IF; END $$").await.map_err(db_err)?;
        if tx.query_opt("SELECT source_checksum FROM brassclaw_product_auth_filesystem_cutovers WHERE tenant_id=$1",&[&tenant]).await.map_err(db_err)?.is_some() {
            if tx.query_opt("SELECT tenant_id FROM brassclaw_product_auth_filesystem_recoveries WHERE tenant_id=$1",&[&tenant]).await.map_err(db_err)?.is_some() { return Err(conflict()); }
            tx.query_one("SELECT brassclaw_require_product_auth_custody($1, 'native')", &[&tenant]).await.map_err(db_err)?;
            tx.commit().await.map_err(db_err)?; return Ok(RebornAuthCustodyState::Native);
        }
        // Inspect all five native stores before transient import. A pending FS
        // owner may not conceal ANY pre-existing native state, even an exact
        // copy: there is no supported partial dual-custody handoff.
        let native_present: bool = tx.query_one("SELECT EXISTS (SELECT 1 FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 UNION ALL SELECT 1 FROM brassclaw_product_auth_flows WHERE tenant_id=$1 UNION ALL SELECT 1 FROM brassclaw_product_auth_interactions WHERE tenant_id=$1 UNION ALL SELECT 1 FROM brassclaw_product_auth_requirement_selections WHERE tenant_id=$1 UNION ALL SELECT 1 FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1)",&[&tenant]).await.map_err(db_err)?.get(0);
        // Only this locked cutover transaction changes the permanent custodian
        // row. Its update fences fixed-snapshot writers as well as current ones.
        tx.execute("INSERT INTO brassclaw_product_auth_custody (tenant_id,owner) VALUES ($1,'native') ON CONFLICT (tenant_id) DO UPDATE SET owner=EXCLUDED.owner", &[&tenant]).await.map_err(db_err)?;
        // All writers are locked. Removing this fence is transaction-local:
        // failure restores it; pending reinstates it before releasing locks;
        // native commit replaces it atomically with the FS writer fence.
        tx.execute(
            "DELETE FROM brassclaw_product_auth_filesystem_recoveries WHERE tenant_id=$1",
            &[&tenant],
        )
        .await
        .map_err(db_err)?;
        tx.batch_execute("SAVEPOINT auth_filesystem_import")
            .await
            .map_err(db_err)?;
        // Identity comes from validated typed scope plus its actual mount
        // resolution below, not merely a filename or caller-supplied tenant.
        let rows=tx.query("SELECT path,contents,version FROM root_filesystem_entries WHERE NOT is_dir AND split_part(path,'/',3)=$1 AND path LIKE '/tenants/%/users/%/secrets/%/product-auth/%' ORDER BY path",&[&tenant]).await.map_err(db_err)?;
        let mut source = sha2::Sha256::new();
        use sha2::Digest as _;
        let mut accounts: BTreeMap<
            CredentialAccountId,
            (
                CredentialAccount,
                BTreeMap<CredentialRefreshId, CredentialRefreshAction>,
            ),
        > = BTreeMap::new();
        let mut refreshes = Vec::new();
        let mut selections = Vec::new();
        let mut recovery_pending = false;
        for row in &rows {
            let path: String = row.get(0);
            let bytes: Vec<u8> = row.get(1);
            let version: i64 = row.get(2);
            source.update((path.len() as u64).to_be_bytes());
            source.update(path.as_bytes());
            source.update(version.to_be_bytes());
            source.update((bytes.len() as u64).to_be_bytes());
            source.update(&bytes);
            let data = source_json(&bytes)?;
            let parent = path.rsplit('/').nth(1).ok_or_else(conflict)?;
            if !matches!(
                parent,
                "accounts" | "flows" | "interactions" | "requirement-selections"
            ) && path.contains("/refresh-actions/")
            {
                let action: CredentialRefreshAction = value(&data)?;
                canonical(
                    &action.original.scope,
                    &paths::refresh_action_path(&action.original)?,
                    &path,
                    tenant,
                )?;
                refreshes.push(action);
            } else if parent == "accounts" {
                let account: CredentialAccount = value(&data)?;
                canonical(
                    &account.scope,
                    &paths::account_path(&account.scope, account.id),
                    &path,
                    tenant,
                )?;
                let journal: BTreeMap<CredentialRefreshId, CredentialRefreshAction> = data
                    .get("auth_refresh_actions")
                    .map(value)
                    .transpose()?
                    .unwrap_or_default();
                if accounts
                    .insert(account.id, (account.clone(), journal))
                    .is_some()
                {
                    return Err(conflict());
                }
                import_record(
                    &tx,
                    "brassclaw_product_auth_accounts",
                    &account.scope,
                    &account.id.to_string(),
                    &data,
                )
                .await?;
            } else if parent == "flows" {
                let document: crate::auth_flow_custody::StoredAuthFlow = value(&data)?;
                canonical(
                    &document.flow.scope,
                    &paths::flow_path(&document.flow.scope, document.flow.id),
                    &path,
                    tenant,
                )?;
                // A CAS-only decision still awaiting its account receipt is not
                // a native publication. Preserve everything and require original
                // filesystem reconciliation before this installation cutover.
                if document.fs_callback_publication.is_some()
                    || matches!(
                        document.flow.status,
                        AuthFlowStatus::Completing | AuthFlowStatus::CallbackReceived
                    )
                    || (!is_terminal_status(document.flow.status)
                        && document.auth_custody.dcr_registration.is_some())
                {
                    // A live old process can still receive an external return.
                    // Cut over only after that original has reconciled; never
                    // strand its late result behind the source writer fence.
                    recovery_pending = true;
                }
                import_record(
                    &tx,
                    "brassclaw_product_auth_flows",
                    &document.flow.scope,
                    &document.flow.id.to_string(),
                    &data,
                )
                .await?;
            } else if parent == "interactions" {
                let interaction: StoredManualTokenInteraction = value(&data)?;
                canonical(
                    &interaction.scope,
                    &paths::interaction_path(&interaction.scope, interaction.id),
                    &path,
                    tenant,
                )?;
                if data.get("publication").is_some_and(|p| !p.is_null())
                    && interaction.consumed_at.is_none()
                {
                    recovery_pending = true;
                }
                import_record(
                    &tx,
                    "brassclaw_product_auth_interactions",
                    &interaction.scope,
                    &interaction.id.to_string(),
                    &data,
                )
                .await?;
            } else if parent == "requirement-selections" {
                let selected: brassclaw_auth::CredentialRequirementSelection = value(&data)?;
                let scope = selected.original_scope();
                let key = selected.key()?;
                canonical(
                    scope,
                    &paths::requirement_selection_path(scope, &key),
                    &path,
                    tenant,
                )?;
                import_record(
                    &tx,
                    "brassclaw_product_auth_requirement_selections",
                    scope,
                    key.as_str(),
                    &data,
                )
                .await?;
                selections.push(selected);
            } else {
                return Err(conflict());
            }
        }
        for selected in selections {
            let row = tx
                .query_opt(
                    "SELECT data FROM brassclaw_product_auth_flows WHERE tenant_id=$1 AND id=$2",
                    &[&tenant, &selected.original_flow().to_string()],
                )
                .await
                .map_err(db_err)?
                .ok_or_else(conflict)?;
            let document: crate::auth_flow_custody::StoredAuthFlow = value(&row.get(0))?;
            if document
                .auth_custody
                .original_requirement_selection(
                    selected.original_scope(),
                    selected.original_flow(),
                    &document.flow,
                    selected.requirement(),
                )?
                .as_ref()
                != Some(&selected)
            {
                return Err(conflict());
            }
        }
        let mut imported = std::collections::BTreeSet::new();
        for locator in refreshes {
            let (account, journal) = accounts
                .get(&locator.original.account_id)
                .ok_or_else(conflict)?;
            let request = locator.original.request();
            let action = crate::product_auth_durable::refresh::selected_original(
                &request,
                Some(locator),
                journal.get(&request.id),
            )?
            .ok_or_else(conflict)?;
            if account.scope != action.expected_account.scope {
                return Err(conflict());
            }
            if matches!(
                action.outcome,
                brassclaw_auth::CredentialRefreshOutcome::Submitted
            ) {
                recovery_pending = true;
            }
            import_refresh_record(&tx, tenant, &action).await?;
            if !imported.insert((action.original.account_id, action.original.id)) {
                return Err(conflict());
            }
        }
        for (id, (_, journal)) in &accounts {
            for action in journal.keys() {
                if !imported.contains(&(*id, *action)) {
                    return Err(conflict());
                }
            }
        }
        if recovery_pending {
            if native_present {
                return Err(conflict());
            }
            // Every original, index and cross-reference has now been checked.
            // Discard the transient import, then persist ONLY the native writer
            // fence. The original filesystem remains the exclusive custodian.
            tx.batch_execute("ROLLBACK TO SAVEPOINT auth_filesystem_import")
                .await
                .map_err(db_err)?;
            tx.execute(
                "INSERT INTO brassclaw_product_auth_filesystem_recoveries (tenant_id) VALUES ($1)",
                &[&tenant],
            )
            .await
            .map_err(db_err)?;
            tx.execute(
                "UPDATE brassclaw_product_auth_custody SET owner='filesystem' WHERE tenant_id=$1",
                &[&tenant],
            )
            .await
            .map_err(db_err)?;
            tx.commit().await.map_err(db_err)?;
            return Ok(RebornAuthCustodyState::FilesystemRecoveryPending);
        }
        let checksum = format!("{:x}", source.finalize());
        let count = i64::try_from(rows.len()).map_err(|_| conflict())?;
        tx.execute("INSERT INTO brassclaw_product_auth_filesystem_cutovers (tenant_id,source_checksum,source_records) VALUES ($1,$2,$3)",&[&tenant,&checksum,&count]).await.map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(RebornAuthCustodyState::Native)
    }
}

#[cfg(all(test, feature = "skills-db"))]
mod tests {
    use super::*;
    use brassclaw_auth::{AuthContinuationRef, AuthGateRef, AuthSurface, TurnRunRef};
    use brassclaw_filesystem::{CasExpectation, Entry};
    use brassclaw_host_api::{InvocationId, ResourceScope, TenantId, UserId};

    #[tokio::test]
    async fn native_default_auth_migration_retains_accounts_flows_interactions_and_fences_old_writers()
     {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
            rig.pool.as_ref().clone(),
        ));
        root.run_migrations().await.unwrap();
        let filesystem = crate::wrap_scoped(root.clone());
        let secrets = Arc::new(
            PgSecretStore::new(
                rig.pool.as_ref().clone(),
                brassclaw_secrets::keychain::generate_master_key_hex().into(),
                "reborn-cli",
            )
            .unwrap(),
        );
        let legacy = crate::product_auth_durable::FilesystemAuthProductServices::new(
            filesystem.clone(),
            secrets.clone(),
        );
        let native = PgAuthProductServices::new(rig.pool.clone(), secrets.clone());
        let scope = AuthProductScope::new(
            ResourceScope {
                tenant_id: TenantId::new("reborn-cli").unwrap(),
                user_id: UserId::new("original-installer").unwrap(),
                agent_id: None,
                project_id: None,
                thread_id: None,
                invocation_id: InvocationId::new(),
            },
            AuthSurface::SetupAdmin,
        );
        let account = legacy
            .create_account(NewCredentialAccount {
                scope: scope.clone(),
                provider: AuthProviderId::new("google").unwrap(),
                label: CredentialAccountLabel::new("Existing reusable account").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: vec![],
                access_secret: Some(SecretHandle::new("retained-api-access").unwrap()),
                access_expires_at: None,
                refresh_secret: Some(SecretHandle::new("retained-refresh-input").unwrap()),
                scopes: vec![],
            })
            .await
            .unwrap();
        let refresh = account.refresh_secret.clone().unwrap();
        secrets
            .put(
                scope.resource.clone(),
                refresh.clone(),
                "migration-only-original-refresh".into(),
            )
            .await
            .unwrap();
        let reference = secrets
            .record_reference(&scope.resource, &refresh)
            .await
            .unwrap()
            .unwrap();
        let request = brassclaw_auth::CredentialRefreshRequest::new(
            scope.clone(),
            account.provider.clone(),
            account.id,
        );
        let admission = legacy
            .begin_refresh(brassclaw_auth::CredentialRefreshPrepared {
                original: brassclaw_auth::CredentialRefreshOriginal::from(&request),
                expected_account: account.clone(),
                preparation: Some(brassclaw_auth::OAuthProviderRefreshPreparation {
                    destinations: brassclaw_auth::OAuthTokenDestinations {
                        access_secret: SecretHandle::new("unused-migration-access-destination")
                            .unwrap(),
                        refresh_secret: SecretHandle::new("unused-migration-refresh-destination")
                            .unwrap(),
                    },
                    source: brassclaw_auth::OAuthRefreshSource {
                        scope: scope.resource.clone(),
                        handle: refresh,
                        record_reference: reference,
                    },
                }),
            })
            .await
            .unwrap();
        let brassclaw_auth::CredentialRefreshAdmission::Dispatch(action) = admission else {
            panic!("original filesystem admission")
        };
        // Connection A fixes its snapshot before B publishes pending custody.
        // It performs NO DML until after B commits, so table-lock serialization
        // alone cannot protect this insertion from a stale marker lookup.
        let mut stale_native_client = rig.pool.get().await.unwrap();
        let stale_native = stale_native_client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        let absent: i64 = stale_native.query_one(
            "SELECT count(*) FROM brassclaw_product_auth_filesystem_recoveries WHERE tenant_id=$1",
            &[&"reborn-cli"],
        ).await.unwrap().get(0);
        assert_eq!(absent, 0);
        // A potentially live old claim must not be stranded by cutover. These
        // are actual FS journal writes, with no provider execution or success.
        assert_eq!(
            native.migrate_filesystem_auth("reborn-cli").await.unwrap(),
            RebornAuthCustodyState::FilesystemRecoveryPending
        );
        let rejected = stale_native.execute(
            "INSERT INTO brassclaw_product_auth_accounts (tenant_id,id,user_id,data) VALUES ($1,'fixed-snapshot-native','blocked','{}'::jsonb)",
            &[&"reborn-cli"],
        ).await.unwrap_err();
        assert_eq!(
            rejected.as_db_error().map(|e| e.code().code()),
            Some("40001")
        );
        stale_native.rollback().await.unwrap();
        drop(stale_native_client);
        assert!(rig.pool.get().await.unwrap().query_opt(
            "SELECT id FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id='fixed-snapshot-native'",
            &[&"reborn-cli"],
        ).await.unwrap().is_none());
        assert!(
            native
                .read_account(&scope, account.id)
                .await
                .unwrap()
                .is_none()
        );
        // This test runner performed no transport. Retain that original stop
        // through the real FS result/account publication, then migrate it.
        let stopped = legacy
            .complete_refresh(brassclaw_auth::CredentialRefreshCompletion {
                dispatched: action,
                outcome: brassclaw_auth::CredentialRefreshOutcome::NotSubmitted(
                    brassclaw_auth::CredentialRefreshStopReason::HostPreparationFailed,
                ),
            })
            .await
            .unwrap();
        let challenge = legacy
            .request_secret_input(ManualTokenSetupRequest {
                requirement_setup: None,
                scope: scope.clone(),
                provider: account.provider.clone(),
                label: account.label.clone(),
                continuation: AuthContinuationRef::SetupOnly,
                update_binding: None,
                expires_at: Utc::now() + chrono::Duration::minutes(5),
            })
            .await
            .unwrap();
        let flow = legacy
            .create_flow(NewAuthFlow {
                requirement_setup: None,
                id: None,
                scope: scope.clone(),
                kind: brassclaw_auth::AuthFlowKind::IntegrationCredential,
                provider: account.provider.clone(),
                challenge: challenge.clone(),
                continuation: AuthContinuationRef::TurnGateResume {
                    turn_run_ref: TurnRunRef::new("original-run").unwrap(),
                    gate_ref: AuthGateRef::new("original-gate").unwrap(),
                },
                update_binding: None,
                opaque_state_hash: None,
                pkce_verifier_hash: None,
                expires_at: Utc::now() + chrono::Duration::minutes(5),
            })
            .await
            .unwrap();
        let account_path = paths::account_path(&scope, account.id);
        let original = filesystem
            .get(&scope.resource, &account_path)
            .await
            .unwrap()
            .unwrap();
        // A fixes a second snapshot while the unchanged source row still
        // belongs to FS. B then commits native cutover, changing only custody
        // and destination rows; no concurrent source-row update can accidentally
        // supply the serialization failure this test requires from the fence.
        let mut stale_fs_client = rig.pool.get().await.unwrap();
        let stale_fs = stale_fs_client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        let source_path: String = stale_fs
            .query_one(
                "SELECT path FROM root_filesystem_entries WHERE path LIKE $1",
                &[&format!("%/accounts/{}.json", account.id)],
            )
            .await
            .unwrap()
            .get(0);
        let before: String = stale_fs
            .query_one(
                "SELECT owner FROM brassclaw_product_auth_custody WHERE tenant_id=$1",
                &[&"reborn-cli"],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(before, "filesystem");
        assert_eq!(
            native.migrate_filesystem_auth("reborn-cli").await.unwrap(),
            RebornAuthCustodyState::Native
        );
        let rejected = stale_fs
            .execute(
                "UPDATE root_filesystem_entries SET contents=contents WHERE path=$1",
                &[&source_path],
            )
            .await
            .unwrap_err();
        assert_eq!(
            rejected.as_db_error().map(|e| e.code().code()),
            Some("40001")
        );
        stale_fs.rollback().await.unwrap();
        drop(stale_fs_client);
        assert_eq!(
            native.read_refresh_action(&request).await.unwrap(),
            Some(stopped)
        );
        assert_eq!(
            native
                .read_account(&scope, account.id)
                .await
                .unwrap()
                .unwrap()
                .0,
            account
        );
        assert_eq!(
            native.read_flow(&scope, flow.id).await.unwrap().unwrap().0,
            flow
        );
        let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
            panic!("manual challenge")
        };
        assert!(
            native
                .read_interaction(&scope, interaction_id)
                .await
                .unwrap()
                .is_some()
        );
        let retained = filesystem
            .get(&scope.resource, &account_path)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.entry.body, original.entry.body);
        assert_eq!(retained.version, original.version);
        assert!(
            filesystem
                .put(
                    &scope.resource,
                    &account_path,
                    Entry::bytes(original.entry.body),
                    CasExpectation::Any
                )
                .await
                .is_err()
        );
        let next = paths::account_path(&scope, CredentialAccountId::new());
        assert!(
            filesystem
                .put(
                    &scope.resource,
                    &next,
                    Entry::bytes(b"{}".to_vec()),
                    CasExpectation::Absent
                )
                .await
                .is_err()
        );
        // Restart/readback does not import archived bytes over newer native state.
        native
            .update_status(&scope, account.id, CredentialAccountStatus::Revoked)
            .await
            .unwrap();
        native.migrate_filesystem_auth("reborn-cli").await.unwrap();
        assert_eq!(
            native
                .read_account(&scope, account.id)
                .await
                .unwrap()
                .unwrap()
                .0
                .status,
            CredentialAccountStatus::Revoked
        );
    }
}

#[cfg(all(test, feature = "skills-db"))]
mod conflict_tests {
    use super::*;
    use brassclaw_filesystem::{CasExpectation, Entry, RootFilesystem};
    use brassclaw_host_api::VirtualPath;
    #[tokio::test]
    async fn native_auth_migration_rejects_duplicate_original_keys_and_retains_exact_bytes() {
        use brassclaw_auth::{AuthContinuationRef, AuthSurface};
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
            rig.pool.as_ref().clone(),
        ));
        root.run_migrations().await.unwrap();
        let fs = crate::wrap_scoped(root);
        let secrets = Arc::new(
            PgSecretStore::new(
                rig.pool.as_ref().clone(),
                brassclaw_secrets::keychain::generate_master_key_hex().into(),
                "reborn-cli",
            )
            .unwrap(),
        );
        let old = crate::product_auth_durable::FilesystemAuthProductServices::new(
            fs.clone(),
            secrets.clone(),
        );
        let native = PgAuthProductServices::new(rig.pool.clone(), secrets);
        let scope = AuthProductScope::new(
            brassclaw_host_api::ResourceScope {
                tenant_id: brassclaw_host_api::TenantId::new("reborn-cli").unwrap(),
                user_id: brassclaw_host_api::UserId::new("zz-original").unwrap(),
                agent_id: None,
                project_id: None,
                thread_id: None,
                invocation_id: brassclaw_host_api::InvocationId::new(),
            },
            AuthSurface::SetupAdmin,
        );
        let new_account = |scope| NewCredentialAccount {
            scope,
            provider: AuthProviderId::new("google").unwrap(),
            label: CredentialAccountLabel::new("Parser custody fixture").unwrap(),
            status: CredentialAccountStatus::PendingSetup,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: None,
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        };
        let mut earlier_scope = scope.clone();
        earlier_scope.resource.user_id = brassclaw_host_api::UserId::new("aa-earlier").unwrap();
        let earlier = old
            .create_account(new_account(earlier_scope.clone()))
            .await
            .unwrap();
        let account = old
            .create_account(new_account(scope.clone()))
            .await
            .unwrap();
        let challenge = old
            .request_secret_input(ManualTokenSetupRequest {
                requirement_setup: None,
                scope: scope.clone(),
                provider: account.provider.clone(),
                label: account.label.clone(),
                continuation: AuthContinuationRef::SetupOnly,
                update_binding: None,
                expires_at: Utc::now() + chrono::Duration::minutes(5),
            })
            .await
            .unwrap();
        let flow = old
            .create_flow(NewAuthFlow {
                requirement_setup: None,
                id: None,
                scope: scope.clone(),
                kind: brassclaw_auth::AuthFlowKind::IntegrationCredential,
                provider: account.provider.clone(),
                challenge,
                continuation: AuthContinuationRef::SetupOnly,
                update_binding: None,
                opaque_state_hash: None,
                pkce_verifier_hash: None,
                expires_at: Utc::now() + chrono::Duration::minutes(5),
            })
            .await
            .unwrap();
        let account_path = paths::account_path(&scope, account.id);
        let flow_path = paths::flow_path(&scope, flow.id);
        let account_bytes = fs
            .get(&scope.resource, &account_path)
            .await
            .unwrap()
            .unwrap()
            .entry
            .body;
        let flow_bytes = fs
            .get(&scope.resource, &flow_path)
            .await
            .unwrap()
            .unwrap()
            .entry
            .body;
        let account_text = std::str::from_utf8(&account_bytes).unwrap();
        let flow_text = std::str::from_utf8(&flow_bytes).unwrap();
        let user = format!(
            "\"user_id\":{}",
            serde_json::to_string(&scope.resource.user_id).unwrap()
        );
        assert_eq!(account_text.matches(&user).count(), 1);
        assert!(!flow_text.contains("\"auth_custody\""));
        let cases = [
            (
                account_path.clone(),
                format!("{{\"id\":\"{}\",{}", account.id, &account_text[1..]),
                false,
                account_bytes.clone(),
            ),
            (
                account_path.clone(),
                account_text.replace(&user, &format!("\"user_id\":\"conflicting-user\",{user}")),
                false,
                account_bytes.clone(),
            ),
            (
                flow_path,
                format!(
                    "{{\"auth_custody\":{{\"oauth_callback\":null,\"oauth_callback\":null}},{}",
                    &flow_text[1..]
                ),
                true,
                flow_bytes,
            ),
        ];
        for (path, ambiguous, is_flow, original) in cases {
            // These are ambiguous actual record fields, not malformed JSON or
            // a fabricated provider result. Direct typed readers reject them.
            if is_flow {
                assert!(
                    serde_json::from_str::<crate::auth_flow_custody::StoredAuthFlow>(&ambiguous)
                        .is_err()
                );
            } else {
                assert!(serde_json::from_str::<CredentialAccount>(&ambiguous).is_err());
            }
            fs.put(
                &scope.resource,
                &path,
                Entry::bytes(ambiguous.as_bytes().to_vec()),
                CasExpectation::Any,
            )
            .await
            .unwrap();
            let before = fs.get(&scope.resource, &path).await.unwrap().unwrap();
            assert_eq!(
                native
                    .migrate_filesystem_auth("reborn-cli")
                    .await
                    .unwrap_err(),
                AuthProductError::BackendConflict
            );
            let after = fs.get(&scope.resource, &path).await.unwrap().unwrap();
            assert_eq!(after.entry.body, ambiguous.as_bytes());
            assert_eq!(after.entry.body, before.entry.body);
            assert_eq!(after.version, before.version);
            assert!(
                native
                    .read_account(&earlier_scope, earlier.id)
                    .await
                    .unwrap()
                    .is_none(),
                "earlier transient imports roll back"
            );
            let count: i64 = rig.pool.get().await.unwrap().query_one(
                "SELECT (SELECT count(*) FROM brassclaw_product_auth_accounts) + (SELECT count(*) FROM brassclaw_product_auth_flows) + (SELECT count(*) FROM brassclaw_product_auth_interactions) + (SELECT count(*) FROM brassclaw_product_auth_refreshes) + (SELECT count(*) FROM brassclaw_product_auth_requirement_selections) + (SELECT count(*) FROM brassclaw_product_auth_custody) + (SELECT count(*) FROM brassclaw_product_auth_filesystem_cutovers) + (SELECT count(*) FROM brassclaw_product_auth_filesystem_recoveries)", &[],
            ).await.unwrap().get(0);
            assert_eq!(count, 0);
            fs.put(
                &scope.resource,
                &path,
                Entry::bytes(original),
                CasExpectation::Any,
            )
            .await
            .unwrap();
        }
        // JSON-looking source/response strings stay opaque data. The strict
        // owner must not recursively parse their deliberately duplicated keys.
        let embedded = r#"{"id":"first","id":"second","source":"print('data')"}"#;
        let with_text = format!(
            "{{\"historical_source_text\":{},{}",
            serde_json::to_string(embedded).unwrap(),
            &account_text[1..]
        );
        fs.put(
            &scope.resource,
            &account_path,
            Entry::bytes(with_text.as_bytes().to_vec()),
            CasExpectation::Any,
        )
        .await
        .unwrap();
        assert_eq!(
            native.migrate_filesystem_auth("reborn-cli").await.unwrap(),
            RebornAuthCustodyState::Native
        );
        let stored: serde_json::Value = rig
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT data FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2",
                &[&"reborn-cli", &account.id.to_string()],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(stored["historical_source_text"], embedded);
        assert_eq!(
            fs.get(&scope.resource, &account_path)
                .await
                .unwrap()
                .unwrap()
                .entry
                .body,
            with_text.as_bytes()
        );
    }
    #[tokio::test]
    async fn native_auth_migration_malformed_original_rolls_back_without_cutover_or_source_loss() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let fs = brassclaw_filesystem::PostgresRootFilesystem::new(rig.pool.as_ref().clone());
        fs.run_migrations().await.unwrap();
        let path = VirtualPath::new(
            "/tenants/reborn-cli/users/operator/secrets/scope/product-auth/web/flows/invalid.json",
        )
        .unwrap();
        fs.put(
            &path,
            Entry::bytes(b"malformed original".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
        let auth = PgAuthProductServices::new(
            rig.pool.clone(),
            Arc::new(
                PgSecretStore::new(
                    rig.pool.as_ref().clone(),
                    brassclaw_secrets::keychain::generate_master_key_hex().into(),
                    "reborn-cli",
                )
                .unwrap(),
            ),
        );
        assert!(auth.migrate_filesystem_auth("reborn-cli").await.is_err());
        assert_eq!(
            fs.get(&path).await.unwrap().unwrap().entry.body,
            b"malformed original"
        );
        assert!(rig.pool.get().await.unwrap().query_opt("SELECT tenant_id FROM brassclaw_product_auth_filesystem_cutovers WHERE tenant_id=$1",&[&"reborn-cli"]).await.unwrap().is_none());
        fs.put(
            &path,
            Entry::bytes(b"still owned by filesystem".to_vec()),
            CasExpectation::Any,
        )
        .await
        .unwrap();
    }
}

#[cfg(all(test, feature = "skills-db"))]
mod indexed_conflict_tests {
    use super::*;
    use brassclaw_auth::{
        AuthSurface, CredentialRefreshOriginal, CredentialRefreshOutcome,
        CredentialRefreshPublication,
    };
    use brassclaw_host_api::{InvocationId, ResourceScope, TenantId, UserId};

    #[tokio::test]
    async fn native_auth_migration_rejects_same_json_with_conflicting_native_indexes() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let scope = AuthProductScope::new(
            ResourceScope {
                tenant_id: TenantId::new("indexed-custody").unwrap(),
                user_id: UserId::new("original-user").unwrap(),
                agent_id: None,
                project_id: None,
                thread_id: None,
                invocation_id: InvocationId::new(),
            },
            AuthSurface::Web,
        );
        // SQL identity/index fidelity only: no provider/account approval or
        // issuer completion is manufactured by these controlled data inputs.
        let json = serde_json::json!({"original":"unchanged JSON bytes"});
        let mut connection = rig.pool.get().await.unwrap();
        for table in [
            "brassclaw_product_auth_accounts",
            "brassclaw_product_auth_flows",
            "brassclaw_product_auth_interactions",
            "brassclaw_product_auth_requirement_selections",
        ] {
            let tx = connection.transaction().await.unwrap();
            import_record(&tx, table, &scope, "original-id", &json)
                .await
                .unwrap();
            tx.execute(&format!("UPDATE {table} SET user_id='different-indexed-user' WHERE tenant_id=$1 AND id=$2"),&[&scope.resource.tenant_id.as_str(),&"original-id"]).await.unwrap();
            assert_eq!(
                import_record(&tx, table, &scope, "original-id", &json)
                    .await
                    .unwrap_err(),
                AuthProductError::BackendConflict
            );
            let row = tx
                .query_one(
                    &format!("SELECT user_id,data FROM {table} WHERE tenant_id=$1 AND id=$2"),
                    &[&scope.resource.tenant_id.as_str(), &"original-id"],
                )
                .await
                .unwrap();
            assert_eq!(row.get::<_, String>(0), "different-indexed-user");
            assert_eq!(row.get::<_, serde_json::Value>(1), json);
            tx.rollback().await.unwrap();
        }
        let now = Utc::now();
        let account = CredentialAccount {
            id: CredentialAccountId::new(),
            scope: scope.clone(),
            provider: AuthProviderId::new("google").unwrap(),
            label: CredentialAccountLabel::new("SQL original identity").unwrap(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(SecretHandle::new("indexed-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("indexed-refresh").unwrap()),
            scopes: vec![],
            created_at: now,
            updated_at: now,
        };
        let request =
            CredentialRefreshRequest::new(scope.clone(), account.provider.clone(), account.id);
        let action = CredentialRefreshAction {
            original: CredentialRefreshOriginal::from(&request),
            expected_account: account,
            destinations: None,
            source: None,
            outcome: CredentialRefreshOutcome::Unknown(
                brassclaw_auth::AuthErrorCode::BackendUnavailable,
            ),
            publication: CredentialRefreshPublication::Pending,
            published_account: None,
            created_at: now,
            updated_at: now,
        };
        for assignment in [
            "account_id='different-indexed-account'",
            "refresh_handle=NULL",
            "blocks_refresh=FALSE",
        ] {
            let tx = connection.transaction().await.unwrap();
            import_refresh_record(&tx, scope.resource.tenant_id.as_str(), &action)
                .await
                .unwrap();
            tx.execute(&format!("UPDATE brassclaw_product_auth_refreshes SET {assignment} WHERE tenant_id=$1 AND scope_ref=$2 AND id=$3"),&[&scope.resource.tenant_id.as_str(),&action.original.scope_ref().unwrap(),&action.original.id.to_string()]).await.unwrap();
            assert_eq!(
                import_refresh_record(&tx, scope.resource.tenant_id.as_str(), &action)
                    .await
                    .unwrap_err(),
                AuthProductError::BackendConflict
            );
            let row=tx.query_one("SELECT data FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1 AND scope_ref=$2 AND id=$3",&[&scope.resource.tenant_id.as_str(),&action.original.scope_ref().unwrap(),&action.original.id.to_string()]).await.unwrap();
            assert_eq!(
                row.get::<_, serde_json::Value>(0),
                serde_json::to_value(&action).unwrap()
            );
            tx.rollback().await.unwrap();
        }
    }
}
