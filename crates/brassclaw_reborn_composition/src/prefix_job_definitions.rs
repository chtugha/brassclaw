//! Prefix metadata linked to the ordinary turn run, never an execution queue.
//! Definitions pin data/artifacts, not authority. The admission owner must check
//! the selected program's trusted approval, current kernel policy and real claim.
use brassclaw_engine::executor::retained_source::InspectedRetainedProgram;
use brassclaw_pg::PgPool;
use brassclaw_threads::ThreadScope;
use brassclaw_turns::TurnScope;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("invalid prefix job definition")]
    Invalid,
    #[error("prefix job request conflicts with its retained definition")]
    Conflict,
    #[error("prefix job storage unavailable")]
    Unavailable,
}
#[path = "prefix_job_target.rs"]
mod target;
pub(crate) use target::Target;
#[path = "prefix_job_origin.rs"]
mod origin;
pub(crate) use origin::Origin;

pub(crate) struct Definition {
    pub(crate) id: Uuid,
    pub(crate) target: Target,
    pub(crate) operation: String,
    pub(crate) mode: String,
    pub(crate) request: Value,
    pub(crate) request_bytes: String,
    pub(crate) program_bytes: String,
    pub(crate) input: Value,
    pub(crate) input_bytes: String,
    pub(crate) original_input_limits: brassclaw_skills::value_contract::ContractLimits,
    pub(crate) origin: Origin,
    pub(crate) created_by: String,
}
impl Definition {
    pub(crate) fn original_checksum(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(Sha256::digest(self.request_bytes.as_bytes()));
        hash.update(Sha256::digest(self.program_bytes.as_bytes()));
        hash.update(Sha256::digest(self.input_bytes.as_bytes()));
        hash.finalize().into()
    }
}
pub(crate) struct Reservation<'a> {
    pub(crate) id: Uuid,
    pub(crate) target: &'a Target,
    pub(crate) operation: &'a str,
    pub(crate) mode: &'a str,
    pub(crate) request: &'a Value,
    pub(crate) program: &'a InspectedRetainedProgram,
    pub(crate) input: &'a Value,
    pub(crate) original_input_limits: brassclaw_skills::value_contract::ContractLimits,
    pub(crate) origin: &'a Origin,
    pub(crate) created_by: &'a str,
}
pub(crate) struct Header {
    pub(crate) id: Uuid,
    pub(crate) target: Target,
    pub(crate) operation: String,
    pub(crate) mode: String,
    pub(crate) origin: Origin,
    pub(crate) created_by: String,
    pub(crate) created_at: String,
    pub(crate) request_sha256: String,
    pub(crate) program_sha256: String,
    pub(crate) input_sha256: String,
}
pub(crate) struct Store {
    pool: Arc<PgPool>,
}
const COLUMNS: &str = "job_id,profile_name,profile_revision,operation,mode,request_bytes,request_checksum,program_bytes,program_checksum,input_bytes,input_checksum,scope,thread_scope,created_by,original_input_limits,definition_format,private_context,original_preparation_bytes,original_preparation_checksum";

// Frozen V167 input-codec domain. SQL NULL denotes only that historical format,
// never today's settings or a future value of EXECUTION_DATA_LIMITS. Request
// metadata (65536 bytes) and selected-program metadata (REVISION_LIMITS) remain
// separately versioned format constraints, not executing task-data limits.
pub(crate) const LEGACY_INPUT_LIMITS: brassclaw_skills::value_contract::ContractLimits =
    brassclaw_skills::value_contract::ContractLimits {
        max_depth: 64,
        max_nodes: 262_144,
        max_bytes: 67_108_864,
    };
