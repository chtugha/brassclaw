//! One scoped, ordered live queue projection. No Work state or scheduler.
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_product_workflow::{
    RecipeKind, RecipeStoreError, ValidationQueueFilter, ValidationQueueItem,
    ValidationQueueListResponse,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    format: String,
    project: String,
    scope_checksum: String,
    bucket: ValidationQueueFilter,
    at: DateTime<Utc>,
    class: i16,
    component: Uuid,
}
fn invalid() -> RecipeStoreError {
    RecipeStoreError::Invalid("invalid queue page cursor".into())
}
fn scope_checksum(scope: &ComponentScope) -> Result<String, RecipeStoreError> {
    Ok(hex::encode(Sha256::digest(
        serde_json::to_vec(&[
            &scope.tenant_id,
            &scope.user_id,
            &scope.agent_id,
            &scope.project_id,
        ])
        .map_err(|_| invalid())?,
    )))
}
fn cursor(
    value: &str,
    scope: &ComponentScope,
    bucket: ValidationQueueFilter,
) -> Result<Cursor, RecipeStoreError> {
    let value: Cursor =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid())?)
            .map_err(|_| invalid())?;
    if value.format != "validation-queue-page/1"
        || value.project != scope.project_id
        || value.scope_checksum != scope_checksum(scope)?
        || value.bucket != bucket
        || value.component.is_nil()
        || !matches!(value.class,0|1..=3|13|21..=23)
    {
        return Err(invalid());
    }
    Ok(value)
}

