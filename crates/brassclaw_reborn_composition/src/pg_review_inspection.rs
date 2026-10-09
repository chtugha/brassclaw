//! Operator evidence inspection. An observation is not effect reconciliation.
use std::sync::Arc;

use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    PostTurnReviewInspection, PostTurnReviewList, PostTurnReviewSummary, RecipeStoreError,
    ReviewDispositionReceipt, ReviewDispositionRequest, ReviewDispositionView,
};
use tokio_postgres::GenericClient;
use uuid::Uuid;

fn id(value: &str) -> Result<Uuid, RecipeStoreError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| RecipeStoreError::Invalid("canonical non-nil UUID required".into()))
}
fn unavailable(_: impl std::fmt::Display) -> RecipeStoreError {
    // Database errors may contain retained user/model content.
    RecipeStoreError::Unavailable("review inspection storage unavailable".into())
}

pub(crate) async fn list(
    pool: &Arc<PgPool>,
    after: Option<&str>,
) -> Result<PostTurnReviewList, RecipeStoreError> {
    let after = after.map(id).transpose()?.unwrap_or(Uuid::nil());
    let client = pool.get().await.map_err(unavailable)?;
    let rows = client
        .query(
            "SELECT source_run_id,attempt_id,phase,model_dispatch_count,
        EXISTS(SELECT 1 FROM brassclaw_monty_review_settlements s WHERE s.attempt_id=w.attempt_id),
        (SELECT count(*) FROM brassclaw_monty_review_dispositions d WHERE d.attempt_id=w.attempt_id)
        FROM brassclaw_monty_review_work w WHERE attempt_id>$1 ORDER BY attempt_id LIMIT 51",
            &[&after],
        )
        .await
        .map_err(unavailable)?;
    let items: Vec<_> = rows
        .iter()
        .take(50)
        .map(|r| PostTurnReviewSummary {
            source_run_id: r.get::<_, Uuid>(0).to_string(),
            attempt_id: r.get::<_, Uuid>(1).to_string(),
            phase: r.get(2),
            model_dispatch_count: r.get(3),
            settlement_recorded: r.get(4),
            disposition_count: r.get(5),
        })
        .collect();
    let next_after = (rows.len() > 50).then(|| items.last().unwrap().attempt_id.clone());
    Ok(PostTurnReviewList { items, next_after })
}

// One PostgreSQL statement gives one coherent observation. Raw exact byte strings
// are preserved inside the JSON envelope; display never executes their contents.
async fn snapshot<C: GenericClient + Sync>(
    client: &C,
    attempt: Uuid,
) -> Result<(String, String, String), RecipeStoreError> {
    let row = client.query_opt("WITH observed AS (
        SELECT w.phase,jsonb_build_object('format','monty-review-inspection/1',
            'work',to_jsonb(w),'event',to_jsonb(e),
            'journal_integrity',w.event_checksum=e.event_checksum
                AND e.event_checksum=encode(sha256(convert_to(e.event_bytes,'UTF8')),'hex')
                AND NOT EXISTS(SELECT 1 FROM brassclaw_monty_review_operations o
                    WHERE o.attempt_id=w.attempt_id AND (
                        o.input_checksum<>encode(sha256(convert_to(o.input_bytes,'UTF8')),'hex')
                        OR (o.output_bytes IS NOT NULL AND o.output_checksum IS DISTINCT FROM
                            encode(sha256(convert_to(o.output_bytes,'UTF8')),'hex')))),
            'operations',COALESCE((SELECT jsonb_agg(to_jsonb(o) ORDER BY o.operation)
                FROM brassclaw_monty_review_operations o WHERE o.attempt_id=w.attempt_id),'[]'::jsonb),
            'settlement',(SELECT to_jsonb(s) FROM brassclaw_monty_review_settlements s
                WHERE s.attempt_id=w.attempt_id))::text AS bytes
        FROM brassclaw_monty_review_work w JOIN brassclaw_monty_review_events e ON e.run_id=w.source_run_id
        WHERE w.attempt_id=$1)
        SELECT phase,CASE WHEN octet_length(bytes)<=33554432 THEN bytes END,
            encode(sha256(convert_to(bytes,'UTF8')),'hex') FROM observed", &[&attempt])
        .await.map_err(unavailable)?.ok_or_else(|| RecipeStoreError::NotFound("review attempt not found".into()))?;
    let bytes: Option<String> = row.get(1);
    Ok((
        row.get(0),
        bytes.ok_or_else(|| {
            RecipeStoreError::Invalid("inspection exceeds 32 MiB; no evidence was truncated".into())
        })?,
        row.get(2),
    ))
}