fn decode_input_limits(
    value: Option<Value>,
) -> Result<brassclaw_skills::value_contract::ContractLimits, Error> {
    let Some(value) = value else {
        return Ok(LEGACY_INPUT_LIMITS);
    };
    let object = value
        .as_object()
        .filter(|v| v.len() == 3)
        .ok_or(Error::Invalid)?;
    let count = |name| {
        object
            .get(name)
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .filter(|v| *v > 0)
            .ok_or(Error::Invalid)
    };
    let limits = brassclaw_skills::value_contract::ContractLimits {
        max_depth: count("max_depth")?,
        max_nodes: count("max_nodes")?,
        max_bytes: count("max_bytes")?,
    };
    u32::try_from(limits.max_depth).map_err(|_| Error::Invalid)?;
    Ok(limits)
}
fn encode_input_limits(
    limits: brassclaw_skills::value_contract::ContractLimits,
) -> Result<Value, Error> {
    let value = serde_json::json!({"max_depth":limits.max_depth,"max_nodes":limits.max_nodes,"max_bytes":limits.max_bytes});
    decode_input_limits(Some(value.clone()))?;
    Ok(value)
}
/// Convert only the three effective, already validated existing settings fields.
/// This is data capture, not another settings authority or task/resource grant.
pub(crate) fn captured_input_limits(
    policy: brassclaw_host_api::MontyExecutionLimits,
) -> Result<brassclaw_skills::value_contract::ContractLimits, Error> {
    policy.validate().map_err(|_| Error::Invalid)?;
    decode_input_limits(Some(
        serde_json::json!({"max_depth":policy.max_value_depth,"max_nodes":policy.max_value_nodes,"max_bytes":policy.max_value_bytes}),
    ))
}

/// Read the current effective execution limits from the default settings row
/// within the caller's transaction. Returns the `LEGACY_INPUT_LIMITS` default
/// when no row exists, matching the behaviour of `preparation_policy()`.
/// Must only be called inside the transaction that owns the coherent snapshot.
pub(crate) async fn read_effective_limits_in_transaction(
    client: &(impl tokio_postgres::GenericClient + Sync),
) -> Result<brassclaw_skills::value_contract::ContractLimits, Error> {
    let row = client
        .query_opt(
            "SELECT execution_limits FROM reborn_monty_vm_settings \
             WHERE tenant_id='default' AND user_id='default' \
               AND agent_id='default' AND project_id='default'",
            &[],
        )
        .await
        .map_err(|_| Error::Unavailable)?;
    let policy: brassclaw_host_api::MontyExecutionLimits = match row {
        Some(r) => {
            serde_json::from_value(r.get::<_, serde_json::Value>(0)).map_err(|_| Error::Invalid)?
        }
        None => brassclaw_host_api::MontyExecutionLimits::default(),
    };
    captured_input_limits(policy)
}

