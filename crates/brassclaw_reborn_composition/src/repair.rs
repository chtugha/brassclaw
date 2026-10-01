//! `brassclaw repair` — force-overwrite all `source='system'` component rows.
//!
//! The normal seeder uses `ON CONFLICT DO NOTHING` — it will not overwrite a
//! corrupted or stale row. This module provides a separate SQL path that
//! unconditionally overwrites all `source='system'` prose rows with the
//! compiled-in seed values, giving operators a deterministic, auditable
//! recovery path after the boot content integrity check fails.
//!
//! # Usage
//!
//! ```text
//! brassclaw repair [--dry-run]
//! ```
//!
//! With `--dry-run`: executes inside a transaction that is always rolled back;
//! prints what would be overwritten without committing.

#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_host_api::SYSTEM_RESERVED_ID;
use brassclaw_pg::PgPool;
use thiserror::Error;

use crate::booted_db::BootedDb;
use crate::builtin_bootstrap::{
    CODEACT_POSTAMBLE_SEED, CODEACT_PREAMBLE_SEED, COMPACTION_SUMMARIZER_SEED,
    DEFAULT_ORCHESTRATOR_SEED, DIRECTION_CODER_SEED, DIRECTION_EXPLORER_SEED,
    DIRECTION_GENERAL_SEED, DIRECTION_RESEARCHER_SEED, FAILURE_EXPLANATION_SEED,
    SEMPAI_AUDIT_SEED,
};
use crate::checksum::sha256_hex;

/// Marker values for seeded builtins — matches builtin_bootstrap.rs.
const SEED_USER: &str = SYSTEM_RESERVED_ID;
const SEED_AGENT: &str = "default";
const SEED_PROJECT: &str = "system";

/// Errors raised by `repair_builtin_components`.
#[derive(Debug, Error)]
pub enum RepairError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
}

/// Report returned by `repair_builtin_components`.
#[derive(Debug)]
pub struct RepairReport {
    /// Number of rows restored (or that would be restored in dry-run mode).
    pub restored: u64,
}

/// Data for one system skill row to repair.
struct SkillSeed {
    name: &'static str,
    description: &'static str,
    body: &'static str,
    class_code: i16,
}

/// Force-reseed all `source='system'` skill component rows from compiled-in
/// seed constants, using `ON CONFLICT DO UPDATE` to overwrite existing rows.
/// This is intentionally different from the normal seeder's `DO NOTHING`.
///
/// When `dry_run` is `true`, executes inside a transaction that is always
/// rolled back; prints what would be overwritten without committing.
pub async fn repair_builtin_components(
    booted_db: &BootedDb,
    tenant_id: &str,
    dry_run: bool,
) -> Result<RepairReport, RepairError> {
    let pool: &Arc<PgPool> = booted_db.pool();
    let client = pool.get().await.map_err(|e| RepairError::Pool {
        reason: e.to_string(),
    })?;

    // --- Define seed rows for each covered component ---
    let skill_seeds: &[SkillSeed] = &[
        SkillSeed {
            name: "orchestrator:main",
            description: "Main orchestrator loop (class 10)",
            body: DEFAULT_ORCHESTRATOR_SEED,
            class_code: 10,
        },
        SkillSeed {
            name: "codeact_preamble",
            description: "CodeAct system prompt preamble (class 10)",
            body: CODEACT_PREAMBLE_SEED,
            class_code: 10,
        },
        SkillSeed {
            name: "codeact_postamble",
            description: "CodeAct system prompt postamble (class 10)",
            body: CODEACT_POSTAMBLE_SEED,
            class_code: 10,
        },
        SkillSeed {
            name: "failure_explanation",
            description: "Failure explanation system prompt (class 3)",
            body: FAILURE_EXPLANATION_SEED,
            class_code: 3,
        },
        SkillSeed {
            name: "compaction_summarizer_fresh",
            description: "Compaction summarizer prompt — fresh conversation (class 10)",
            body: COMPACTION_SUMMARIZER_SEED,
            class_code: 10,
        },
        SkillSeed {
            name: "sempai_audit",
            description: "Sempai audit persona (class 10)",
            body: SEMPAI_AUDIT_SEED,
            class_code: 10,
        },
        SkillSeed {
            name: "subagent:direction:general",
            description: "Subagent direction: general (class 10)",
            body: DIRECTION_GENERAL_SEED,
            class_code: 10,
        },
        SkillSeed {
            name: "subagent:direction:researcher",
            description: "Subagent direction: researcher (class 10)",
            body: DIRECTION_RESEARCHER_SEED,
            class_code: 10,
        },
        SkillSeed {
            name: "subagent:direction:explorer",
            description: "Subagent direction: explorer (class 10)",
            body: DIRECTION_EXPLORER_SEED,
            class_code: 10,
        },
        SkillSeed {
            name: "subagent:direction:coder",
            description: "Subagent direction: coder (class 10)",
            body: DIRECTION_CODER_SEED,
            class_code: 10,
        },
    ];

    let mut restored: u64 = 0;

    if dry_run {
        // In dry-run mode, just count which rows would be affected without
        // actually running any INSERTs.
        let rows = client
            .query(
                "SELECT name FROM reborn_skills
                  WHERE tenant_id = $1 AND user_id = $2
                    AND agent_id = $3 AND project_id = $4
                    AND source = 'system'",
                &[&tenant_id, &SEED_USER, &SEED_AGENT, &SEED_PROJECT],
            )
            .await
            .map_err(|e| RepairError::Db {
                reason: e.to_string(),
            })?;
        let existing_names: std::collections::HashSet<String> =
            rows.into_iter().map(|r| r.get::<_, String>(0)).collect();

        for seed in skill_seeds {
            if existing_names.contains(seed.name) {
                println!("  [dry-run] Would restore: {}", seed.name);
            } else {
                println!("  [dry-run] Would insert (new): {}", seed.name);
            }
            restored += 1;
        }
        return Ok(RepairReport { restored });
    }

    // Live mode: upsert each row with ON CONFLICT DO UPDATE.
    for seed in skill_seeds {
        let checksum = sha256_hex(seed.body);
        client
            .execute(
                "INSERT INTO reborn_skills
                    (tenant_id, user_id, agent_id, project_id,
                     name, description, body, class_code,
                     consumer_tags, intent_examples,
                     source, validation_status, content_checksum)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '{}', '[]',
                         'system', 'validated', $9)
                 ON CONFLICT (tenant_id, user_id, agent_id, project_id, name)
                 DO UPDATE SET
                     body              = EXCLUDED.body,
                     description       = EXCLUDED.description,
                     class_code        = EXCLUDED.class_code,
                     content_checksum  = EXCLUDED.content_checksum,
                     validation_status = 'validated',
                     source            = 'system'",
                &[
                    &tenant_id,
                    &SEED_USER,
                    &SEED_AGENT,
                    &SEED_PROJECT,
                    &seed.name,
                    &seed.description,
                    &seed.body,
                    &seed.class_code,
                    &checksum,
                ],
            )
            .await
            .map_err(|e| RepairError::Db {
                reason: format!("failed to upsert '{}': {e}", seed.name),
            })?;
        restored += 1;
    }

    Ok(RepairReport { restored })
}
