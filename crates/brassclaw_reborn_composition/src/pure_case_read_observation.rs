//! Test-only passive trace of actual failed and settled historical reads. It neither
//! repeats a read nor changes routing, verification, deadlines or settlement.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use uuid::Uuid;

pub(crate) struct Trace {
    id: Uuid,
    revision: u64,
    timeout: Duration,
    started: Instant,
    phases: Mutex<Vec<(&'static str, u128)>>,
    relation: Mutex<Option<Value>>,
}
tokio::task_local! { static ACTIVE: Arc<Trace>; }
impl Trace {
    pub(crate) fn new(id: Uuid, revision: u64, timeout: Duration) -> Arc<Self> {
        Arc::new(Self {
            id,
            revision,
            timeout,
            started: Instant::now(),
            phases: Mutex::new(Vec::new()),
            relation: Mutex::new(None),
        })
    }
    pub(crate) fn report(&self, outcome: &'static str, pool: &brassclaw_pg::PgPool) {
        let pool = pool.status();
        let phases = self
            .phases
            .lock()
            .map(|phases| json!(*phases))
            .unwrap_or_else(|_| json!("unavailable"));
        let relation = self
            .relation
            .lock()
            .map(|relation| json!(*relation))
            .unwrap_or_else(|_| json!("unavailable"));
        eprintln!(
            "Original Pure case historical read {}",
            json!({
                "format":"original-pure-case-read-diagnostic/1","case_id":self.id,
                "settings_revision":self.revision,"timeout_millis":self.timeout.as_millis(),
                "elapsed_millis":self.started.elapsed().as_millis(),"outcome":outcome,
                "phases":phases,"native_relation":relation,
                "pool":{"max_size":pool.max_size,"size":pool.size,"available":pool.available,"waiting":pool.waiting},
            })
        );
    }
}
pub(crate) async fn scope<T>(trace: Arc<Trace>, future: impl std::future::Future<Output = T>) -> T {
    ACTIVE.scope(trace, future).await
}
pub(crate) fn note(phase: &'static str) {
    let _ = ACTIVE.try_with(|trace| {
        if let Ok(mut phases) = trace.phases.lock() {
            phases.push((phase, trace.started.elapsed().as_millis()));
        }
    });
}
pub(crate) fn relation(flags: Value) {
    let _ = ACTIVE.try_with(|trace| {
        if let Ok(mut relation) = trace.relation.lock() {
            *relation = Some(flags);
        }
    });
}

/// Only boolean relationships for this already authenticated original, under
/// its original RR snapshot. Never source, input, output, exception, key or SQL
/// diagnostic text. This runs only after the actual guarded join returns none.
pub(crate) async fn missing_relation(
    tx: &tokio_postgres::Transaction<'_>,
    id: Uuid,
    task: Value,
    source: &str,
    inputs: &Value,
) {
    if ACTIVE.try_with(|_| ()).is_err() {
        return;
    }
    let result = tx.query_one("SELECT e.case_id IS NOT NULL,o.case_id IS NOT NULL,s.case_id IS NOT NULL,i.invocation_id IS NOT NULL,a.run_id IS NOT NULL,
        COALESCE(i.phase='answered' AND i.attempt_count=1,false),
        COALESCE(i.selection_bytes::jsonb->>'capability_id'='builtin.pure_review.invoke_original',false),
        COALESCE(i.arguments_bytes::jsonb='{}'::jsonb,false),
        COALESCE(i.answer_bytes::jsonb=jsonb_build_object('kind','return','value',o.observation_bytes),false),
        COALESCE((a.outcome->>'internal_receipt')::jsonb=jsonb_build_object('format','original-pure-case-receipt/1','observation',o.observation_bytes::jsonb),false),
        COALESCE(e.execution_bytes::jsonb->'task'=$2::jsonb,false),
        COALESCE(s.receipt_bytes::jsonb->'command'->'command'->'selected'->>'source'=$3,false),
        COALESCE(s.receipt_bytes::jsonb->'command'->'command'->'selected'->'aliases'='[]'::jsonb,false),
        COALESCE(s.receipt_bytes::jsonb->'command'->'command'->'inputs'=$4::jsonb,false),
        COALESCE(o.observation_bytes::jsonb->'invocation_checksum'=(SELECT jsonb_agg(get_byte(sha256(convert_to($3,'UTF8')),k) ORDER BY k) FROM generate_series(0,31)k),false),
        COALESCE(original_pure_case_observation_valid(e.case_id,e.checksum,o.observation_bytes,o.native_receipts_bytes),false)
        FROM (SELECT $1::uuid AS case_id) original
        LEFT JOIN reborn_original_pure_case_executions e USING(case_id)
        LEFT JOIN reborn_original_pure_case_observations o USING(case_id)
        LEFT JOIN reborn_original_pure_case_native_receipts s ON s.case_id=original.case_id AND s.stage='start'
        LEFT JOIN brassclaw_monty_tool_invocations i ON i.invocation_id=e.invocation_id AND i.run_id=e.run_id
        LEFT JOIN brassclaw_monty_task_admissions a ON a.run_id=e.run_id", &[&id,&task,&source,inputs]).await;
    match result {
        Ok(row) => {
            let names = [
                "execution_present",
                "observation_present",
                "start_present",
                "invocation_present",
                "admission_present",
                "answered_once",
                "capability_matches",
                "arguments_match",
                "answer_matches",
                "receipt_matches",
                "task_matches",
                "source_matches",
                "aliases_match",
                "inputs_match",
                "invocation_checksum_matches",
                "native_observation_valid",
            ];
            let flags: serde_json::Map<String, Value> = names
                .into_iter()
                .enumerate()
                .map(|(index, name)| (name.into(), json!(row.get::<_, bool>(index))))
                .collect();
            relation(Value::Object(flags));
        }
        Err(error) => relation(
            json!({"observation":"query_failed","sqlstate":error.code().map(|code|code.code())}),
        ),
    }
}