#[cfg(test)]
#[path = "prefix_job_input_identity_tests.rs"]
mod input_identity_tests;
#[cfg(test)]
#[path = "prefix_pair_job_target_tests.rs"]
mod pair_target_tests;
fn sha(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
fn parse(
    bytes: &str,
    checksum: &str,
    limits: brassclaw_skills::value_contract::ContractLimits,
) -> Result<Value, Error> {
    if sha(bytes) != checksum {
        return Err(Error::Invalid);
    }
    brassclaw_skills::value_contract::strict_json(bytes, limits).map_err(|_| Error::Invalid)
}
fn validate_scope(scope: &TurnScope, thread: &ThreadScope, actor: &str) -> Result<(), Error> {
    if scope.tenant_id != thread.tenant_id
        || scope.agent_id.as_ref() != Some(&thread.agent_id)
        || scope.project_id != thread.project_id
        || scope.explicit_owner_user_id() != thread.owner_user_id.as_ref()
        || scope.explicit_owner_user_id().map(|owner| owner.as_str()) != Some(actor)
    {
        return Err(Error::Invalid);
    }
    Ok(())
}
fn decode(row: tokio_postgres::Row) -> Result<Definition, Error> {
    let request_bytes: String = row.get(5);
    let request = parse(
        &request_bytes,
        &row.get::<_, String>(6),
        brassclaw_skills::value_contract::ContractLimits {
            max_bytes: 65536,
            ..brassclaw_skills::component_revision::REVISION_LIMITS
        },
    )?;
    let program_bytes: String = row.get(7);
    parse(
        &program_bytes,
        &row.get::<_, String>(8),
        brassclaw_skills::component_revision::REVISION_LIMITS,
    )?;
    let original_input_limits = decode_input_limits(row.try_get(14).map_err(|_| Error::Invalid)?)?;
    let input_bytes: String = row.get(9);
    let input = parse(
        &input_bytes,
        &row.get::<_, String>(10),
        original_input_limits,
    )?;
    let origin = origin::decode(
        &row.get::<_, String>(15),
        row.get(11),
        row.get(12),
        row.get(16),
        row.get(17),
        row.get(18),
        &row.get::<_, String>(13),
    )?;
    if let Origin::Private { preparation, .. } = &origin {
        let limits = captured_input_limits(preparation.policy())?;
        if limits.max_depth != original_input_limits.max_depth
            || limits.max_nodes != original_input_limits.max_nodes
            || limits.max_bytes != original_input_limits.max_bytes
        {
            return Err(Error::Invalid);
        }
    }
    let operation: String = row.get(3);
    let mode: String = row.get(4);
    let target = Target::from_columns(row.get(1), row.get(2), &operation, &mode, &request)?;
    Ok(Definition {
        id: row.get(0),
        target,
        operation,
        mode,
        request,
        request_bytes,
        program_bytes,
        input,
        input_bytes,
        original_input_limits,
        origin,
        created_by: row.get(13),
    })
}
fn same_request(existing: &Definition, r: &Reservation<'_>) -> bool {
    existing.request == *r.request
        && existing.input == *r.input
        && existing.target == *r.target
        && existing.operation == r.operation
        && existing.mode == r.mode
        && existing.created_by == r.created_by
        && existing.origin == *r.origin
    // Original codec meaning remains fixed when a retry arrives after a
    // live settings edit. Offered new limits cannot reinterpret old bytes.
}

impl Store {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
    /// Small read projection; status polling never reloads compiler/source blobs.
    pub(crate) async fn header(&self, id: Uuid) -> Result<Option<Header>, Error> {
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let Some(row) = client.query_opt("SELECT job_id,profile_name,profile_revision,operation,mode,scope,thread_scope,created_by,created_at,request_checksum,program_checksum,input_checksum,request_bytes,original_input_limits,definition_format,private_context,original_preparation_bytes,original_preparation_checksum FROM brassclaw_prefix_job_definitions WHERE job_id=$1", &[&id]).await.map_err(|_| Error::Unavailable)? else { return Ok(None); };
        decode_input_limits(row.try_get(13).map_err(|_| Error::Invalid)?)?;
        let actor: String = row.get(7);
        let origin = origin::decode(
            &row.get::<_, String>(14),
            row.get(5),
            row.get(6),
            row.get(15),
            row.get(16),
            row.get(17),
            &actor,
        )?;
        let operation: String = row.get(3);
        let mode: String = row.get(4);
        let request = parse(
            &row.get::<_, String>(12),
            &row.get::<_, String>(9),
            brassclaw_skills::value_contract::ContractLimits {
                max_bytes: 65536,
                ..brassclaw_skills::component_revision::REVISION_LIMITS
            },
        )?;
        let target = Target::from_columns(row.get(1), row.get(2), &operation, &mode, &request)?;
        Ok(Some(Header {
            id: row.get(0),
            target,
            operation,
            mode,
            origin,
            created_by: actor,
            created_at: row.get::<_, chrono::DateTime<chrono::Utc>>(8).to_rfc3339(),
            request_sha256: row.get(9),
            program_sha256: row.get(10),
            input_sha256: row.get(11),
        }))
    }
    pub(crate) async fn get(&self, id: Uuid) -> Result<Option<Definition>, Error> {
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        Self::get_on(&**client, id).await
    }

    /// Read the original immutable job with the caller's transaction. Mutation
    /// custody never acquires a second pooled connection while holding locks.
    pub(crate) async fn get_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        id: Uuid,
    ) -> Result<Option<Definition>, Error> {
        Self::get_on(tx, id).await
    }

    pub(crate) async fn get_on<C: tokio_postgres::GenericClient + Sync>(
        client: &C,
        id: Uuid,
    ) -> Result<Option<Definition>, Error> {
        client
            .query_opt(
                &format!("SELECT {COLUMNS} FROM brassclaw_prefix_job_definitions WHERE job_id=$1"),
                &[&id],
            )
            .await
            .map_err(|_| Error::Unavailable)?
            .map(decode)
            .transpose()
    }
    /// Same request UUID/content returns the first immutable capture, including
    /// after a concurrent catalogue/profile change. It never starts or retries a
    /// run. The caller submits that retained definition through normal ingress.
    pub(crate) async fn reserve(&self, r: Reservation<'_>) -> Result<Definition, Error> {
        if r.id.is_nil() || r.created_by.is_empty() || r.created_by.len() > 512 {
            return Err(Error::Invalid);
        }
        r.origin.validate(r.created_by)?;
        r.target.columns(r.operation, r.mode, r.request)?;
        if let Target::Pair {
            specification,
            proposal,
        } = r.target
        {
            // A reconnect needs only its original immutable definition. Never
            // recollect sources or replace a previously selected program/input.
            if let Some(existing) = self.get(r.id).await? {
                if !same_request(&existing, &r) {
                    return Err(Error::Conflict);
                }
                return Ok(existing);
            }
            let packet = crate::prefix_model_designs::PgPrefixModelDesigns::new(
                self.pool.clone(),
                r.origin.tenant_id().as_str().into(),
            )
            .case_packet(specification, proposal)
            .await
            .map_err(|error| match error {
                brassclaw_product_workflow::InterceptorConfigServiceError::Unavailable => {
                    Error::Unavailable
                }
                _ => Error::Invalid,
            })?;
            if *r.input != serde_json::json!({"case_packet_bytes":packet.case_packet_bytes}) {
                return Err(Error::Invalid);
            }
        }
        let mut client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let tx = client.transaction().await.map_err(|_| Error::Unavailable)?;
        let stored = Self::reserve_prepared_in_transaction(&tx, r).await?;
        tx.commit().await.map_err(|_| Error::Unavailable)?;
        Ok(stored)
    }

    /// Caller owns the only commit. Pair's existing pool-based original packet
    /// validation stays outside its transaction in reserve; it cannot be bypassed
    /// by this ModelSetup/Profile entry point.
    pub(crate) async fn reserve_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        r: Reservation<'_>,
    ) -> Result<Definition, Error> {
        if matches!(r.target, Target::Pair { .. }) {
            return Err(Error::Invalid);
        }
        Self::reserve_prepared_in_transaction(tx, r).await
    }

    async fn reserve_prepared_in_transaction(
        tx: &tokio_postgres::Transaction<'_>,
        r: Reservation<'_>,
    ) -> Result<Definition, Error> {
        if r.id.is_nil() || r.created_by.is_empty() || r.created_by.len() > 512 {
            return Err(Error::Invalid);
        }
        r.origin.validate(r.created_by)?;
        let (profile, revision) = r.target.columns(r.operation, r.mode, r.request)?;
        let request_bytes = serde_json::to_string(r.request).map_err(|_| Error::Invalid)?;
        let request_checksum = sha(&request_bytes);
        if let Some(existing) = Self::get_in_transaction(tx, r.id).await? {
            if !same_request(&existing, &r) {
                return Err(Error::Conflict);
            }
            return Ok(existing);
        }
        // Verify the offered input limits match the effective preparation policy
        // at commit time. A concurrent live settings edit between qualification
        // capture and this reserve must be detected and rejected rather than
        // silently storing a stale codec boundary.
        let effective_limits = match r.origin {
            Origin::Chat { .. } => read_effective_limits_in_transaction(tx).await?,
            Origin::Private { preparation, .. } => {
                if r.operation != "generate"
                    || r.mode != "source_only"
                    || !matches!(r.target, Target::Profile { .. })
                {
                    return Err(Error::Invalid);
                }
                captured_input_limits(preparation.policy())?
            }
        };
        if r.original_input_limits.max_depth != effective_limits.max_depth
            || r.original_input_limits.max_nodes != effective_limits.max_nodes
            || r.original_input_limits.max_bytes != effective_limits.max_bytes
        {
            return Err(Error::Conflict);
        }
        if profile.is_some() {
            let current = tx
                .query_opt(
                    "SELECT revision FROM reborn_prefix_profiles WHERE name=$1 FOR SHARE",
                    &[&profile],
                )
                .await
                .map_err(|_| Error::Unavailable)?
                .ok_or(Error::Invalid)?;
            if Some(current.get::<_, i64>(0)) != revision {
                return Err(Error::Conflict);
            }
        }
        // Inspecting/binding is mechanical; it cannot turn a draft into an
        // approved pipeline. Actual admission revalidates trusted provenance.
        r.program
            .program()
            .inputs()
            .bind_task_inputs_with_limits(r.input, r.original_input_limits)
            .map_err(|_| Error::Invalid)?;
        let program_bytes = r
            .program
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(|_| Error::Invalid)?
            .exact_bytes()
            .to_owned();
        let input_bytes = serde_json::to_string(r.input).map_err(|_| Error::Invalid)?;
        parse(
            &request_bytes,
            &request_checksum,
            brassclaw_skills::value_contract::ContractLimits {
                max_bytes: 65536,
                ..brassclaw_skills::component_revision::REVISION_LIMITS
            },
        )?;
        parse(
            &program_bytes,
            &sha(&program_bytes),
            brassclaw_skills::component_revision::REVISION_LIMITS,
        )?;
        parse(&input_bytes, &sha(&input_bytes), r.original_input_limits)?;
        // Every newly created reservation writes a concrete validated record;
        // NULL is read-only legacy compatibility, never a new default.
        let original_input_limits = encode_input_limits(r.original_input_limits)?;
        let (scope, thread_scope, private_context, preparation_bytes, preparation_checksum) =
            match r.origin {
                Origin::Chat {
                    scope,
                    thread_scope,
                } => (
                    Some(serde_json::to_value(scope).map_err(|_| Error::Invalid)?),
                    Some(serde_json::to_value(thread_scope).map_err(|_| Error::Invalid)?),
                    None,
                    None,
                    None,
                ),
                Origin::Private {
                    context,
                    preparation,
                } => (
                    None,
                    None,
                    Some(serde_json::to_value(context).map_err(|_| Error::Invalid)?),
                    Some(preparation.bytes()),
                    Some(preparation.checksum()),
                ),
            };
        tx.execute("INSERT INTO brassclaw_prefix_job_definitions(job_id,profile_name,profile_revision,operation,mode,request_bytes,request_checksum,program_bytes,program_checksum,input_bytes,input_checksum,scope,thread_scope,created_by,original_input_limits,definition_format,private_context,original_preparation_bytes,original_preparation_checksum) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19) ON CONFLICT(job_id) DO NOTHING",&[&r.id,&profile,&revision,&r.operation,&r.mode,&request_bytes,&request_checksum,&program_bytes,&sha(&program_bytes),&input_bytes,&sha(&input_bytes),&scope,&thread_scope,&r.created_by,&original_input_limits,&r.origin.format(),&private_context,&preparation_bytes,&preparation_checksum]).await.map_err(|_| Error::Unavailable)?;
        let stored = Self::get_in_transaction(tx, r.id)
            .await?
            .ok_or(Error::Unavailable)?;
        if !same_request(&stored, &r) {
            return Err(Error::Conflict);
        }
        Ok(stored)
    }
}

