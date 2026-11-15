//! Retire only untouched legacy seed requests, preserving their exact originals.
//! Canonical definitions come from the seeder, never a source label or DB scan.
use std::{cell::RefCell, collections::BTreeMap, future::Future};

use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_pg::PgPool;
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Default)]
struct Package {
    definitions: BTreeMap<(String, String), Value>,
    conflict: bool,
}
tokio::task_local! { static PACKAGE: RefCell<Package>; }

pub(crate) async fn scope<T>(work: impl Future<Output = T>) -> T {
    PACKAGE.scope(RefCell::new(Package::default()), work).await
}

/// Only canonical constructors call this inside shared component boot.
pub(crate) fn expect(table: &str, definition: &impl Serialize) {
    let value = serde_json::to_value(definition).expect("seed definitions serialize");
    let key = format!(
        "{}",
        json!([
            value["tenant_id"],
            value["user_id"],
            value["agent_id"],
            value["project_id"],
            value["name"]
        ])
    );
    let _ = PACKAGE.try_with(|state| {
        let mut state = state.borrow_mut();
        let key = (table.to_owned(), key);
        if let Some(prior) = state.definitions.get(&key) {
            if prior != &value {
                state.conflict = true;
            }
        } else {
            state.definitions.insert(key, value);
        }
    });
}

/// Catalogues assemble their canonical children incrementally. Compare sets,
/// because the existing append SQL does not preserve array ordering.
pub(crate) fn children(tenant: &str, name: &str, ids: &[Uuid]) {
    let _ = PACKAGE.try_with(|state| {
        for ((table, _), value) in &mut state.borrow_mut().definitions {
            if table == "reborn_extension_catalogues"
                && value["tenant_id"] == tenant
                && value["name"] == name
            {
                let array = value["child_component_ids"]
                    .as_array_mut()
                    .expect("catalogue children");
                for id in ids {
                    let id = json!(id);
                    if !array.contains(&id) {
                        array.push(id);
                    }
                }
            }
        }
    });
}

// Explicit /1 projection: unknown columns refuse cleanup. These are only
// generated identity, observed accounting and derived delivery fields.
const GENERATED: &[&str] = &[
    "id",
    "prompt_uid",
    "created_at",
    "updated_at",
    "usage_count",
    "success_count",
    "failure_count",
    "wilson_lower",
    "tier",
    "confidence",
    "last_audit_at",
    "audit_failure_count",
    "content_hash",
    "content_checksum",
    "formatted_content",
    "validation_status",
];

fn canonical_matches(table: &str, actual: &Value, expected: &Value) -> bool {
    if !matches!(
        actual["validation_status"].as_str(),
        Some("validated" | "pending")
    ) {
        return false;
    }
    let mut defaults = json!({
        "dependency_registry":null,"prior_knowledge_content":null,"override_prompt_creation":false,
        "similarity_parent_id":null,"replaces_id":null,"parent_version":null,
        "content_checksum":null,"review_feedback":null,"review_attempts":0,"rejected_at":null,
        "validation_errors":[],"queue_code":null
    });
    let class = match actual["class_code"].as_i64() {
        Some(c) => c,
        None => return false,
    };
    let expected_class = match table {
        "reborn_tools" => 0,
        "reborn_tool_skills" => 13,
        "reborn_skills" => match expected["class_code"].as_i64() {
            Some(c) => c,
            None => return false,
        },
        "reborn_python_code" => 22,
        "reborn_recipes" => 21,
        "reborn_extension_catalogues" => 23,
        "reborn_actions" => 16,
        _ => return false,
    };
    if class != expected_class {
        return false;
    }
    defaults["class_code"] = json!(class);
    if matches!(class, 1..=3 | 10 | 50) {
        defaults.as_object_mut().unwrap().extend(json!({
            "compatibility":"","license":"","allowed_tools":[],"version":"0.0.0",
            "keywords":[],"exclude_keywords":[],"patterns":[],"tags":[],"max_context_tokens":2500,
            "setup_marker":null,"required_binaries":[],"required_env":[],"required_config":[]
        }).as_object().unwrap().clone());
    }
    if class == 21 {
        defaults["status"] = json!("active");
    }
    if class == 16 {
        defaults.as_object_mut().unwrap().extend(json!({"preconditions":null,"error_handling":null,
            "timeout_secs":60,"param_schema":null,"param_template":null,"override_prompt_creation":true}).as_object().unwrap().clone());
    }
    defaults
        .as_object_mut()
        .unwrap()
        .extend(expected.as_object().unwrap().clone());
    let Some(actual) = actual.as_object() else {
        return false;
    };
    for (field, value) in actual {
        if GENERATED.contains(&field.as_str()) {
            continue;
        }
        let Some(expected) = defaults.get(field) else {
            return false;
        };
        if field == "child_component_ids" {
            let mut a = value.as_array().cloned().unwrap_or_default();
            let mut b = expected.as_array().cloned().unwrap_or_default();
            a.sort_by_key(Value::to_string);
            b.sort_by_key(Value::to_string);
            if a != b {
                return false;
            }
        } else if value != expected {
            return false;
        }
    }
    expected
        .as_object()
        .unwrap()
        .keys()
        .all(|k| actual.contains_key(k))
}