fn projection_sql() -> String {
    // Content/source text is absent; all class branches use actual scoped rows.
    let tables = [
        "reborn_tools",
        "reborn_skills",
        "reborn_tool_skills",
        "reborn_recipes",
        "reborn_python_code",
        "reborn_extension_catalogues",
    ];
    let components=tables.iter().map(|table| {
        let trigger=if *table=="reborn_recipes" {"trigger"} else {"NULL::jsonb"};
        format!("SELECT id,tenant_id,user_id,agent_id,project_id,class_code,name,description,consumer_tags,source,validation_status,created_at,{trigger} AS trigger
            FROM {table} WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND class_code IN (0,1,2,3,13,21,22,23)")
    }).collect::<Vec<_>>().join(" UNION ALL ");
    format!("WITH components AS ({components}), queue_rows AS (
        SELECT * FROM reborn_validation_queue WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND component_class IN (0,1,2,3,13,21,22,23)
      ), projected AS (
        SELECT COALESCE(c.id,q.component_id) AS id,COALESCE(c.class_code,q.component_class) AS class_code,
          COALESCE(c.name,'Missing component') AS name,COALESCE(c.description,'') AS description,
          COALESCE(c.consumer_tags,ARRAY[]::text[]) AS consumer_tags,COALESCE(c.source,'') AS source,
          COALESCE(q.state,(CASE c.validation_status WHEN 'pending' THEN 1 WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
            WHEN 'review_requested' THEN 2 WHEN 'upgrade_queued' THEN 2 WHEN 'rejected' THEN 3 WHEN 'garbage' THEN 4 ELSE 0 END)::smallint) AS state,
          CASE WHEN q.id IS NULL THEN c.validation_status WHEN q.state=1 THEN 'pending' WHEN q.state=2 THEN 'auto_passed'
            WHEN q.state=3 THEN 'rejected' ELSE 'garbage' END AS validation_status,
          COALESCE(q.counter,0) AS counter,q.review_feedback,COALESCE(q.validation_errors,ARRAY[]::text[]) AS validation_errors,
          COALESCE(q.submitted_at,c.created_at) AS sort_time,COALESCE(c.created_at,q.submitted_at) AS created_at,
          q.q2_actor,q.id AS queue_id,q.original_binding_required,b.submission_id,
          COALESCE(s.subject_bytes::jsonb ? 'function_partner',false) AS joint,c.trigger
        FROM components c FULL OUTER JOIN queue_rows q ON q.component_id=c.id AND q.component_class=c.class_code
          AND q.tenant_id=c.tenant_id AND q.user_id=c.user_id AND q.agent_id=c.agent_id AND q.project_id=c.project_id
        LEFT JOIN reborn_component_review_queue_bindings b ON b.queue_id=q.id
        LEFT JOIN reborn_component_review_submissions s ON s.submission_id=b.submission_id
      )")
}

/// The existing Recipe list bound is a page size across the complete relation.
/// Keyset traversal is live: lifecycle changes may change membership; refresh
/// starts a new traversal. Every page keeps the same global sort and scope.
pub(crate) async fn list_page(
    client: &tokio_postgres::Client,
    scope: &ComponentScope,
    filter: ValidationQueueFilter,
    after: Option<&str>,
    page_size: i64,
    reject_threshold: u8,
) -> Result<ValidationQueueListResponse, RecipeStoreError> {
    let after = after
        .map(|value| cursor(value, scope, filter))
        .transpose()?;
    let at = after.as_ref().map(|c| c.at);
    let class = after.as_ref().map(|c| c.class);
    let component = after.as_ref().map(|c| c.component);
    let threshold = i32::from(reject_threshold);
    let lookahead = page_size
        .checked_add(1)
        .filter(|n| *n > 1)
        .ok_or_else(invalid)?;
    let condition = match filter {
        ValidationQueueFilter::Auto => "state=1",
        ValidationQueueFilter::Manual => "state=2",
        ValidationQueueFilter::Revision => "state=3 AND counter<$5",
        ValidationQueueFilter::Rejection => "state=4 OR (state=3 AND counter>=$5)",
    };
    // Always type/use the owner threshold parameter, including non-rejection pages.
    let sql=format!("{} SELECT * FROM projected WHERE ({condition}) AND $5::int>=0
        AND ($6::timestamptz IS NULL OR (sort_time,class_code,id)>($6::timestamptz,$7::smallint,$8::uuid))
        ORDER BY sort_time,class_code,id LIMIT $9",projection_sql());
    let mut rows = client
        .query(
            &sql,
            &[
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &threshold,
                &at,
                &class,
                &component,
                &lookahead,
            ],
        )
        .await
        .map_err(|_| RecipeStoreError::Unavailable("original queue page unavailable".into()))?;
    let has_more = rows.len() as i64 > page_size;
    if has_more {
        rows.pop();
    }
    let next_cursor = if has_more {
        let last = rows.last().ok_or_else(invalid)?;
        Some(
            URL_SAFE_NO_PAD.encode(
                serde_json::to_vec(&Cursor {
                    format: "validation-queue-page/1".into(),
                    project: scope.project_id.clone(),
                    scope_checksum: scope_checksum(scope)?,
                    bucket: filter,
                    at: last.get("sort_time"),
                    class: last.get("class_code"),
                    component: last.get("id"),
                })
                .map_err(|_| invalid())?,
            ),
        )
    } else {
        None
    };
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let class: i16 = row.get("class_code");
        let state: i16 = row.get("state");
        let counter: i32 = row.get("counter");
        let tags: Vec<String> = row.get("consumer_tags");
        let label = match class {
            0 => "TOOL",
            1..=3 => "SKILL",
            13 => "TOOLSKILL",
            21 => "RECIPE",
            22 => "PYTHONCODE",
            23 => "EXTENSIONCATALOGUE",
            _ => unreachable!(),
        };
        let trigger: Option<serde_json::Value> = row.get("trigger");
        let trigger_summary = trigger
            .as_ref()
            .and_then(|t| t.get("payload"))
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| v.to_string())
            })
            .unwrap_or_default();
        items.push(ValidationQueueItem {
            original_review: crate::validation_queue::original_review_view(
                class,
                row.get("queue_id"),
                row.get("original_binding_required"),
                row.get("submission_id"),
                row.get("joint"),
            ),
            queued_at: Some(
                row.get::<_, DateTime<Utc>>("sort_time")
                    .to_rfc3339_opts(SecondsFormat::Nanos, true),
            ),
            id: row.get::<_, Uuid>("id").to_string(),
            name: row.get("name"),
            item_type: match class {
                21 => RecipeKind::Recipe,
                13 => RecipeKind::ToolSkill,
                _ => RecipeKind::Component,
            },
            category: label.to_ascii_lowercase(),
            description: row.get("description"),
            trigger_summary,
            estimated_tokens: None,
            validation_status: row.get("validation_status"),
            validation_errors: row.get("validation_errors"),
            review_feedback: row.get("review_feedback"),
            review_attempts: counter.max(0) as u32,
            similarity_parent_id: None,
            created_at: row.get::<_, DateTime<Utc>>("created_at").to_rfc3339(),
            source: row.get("source"),
            class_code: class as u16,
            class_label: label.into(),
            queue_code: match state {
                1 => "q1_auto",
                2 => "q2_manual",
                3 if counter < threshold => "q3_revision",
                4 => "garbage",
                _ => "q4_rejection",
            }
            .into(),
            validator_tag_present: tags.iter().any(|t| t == "05:validator"),
            consumer_tags: tags,
            llm_audit_status: "not_applicable".into(),
            llm_audit_findings: vec![],
            q2_actor: row.get("q2_actor"),
        });
    }
    Ok(ValidationQueueListResponse {
        items,
        page_size: u32::try_from(page_size).map_err(|_| invalid())?,
        next_cursor,
    })
}

pub(crate) async fn count_by_status(
    client: &tokio_postgres::Client,
    scope: &ComponentScope,
    status: &str,
) -> Result<u32, RecipeStoreError> {
    let sql = format!(
        "{} SELECT count(*) FROM projected WHERE validation_status=$5",
        projection_sql()
    );
    let row = client
        .query_one(
            &sql,
            &[
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &status,
            ],
        )
        .await
        .map_err(|_| RecipeStoreError::Unavailable("queue count unavailable".into()))?;
    u32::try_from(row.get::<_, i64>(0)).map_err(|_| {
        RecipeStoreError::Unavailable("queue count exceeds existing wire capacity".into())
    })
}