#[cfg(test)]
pub(super) async fn assert_metadata_only(
    catalogue: &Arc<crate::installed_monty_catalogue::InstalledMontyCatalogue>,
) {
    use brassclaw_host_api::{AgentId, ProjectId, TenantId, ThreadId, UserId};
    use serde_json::json;
    let store = Store::new(catalogue.pool.clone());
    let program = catalogue._compiler_program.as_ref().unwrap();
    let id = Uuid::new_v4();
    let tenant = TenantId::new("prefix-definition-test").unwrap();
    let agent = AgentId::new("metadata").unwrap();
    let project = ProjectId::new("system").unwrap();
    let actor = UserId::new("operator").unwrap();
    let scope = TurnScope::new_with_owner(
        tenant.clone(),
        Some(agent.clone()),
        Some(project.clone()),
        ThreadId::new(format!("prefix-job-{id}")).unwrap(),
        Some(actor.clone()),
    );
    let thread_scope = ThreadScope {
        tenant_id: tenant,
        agent_id: agent,
        project_id: Some(project),
        owner_user_id: Some(actor),
    };
    let client = catalogue.pool.get().await.unwrap();
    let revision: i64 = client
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let calls: i64 = client
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    let admissions: i64 = client
        .query_one("SELECT count(*) FROM brassclaw_monty_task_admissions", &[])
        .await
        .unwrap()
        .get(0);
    drop(client);
    let fixture = crate::bootstrap_compiler_fixture::CompilerFixture::scoped(
        "base-prompt",
        "literal-reference/v1",
        "metadata-definition/1",
    )
    .unwrap();
    let input = json!({"profile":"base-prompt","revision":revision.to_string(),"snapshot_bytes":crate::prefix_generations::canonical(fixture.snapshot()).unwrap()});
    let request =
        json!({"profile":"base-prompt","expected_revision":revision,"mode":"source_only"});
    let target = Target::Profile {
        name: "base-prompt".into(),
        revision: revision as u64,
    };
    macro_rules! reserve {
        ($id:expr,$request:expr,$input:expr) => {
            Reservation {
                original_input_limits: brassclaw_skills::value_contract::ContractLimits {
                    max_depth: program.preparation_limits().bounds.max_value_depth,
                    max_nodes: program.preparation_limits().bounds.max_value_nodes,
                    max_bytes: program.preparation_limits().bounds.max_value_bytes,
                },
                id: $id,
                target: &target,
                operation: "generate",
                mode: "source_only",
                request: $request,
                program,
                input: $input,
                origin: &crate::prefix_job_definitions::Origin::Chat {
                    scope: scope.clone(),
                    thread_scope: thread_scope.clone(),
                },
                created_by: "operator",
            }
        };
    }
    assert!(store.get(id).await.unwrap().is_none());
    let first = store.reserve(reserve!(id, &request, &input)).await.unwrap();
    let stored = store.get(id).await.unwrap().unwrap();
    assert_eq!(stored.id, id);
    assert_eq!(
        stored.target.profile(),
        Some(("base-prompt", revision as u64))
    );
    assert_eq!(stored.input, input);
    assert_eq!(
        stored.program_bytes,
        program
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .unwrap()
            .exact_bytes()
    );
    assert_eq!(
        serde_json::to_value(stored.origin.chat_scope().expect("legacy chat original")).unwrap(),
        serde_json::to_value(&scope).unwrap()
    );
    assert_eq!(
        serde_json::to_value(stored.origin.thread_scope().expect("legacy chat original")).unwrap(),
        serde_json::to_value(&thread_scope).unwrap()
    );
    // Explicit offered input belongs to request identity. A reconnect that
    // supplies no new data uses get(); it does not submit a changed reservation.
    let mut offered = input.clone();
    offered["snapshot_bytes"] = json!("a newer capture must not replace the first");
    assert!(matches!(
        store.reserve(reserve!(id, &request, &offered)).await,
        Err(Error::Conflict)
    ));
    let duplicate = store.reserve(reserve!(id, &request, &input)).await.unwrap();
    assert_eq!(duplicate.input, first.input);
    assert_eq!(duplicate.program_bytes, first.program_bytes);
    input_identity_tests::assert_input_identity(
        catalogue.pool.clone(),
        reserve!(id, &request, &input),
        catalogue._generation_candidates[0].as_ref(),
    )
    .await;
    let changed = json!({"profile":"base-prompt","expected_revision":revision,"mode":"target"});
    assert!(matches!(
        store.reserve(reserve!(id, &changed, &input)).await,
        Err(Error::Conflict)
    ));
    let client = catalogue.pool.get().await.unwrap();
    assert!(
        client
            .execute(
                "UPDATE brassclaw_prefix_job_definitions SET input_bytes='{}' WHERE job_id=$1",
                &[&id]
            )
            .await
            .is_err()
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM brassclaw_prefix_job_definitions WHERE job_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        calls
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_task_admissions", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        admissions
    );
}