/// A single transaction archives before removal; any failure rolls back all
/// retirements. Queue-key, queue-row and component locks match author/Q2 order.
pub(crate) async fn retire(pool: &PgPool) -> Result<u64, String> {
    let (definitions, conflict) = PACKAGE.with(|p| {
        let p = p.borrow();
        (p.definitions.clone(), p.conflict)
    });
    if conflict {
        return Err("conflicting canonical seed definitions in cleanup".into());
    }
    let mut client = pool.get().await.map_err(|e| e.to_string())?;
    let tx = client.transaction().await.map_err(|e| e.to_string())?;
    let mut count = 0;
    // One queue scan in steady state; never issue per-component round trips
    // for an empty queue or acquire locks for every shipped definition.
    let tables = [
        "reborn_tools",
        "reborn_tool_skills",
        "reborn_skills",
        "reborn_python_code",
        "reborn_recipes",
        "reborn_actions",
        "reborn_extension_catalogues",
    ];
    let sql = tables
        .iter()
        .map(|table| {
            format!(
                "SELECT '{table}' AS table_name,c.id,
        jsonb_build_array(c.tenant_id,c.user_id,c.agent_id,c.project_id,c.name) AS key
        FROM {table} c JOIN reborn_validation_queue q ON q.component_id=c.id
        AND q.tenant_id=c.tenant_id AND q.user_id=c.user_id AND q.agent_id=c.agent_id
        AND q.project_id=c.project_id AND q.component_class=c.class_code WHERE q.state=1"
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let candidates = tx.query(&sql, &[]).await.map_err(|e| e.to_string())?;
    for candidate in candidates {
        let table: String = candidate.get(0);
        let id: Uuid = candidate.get(1);
        let key: Value = candidate.get(2);
        let Some(expected) = definitions.get(&(table.clone(), key.to_string())) else {
            continue;
        };
        count += u64::from(retire_one(&tx, &table, id, expected).await?);
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(count)
}

async fn retire_one(
    tx: &tokio_postgres::Transaction<'_>,
    table: &str,
    id: Uuid,
    expected: &Value,
) -> Result<bool, String> {
    let scope = ComponentScope {
        tenant_id: expected["tenant_id"].as_str().unwrap().into(),
        user_id: expected["user_id"].as_str().unwrap().into(),
        agent_id: expected["agent_id"].as_str().unwrap().into(),
        project_id: expected["project_id"].as_str().unwrap().into(),
    };
    crate::validation_queue::ValidationQueueStore::lock_queue_key(tx, &scope, id)
        .await
        .map_err(|e| e.to_string())?;
    let queue = tx.query_opt("SELECT id,(to_jsonb(q))::text,component_class FROM reborn_validation_queue q
            WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND component_id=$5
            AND state=1 AND counter=0 AND proposed_payload IS NULL AND review_feedback IS NULL
            AND cardinality(validation_errors)=0 AND q2_actor IS NULL AND q1_component_bytes IS NULL
            AND q1_queue_bytes IS NULL
            AND NOT EXISTS(SELECT 1 FROM reborn_component_review_queue_bindings b WHERE b.queue_id=q.id)
            AND NOT EXISTS(SELECT 1 FROM reborn_component_review_submissions s WHERE s.component_id=q.component_id)
            FOR UPDATE", &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&id]).await.map_err(|e|e.to_string())?;
    let Some(queue) = queue else {
        return Ok(false);
    };
    let queue_id: Uuid = queue.get(0);
    let queue_bytes: String = queue.get(1);
    let class: i16 = queue.get(2);
    let row = tx
        .query_one(
            &format!("SELECT to_jsonb(c),to_jsonb(c)::text FROM {table} c WHERE id=$1 FOR UPDATE"),
            &[&id],
        )
        .await
        .map_err(|e| e.to_string())?;
    let actual: Value = row.get(0);
    let component_bytes: String = row.get(1);
    if actual["class_code"] != json!(class) || !canonical_matches(table, &actual, expected) {
        return Ok(false);
    }
    let package_bytes = serde_json::to_string(expected).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO reborn_retired_seed_review_requests
            (queue_id,component_id,component_table,queue_bytes,component_bytes,package_bytes,
             queue_checksum,component_checksum,package_checksum)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        &[
            &queue_id,
            &id,
            &table,
            &queue_bytes,
            &component_bytes,
            &package_bytes,
            &crate::checksum::sha256_hex(&queue_bytes),
            &crate::checksum::sha256_hex(&component_bytes),
            &crate::checksum::sha256_hex(&package_bytes),
        ],
    )
    .await
    .map_err(|e| e.to_string())?;
    // Restore only this exact canonical projection, never an edited candidate.
    tx.execute(&format!("UPDATE {table} SET validation_status='validated' WHERE id=$1 AND validation_status='pending'"),&[&id]).await.map_err(|e|e.to_string())?;
    tx.execute(
        "DELETE FROM reborn_validation_queue WHERE id=$1",
        &[&queue_id],
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(true)
}

#[cfg(test)]
#[path = "seed_review_cleanup_tests.rs"]
mod tests;