pub(crate) async fn inspect(
    pool: &Arc<PgPool>,
    attempt: &str,
) -> Result<PostTurnReviewInspection, RecipeStoreError> {
    let attempt = id(attempt)?;
    let mut client = pool.get().await.map_err(unavailable)?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(unavailable)?;
    let (_, evidence_bytes, evidence_checksum) = snapshot(&*tx, attempt).await?;
    let dispositions = tx.query("SELECT disposition_id,evidence_checksum,actor,note FROM brassclaw_monty_review_dispositions
        WHERE attempt_id=$1 ORDER BY recorded_at DESC,disposition_id LIMIT 50", &[&attempt]).await.map_err(unavailable)?
        .iter().map(|r| ReviewDispositionView { receipt: receipt(r.get(0), attempt, r.get(1)), actor: r.get(2), note: r.get(3) }).collect();
    let disposition_count = tx
        .query_one(
            "SELECT count(*) FROM brassclaw_monty_review_dispositions WHERE attempt_id=$1",
            &[&attempt],
        )
        .await
        .map_err(unavailable)?
        .get(0);
    tx.commit().await.map_err(unavailable)?;
    Ok(PostTurnReviewInspection {
        evidence_bytes,
        evidence_checksum,
        dispositions,
        disposition_count,
    })
}
fn receipt(
    disposition_id: Uuid,
    attempt: Uuid,
    evidence_checksum: String,
) -> ReviewDispositionReceipt {
    ReviewDispositionReceipt {
        disposition_id: disposition_id.to_string(),
        attempt_id: attempt.to_string(),
        evidence_checksum,
        retention_released: false,
        work_replayed: false,
    }
}

pub(crate) async fn record(
    pool: &Arc<PgPool>,
    actor: &str,
    attempt: &str,
    request: ReviewDispositionRequest,
) -> Result<ReviewDispositionReceipt, RecipeStoreError> {
    let attempt = id(attempt)?;
    let disposition = id(&request.disposition_id)?;
    if actor.is_empty()
        || actor.len() > 512
        || request.note.trim().is_empty()
        || request.note.len() > 16384
        || request.evidence_checksum.len() != 64
        || !request
            .evidence_checksum
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(RecipeStoreError::Invalid(
            "invalid review observation".into(),
        ));
    }
    let mut client = pool.get().await.map_err(unavailable)?;
    let tx = client.transaction().await.map_err(unavailable)?;
    // Serialize same-ID retries even if their other fields differ. The lock is
    // only for this management write, not permission to dispatch or resume work.
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        &[&disposition.to_string()],
    )
    .await
    .map_err(unavailable)?;
    if let Some(row) = tx
        .query_opt(
            "SELECT attempt_id,actor,evidence_checksum,note
        FROM brassclaw_monty_review_dispositions WHERE disposition_id=$1",
            &[&disposition],
        )
        .await
        .map_err(unavailable)?
    {
        if row.get::<_, Uuid>(0) != attempt
            || row.get::<_, String>(1) != actor
            || row.get::<_, String>(2) != request.evidence_checksum
            || row.get::<_, String>(3) != request.note
        {
            return Err(RecipeStoreError::Conflict(
                "observation ID already retains different evidence".into(),
            ));
        }
        tx.commit().await.map_err(unavailable)?;
        return Ok(receipt(disposition, attempt, request.evidence_checksum));
    }
    let (phase, evidence, checksum) = snapshot(&*tx, attempt).await?;
    if !matches!(phase.as_str(), "failed" | "uncertain" | "incomplete") {
        return Err(RecipeStoreError::Conflict(
            "only stopped reviews accept quarantine observations".into(),
        ));
    }
    if checksum != request.evidence_checksum {
        return Err(RecipeStoreError::Conflict(
            "review evidence changed; inspect again".into(),
        ));
    }
    tx.execute("INSERT INTO brassclaw_monty_review_dispositions
        (disposition_id,attempt_id,actor,evidence_bytes,evidence_checksum,note) VALUES($1,$2,$3,$4,$5,$6)",
        &[&disposition,&attempt,&actor,&evidence,&checksum,&request.note]).await.map_err(unavailable)?;
    tx.commit().await.map_err(unavailable)?;
    Ok(receipt(disposition, attempt, checksum))
}
