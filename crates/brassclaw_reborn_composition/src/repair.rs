//! Conservative recovery for the checksummed system prompt skills.
//!
//! Existing rows are never overwritten unless the caller explicitly confirms
//! the operation. Exact pending checksum matches only need their validation
//! status repaired. Differing content or a non-pending lifecycle decision
//! requires confirmation before the row is reset.

#![forbid(unsafe_code)]

use brassclaw_host_api::SYSTEM_RESERVED_ID;
use thiserror::Error;

use crate::booted_db::BootedDb;
use crate::builtin_bootstrap::{
    CODEACT_POSTAMBLE_SEED, CODEACT_PREAMBLE_SEED, COMPACTION_SUMMARIZER_SEED,
    DEFAULT_ORCHESTRATOR_SEED, DIRECTION_CODER_SEED, DIRECTION_EXPLORER_SEED,
    DIRECTION_GENERAL_SEED, DIRECTION_RESEARCHER_SEED, FAILURE_EXPLANATION_SEED, SEMPAI_AUDIT_SEED,
};
use crate::checksum::sha256_hex;

const SEED_USER: &str = SYSTEM_RESERVED_ID;
const SEED_AGENT: &str = "default";
const SEED_PROJECT: &str = "system";

#[derive(Debug, Error)]
pub enum RepairError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error(
        "repair would overwrite components with content that differs from this binary; explicit operator confirmation is required: {names}"
    )]
    ConfirmationRequired { names: String },
}

#[derive(Debug, Default)]
pub struct RepairReport {
    /// Rows inserted or safely returned to validated status.
    pub restored: u64,
    /// Existing rows whose content differs from the compiled-in seed.
    pub requires_confirmation: Vec<String>,
}

struct SkillSeed {
    name: &'static str,
    description: &'static str,
    body: &'static str,
    class_code: i16,
}

/// Restore the checksummed system prompt skills.
///
/// Dry runs report differences and always roll back. In live mode, differing
/// rows are overwritten only when their exact name is confirmed.
/// Exact content checksums are repaired without rewriting prose or metadata.
pub async fn repair_builtin_components(
    booted_db: &BootedDb,
    tenant_id: &str,
    dry_run: bool,
    confirmed_overwrite: &[String],
) -> Result<RepairReport, RepairError> {
    let pool = booted_db.pool();
    let mut client = pool.get().await.map_err(|e| RepairError::Pool {
        reason: e.to_string(),
    })?;
    let transaction = client.transaction().await.map_err(|e| RepairError::Db {
        reason: e.to_string(),
    })?;

    let seeds = [
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

    let mut report = RepairReport::default();
    let mut overwrite = Vec::new();
    for seed in &seeds {
        let expected = sha256_hex(seed.body);
        let existing = transaction
            .query_opt(
                "SELECT body, validation_status FROM reborn_skills
                  WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                    AND name=$5 AND source='system' FOR UPDATE",
                &[
                    &tenant_id,
                    &SEED_USER,
                    &SEED_AGENT,
                    &SEED_PROJECT,
                    &seed.name,
                ],
            )
            .await
            .map_err(|e| RepairError::Db {
                reason: e.to_string(),
            })?;

        match existing {
            None => {
                report.restored += 1;
                if !dry_run {
                    transaction
                        .execute(
                            "INSERT INTO reborn_skills
                                (tenant_id,user_id,agent_id,project_id,name,description,body,class_code,
                                 consumer_tags,intent_examples,source,validation_status,content_checksum)
                             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'{}','[]','system','validated',$9)",
                            &[&tenant_id, &SEED_USER, &SEED_AGENT, &SEED_PROJECT, &seed.name,
                              &seed.description, &seed.body, &seed.class_code, &expected],
                        )
                        .await
                        .map_err(|e| RepairError::Db { reason: format!("failed to insert '{}': {e}", seed.name) })?;
                }
            }
            Some(row) => {
                let body: String = row.get(0);
                let status: String = row.get(1);
                let actual = sha256_hex(&body);
                if body == seed.body && actual == expected && status == "validated" {
                    // Already correct.
                } else if body == seed.body && actual == expected && status == "pending" {
                    report.restored += 1;
                    if !dry_run {
                        transaction
                                .execute(
                                    "UPDATE reborn_skills SET validation_status='validated', content_checksum=$6, updated_at=now()
                                      WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                                        AND name=$5 AND source='system'",
                                    &[&tenant_id, &SEED_USER, &SEED_AGENT, &SEED_PROJECT, &seed.name, &expected],
                                )
                                .await
                                .map_err(|e| RepairError::Db { reason: format!("failed to validate '{}': {e}", seed.name) })?;
                    }
                } else {
                    report.requires_confirmation.push(seed.name.to_string());
                    overwrite.push((seed, expected));
                }
            }
        }
    }

    let unconfirmed: Vec<String> = report
        .requires_confirmation
        .iter()
        .filter(|name| !confirmed_overwrite.contains(name))
        .cloned()
        .collect();
    if !dry_run && !unconfirmed.is_empty() {
        return Err(RepairError::ConfirmationRequired {
            names: unconfirmed.join(", "),
        });
    }

    if !dry_run {
        for (seed, expected) in overwrite {
            transaction
                .execute(
                    "INSERT INTO reborn_skills
                        (tenant_id,user_id,agent_id,project_id,name,description,body,class_code,
                         consumer_tags,intent_examples,source,validation_status,content_checksum)
                     VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'{}','[]','system','validated',$9)
                     ON CONFLICT (tenant_id,user_id,agent_id,project_id,name)
                     DO UPDATE SET description=EXCLUDED.description, body=EXCLUDED.body,
                       class_code=EXCLUDED.class_code, content_checksum=EXCLUDED.content_checksum,
                       validation_status='validated', source='system', updated_at=now()",
                    &[
                        &tenant_id,
                        &SEED_USER,
                        &SEED_AGENT,
                        &SEED_PROJECT,
                        &seed.name,
                        &seed.description,
                        &seed.body,
                        &seed.class_code,
                        &expected,
                    ],
                )
                .await
                .map_err(|e| RepairError::Db {
                    reason: format!("failed to restore '{}': {e}", seed.name),
                })?;
            report.restored += 1;
        }
    }

    if dry_run {
        transaction.rollback().await.map_err(|e| RepairError::Db {
            reason: e.to_string(),
        })?;
    } else {
        transaction.commit().await.map_err(|e| RepairError::Db {
            reason: e.to_string(),
        })?;
    }
    Ok(report)
}
