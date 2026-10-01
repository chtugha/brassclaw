//! Boot-time content integrity check for `source='system'` component rows.
//!
//! Distinct from [`crate::boot_integrity`] (Phase N) which handles
//! queue-consistency (unqueued non-validated rows). This module verifies the
//! prose content of `source='system'` rows against both:
//!
//! 1. The stored `content_checksum` column (detects DB mutation after seeding).
//! 2. The compile-time [`crate::builtin_bootstrap::EXPECTED_CHECKSUMS`]
//!    (detects binary update without `brassclaw repair`).
//!
//! Covered tables: `reborn_skills` (column `body`), `reborn_tool_skills`
//! (column `content`), `reborn_python_code` (column `content`).
//!
//! The check is a hard boot error — if any mismatch is found the process
//! returns [`ContentIntegrityError::Corrupted`] and the `RebornWebuiBundle`
//! is not returned to the caller. Run `brassclaw repair` to restore all
//! `source='system'` rows from compiled-in seed values.

#![forbid(unsafe_code)]

use thiserror::Error;

use crate::booted_db::BootedDb;
use crate::builtin_bootstrap::EXPECTED_CHECKSUMS;
use crate::checksum::sha256_hex;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Outcome of a successful (non-erroring) content integrity check.
#[derive(Debug)]
pub enum ContentIntegrityOutcome {
    /// All rows checked; none were corrupted.
    Ok { checked: usize },
    /// One or more rows have mismatched checksums.
    Corrupted(Vec<ContentIntegrityMismatch>),
}

/// A single mismatch found during the integrity check.
#[derive(Debug)]
pub struct ContentIntegrityMismatch {
    /// The table the row lives in (e.g. `"reborn_skills"`).
    pub table: &'static str,
    /// The component's `name` field.
    pub name: String,
    /// Expected SHA-256 hex (from `EXPECTED_CHECKSUMS`).
    pub expected: String,
    /// Actual SHA-256 hex (re-computed from the DB row's prose field).
    pub actual: String,
}

/// Errors raised by [`run_content_integrity_check`].
#[derive(Debug, Error)]
pub enum ContentIntegrityError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("content integrity check found corrupted component(s)")]
    Corrupted(Vec<ContentIntegrityMismatch>),
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the boot-time content integrity check for all `source='system'`
/// rows in the three prose-bearing tables.
///
/// Performs a two-way check for each row:
/// - Re-computes `sha256_hex(prose)` and compares to the stored
///   `content_checksum`. Mismatch → row was mutated after seeding (DB
///   corruption or manual edit).
/// - Compares the stored `content_checksum` to
///   `EXPECTED_CHECKSUMS.get(name)`. Mismatch → binary was updated;
///   `brassclaw repair` has not been run yet.
///
/// All mismatches are collected before returning so the operator sees the
/// complete picture in one boot. Returns `ContentIntegrityOutcome::Ok` when
/// all rows pass, or `ContentIntegrityOutcome::Corrupted` on any mismatch.
pub async fn run_content_integrity_check(
    booted_db: &BootedDb,
) -> Result<ContentIntegrityOutcome, ContentIntegrityError> {
    let pool = booted_db.pool();
    let client = pool.get().await.map_err(|e| ContentIntegrityError::Pool {
        reason: e.to_string(),
    })?;

    let mut mismatches: Vec<ContentIntegrityMismatch> = Vec::new();
    let mut checked: usize = 0;

    // --- reborn_skills (prose column: body) ---
    {
        let rows = client
            .query(
                "SELECT name, body, content_checksum
                   FROM reborn_skills
                  WHERE source = 'system' AND content_checksum IS NOT NULL",
                &[],
            )
            .await
            .map_err(|e| ContentIntegrityError::Db {
                reason: e.to_string(),
            })?;
        for row in &rows {
            let name: String = row.get(0);
            let body: String = row.get(1);
            let stored_checksum: String = row.get(2);
            check_row(
                "reborn_skills",
                &name,
                &body,
                &stored_checksum,
                &mut mismatches,
            );
            checked += 1;
        }
    }

    // --- reborn_tool_skills (prose column: content) ---
    {
        let rows = client
            .query(
                "SELECT name, content, content_checksum
                   FROM reborn_tool_skills
                  WHERE source = 'system' AND content_checksum IS NOT NULL",
                &[],
            )
            .await
            .map_err(|e| ContentIntegrityError::Db {
                reason: e.to_string(),
            })?;
        for row in &rows {
            let name: String = row.get(0);
            let content: String = row.get(1);
            let stored_checksum: String = row.get(2);
            check_row(
                "reborn_tool_skills",
                &name,
                &content,
                &stored_checksum,
                &mut mismatches,
            );
            checked += 1;
        }
    }

    // --- reborn_python_code (prose column: content) ---
    {
        let rows = client
            .query(
                "SELECT name, content, content_checksum
                   FROM reborn_python_code
                  WHERE source = 'system' AND content_checksum IS NOT NULL",
                &[],
            )
            .await
            .map_err(|e| ContentIntegrityError::Db {
                reason: e.to_string(),
            })?;
        for row in &rows {
            let name: String = row.get(0);
            let content: String = row.get(1);
            let stored_checksum: String = row.get(2);
            check_row(
                "reborn_python_code",
                &name,
                &content,
                &stored_checksum,
                &mut mismatches,
            );
            checked += 1;
        }
    }

    if mismatches.is_empty() {
        Ok(ContentIntegrityOutcome::Ok { checked })
    } else {
        Ok(ContentIntegrityOutcome::Corrupted(mismatches))
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Perform the two-way check for one row and push any mismatch into `out`.
fn check_row(
    table: &'static str,
    name: &str,
    prose: &str,
    stored_checksum: &str,
    out: &mut Vec<ContentIntegrityMismatch>,
) {
    let computed = sha256_hex(prose);

    // Check 1: recomputed digest must equal stored digest (detects DB mutation).
    if computed != stored_checksum {
        out.push(ContentIntegrityMismatch {
            table,
            name: name.to_string(),
            expected: stored_checksum.to_string(),
            actual: computed,
        });
        return; // no point doing check 2 if the stored checksum itself was tampered
    }

    // Check 2: stored digest must equal compile-time expected digest (detects
    // binary update without repair).
    if let Some(expected) = EXPECTED_CHECKSUMS.get(name) {
        if stored_checksum != expected.as_str() {
            out.push(ContentIntegrityMismatch {
                table,
                name: name.to_string(),
                expected: expected.clone(),
                actual: stored_checksum.to_string(),
            });
        }
    }
    // Rows not in EXPECTED_CHECKSUMS are operator-added system rows — not
    // subject to the compile-time check, only to the DB-mutation check above.
}
