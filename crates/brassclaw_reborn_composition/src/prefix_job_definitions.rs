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
pub(crate) struct Definition {
    pub(crate) id: Uuid,
    pub(crate) profile: String,
    pub(crate) revision: u64,
    pub(crate) operation: String,
    pub(crate) mode: String,
    pub(crate) request: Value,
    pub(crate) program_bytes: String,
    pub(crate) input: Value,
    pub(crate) scope: TurnScope,
    pub(crate) thread_scope: ThreadScope,
    pub(crate) created_by: String,
}
pub(crate) struct Reservation<'a> {
    pub(crate) id: Uuid,
    pub(crate) profile: &'a str,
    pub(crate) revision: u64,
    pub(crate) operation: &'a str,
    pub(crate) mode: &'a str,
    pub(crate) request: &'a Value,
    pub(crate) program: &'a InspectedRetainedProgram,
    pub(crate) input: &'a Value,
    pub(crate) scope: &'a TurnScope,
    pub(crate) thread_scope: &'a ThreadScope,
    pub(crate) created_by: &'a str,
}
pub(crate) struct Header {
    pub(crate) id: Uuid,
    pub(crate) profile: String,
    pub(crate) revision: u64,
    pub(crate) operation: String,
    pub(crate) mode: String,
    pub(crate) scope: TurnScope,
    pub(crate) created_by: String,
    pub(crate) created_at: String,
    pub(crate) request_sha256: String,
    pub(crate) program_sha256: String,
    pub(crate) input_sha256: String,
}
pub(crate) struct Store {
    pool: Arc<PgPool>,
}
const COLUMNS: &str = "job_id,profile_name,profile_revision,operation,mode,request_bytes,request_checksum,program_bytes,program_checksum,input_bytes,input_checksum,scope,thread_scope,created_by";
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
    let input_bytes: String = row.get(9);
    let input = parse(
        &input_bytes,
        &row.get::<_, String>(10),
        brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
    )?;
    let scope = serde_json::from_value(row.get::<_, Value>(11)).map_err(|_| Error::Invalid)?;
    let thread_scope =
        serde_json::from_value(row.get::<_, Value>(12)).map_err(|_| Error::Invalid)?;
    validate_scope(&scope, &thread_scope, &row.get::<_, String>(13))?;
    Ok(Definition {
        id: row.get(0),
        profile: row.get(1),
        revision: u64::try_from(row.get::<_, i64>(2)).map_err(|_| Error::Invalid)?,
        operation: row.get(3),
        mode: row.get(4),
        request,
        program_bytes,
        input,
        scope,
        thread_scope,
        created_by: row.get(13),
    })
}
impl Store {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
    /// Small read projection; status polling never reloads compiler/source blobs.
    pub(crate) async fn header(&self, id: Uuid) -> Result<Option<Header>, Error> {
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let Some(row) = client.query_opt("SELECT job_id,profile_name,profile_revision,operation,mode,scope,thread_scope,created_by,created_at,request_checksum,program_checksum,input_checksum FROM brassclaw_prefix_job_definitions WHERE job_id=$1", &[&id]).await.map_err(|_| Error::Unavailable)? else { return Ok(None); };
        let scope = serde_json::from_value(row.get::<_, Value>(5)).map_err(|_| Error::Invalid)?;
        let thread = serde_json::from_value(row.get::<_, Value>(6)).map_err(|_| Error::Invalid)?;
        let actor: String = row.get(7);
        validate_scope(&scope, &thread, &actor)?;
        Ok(Some(Header {
            id: row.get(0),
            profile: row.get(1),
            revision: u64::try_from(row.get::<_, i64>(2)).map_err(|_| Error::Invalid)?,
            operation: row.get(3),
            mode: row.get(4),
            scope,
            created_by: actor,
            created_at: row.get::<_, chrono::DateTime<chrono::Utc>>(8).to_rfc3339(),
            request_sha256: row.get(9),
            program_sha256: row.get(10),
            input_sha256: row.get(11),
        }))
    }
    pub(crate) async fn get(&self, id: Uuid) -> Result<Option<Definition>, Error> {
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
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
        if r.id.is_nil()
            || r.created_by.is_empty()
            || r.created_by.len() > 512
            || !matches!(
                r.operation,
                "generate" | "redesign" | "model_design" | "create" | "measure"
            )
            || !matches!(r.mode, "source_only" | "target")
        {
            return Err(Error::Invalid);
        }
        validate_scope(r.scope, r.thread_scope, r.created_by)?;
        let revision = i64::try_from(r.revision)
            .ok()
            .filter(|value| *value > 0)
            .ok_or(Error::Invalid)?;
        let request_bytes = serde_json::to_string(r.request).map_err(|_| Error::Invalid)?;
        let request_checksum = sha(&request_bytes);
        let mut client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let tx = client.transaction().await.map_err(|_| Error::Unavailable)?;
        if let Some(row) = tx
            .query_opt(
                &format!("SELECT {COLUMNS} FROM brassclaw_prefix_job_definitions WHERE job_id=$1"),
                &[&r.id],
            )
            .await
            .map_err(|_| Error::Unavailable)?
        {
            let existing = decode(row)?;
            if existing.request != *r.request
                || existing.profile != r.profile
                || existing.revision != r.revision
                || existing.operation != r.operation
                || existing.mode != r.mode
                || existing.created_by != r.created_by
                || existing.scope != *r.scope
                || existing.thread_scope != *r.thread_scope
            {
                return Err(Error::Conflict);
            }
            tx.commit().await.map_err(|_| Error::Unavailable)?;
            return Ok(existing);
        }
        let current = tx
            .query_opt(
                "SELECT revision FROM reborn_prefix_profiles WHERE name=$1 FOR SHARE",
                &[&r.profile],
            )
            .await
            .map_err(|_| Error::Unavailable)?
            .ok_or(Error::Invalid)?;
        if current.get::<_, i64>(0) != revision {
            return Err(Error::Conflict);
        }
        // Inspecting/binding is mechanical; it cannot turn a draft into an
        // approved pipeline. Actual admission revalidates trusted provenance.
        r.program
            .program()
            .inputs()
            .bind_task_inputs_with_limits(
                r.input,
                brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
            )
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
        parse(
            &input_bytes,
            &sha(&input_bytes),
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )?;
        let scope = serde_json::to_value(r.scope).map_err(|_| Error::Invalid)?;
        let thread_scope = serde_json::to_value(r.thread_scope).map_err(|_| Error::Invalid)?;
        tx.execute("INSERT INTO brassclaw_prefix_job_definitions(job_id,profile_name,profile_revision,operation,mode,request_bytes,request_checksum,program_bytes,program_checksum,input_bytes,input_checksum,scope,thread_scope,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14) ON CONFLICT(job_id) DO NOTHING",&[&r.id,&r.profile,&revision,&r.operation,&r.mode,&request_bytes,&request_checksum,&program_bytes,&sha(&program_bytes),&input_bytes,&sha(&input_bytes),&scope,&thread_scope,&r.created_by]).await.map_err(|_| Error::Unavailable)?;
        let row = tx
            .query_one(
                &format!("SELECT {COLUMNS} FROM brassclaw_prefix_job_definitions WHERE job_id=$1"),
                &[&r.id],
            )
            .await
            .map_err(|_| Error::Unavailable)?;
        let stored = decode(row)?;
        if stored.request != *r.request
            || stored.profile != r.profile
            || stored.revision != r.revision
            || stored.operation != r.operation
            || stored.mode != r.mode
            || stored.created_by != r.created_by
            || stored.scope != *r.scope
            || stored.thread_scope != *r.thread_scope
        {
            return Err(Error::Conflict);
        }
        tx.commit().await.map_err(|_| Error::Unavailable)?;
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
    macro_rules! reserve {
        ($id:expr,$request:expr,$input:expr) => {
            Reservation {
                id: $id,
                profile: "base-prompt",
                revision: revision as u64,
                operation: "generate",
                mode: "source_only",
                request: $request,
                program,
                input: $input,
                scope: &scope,
                thread_scope: &thread_scope,
                created_by: "operator",
            }
        };
    }
    assert!(store.get(id).await.unwrap().is_none());
    let first = store.reserve(reserve!(id, &request, &input)).await.unwrap();
    let stored = store.get(id).await.unwrap().unwrap();
    assert_eq!(stored.id, id);
    assert_eq!(stored.profile, "base-prompt");
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
        serde_json::to_value(&stored.scope).unwrap(),
        serde_json::to_value(&scope).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&stored.thread_scope).unwrap(),
        serde_json::to_value(&thread_scope).unwrap()
    );
    // Retry captures retain the first input, even if a caller now offers
    // different source data under the same unchanged transport request.
    let mut offered = input.clone();
    offered["snapshot_bytes"] = json!("a newer capture must not replace the first");
    let duplicate = store
        .reserve(reserve!(id, &request, &offered))
        .await
        .unwrap();
    assert_eq!(duplicate.input, first.input);
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
