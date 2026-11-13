//! Storage and verification of `pure-component-approval/1` records.
//!
//! A `PureComponentApprovalDeclaration` is an untrusted declaration that
//! references existing `reborn_component_review_evidence` records. This module
//! verifies stored checksums, distinct evidence stages and exact component coverage,
//! resolves the full immutable dependency graph, and
//! then stores the approval in `reborn_pure_component_approvals` and its closure
//! in `reborn_pure_component_approval_closure`.
//!
//! Stored declarations do not qualify independent execution in the retained
//! catalogue. Trusted general approval production and exact retained carriage
//! remain separate work; storing a record activates nothing and grants no Tool
//! permission.
//!
//! No public success-write API: only `record_pure_component_approval` may insert.
//!
//! ## Evidence format support
//!
//! Two `component-review-evidence/1` formats are accepted:
//!
//! **Legacy / bootstrap (S-path)** — 9 fields:
//! `format`, `evidence_id`, `kind`, `validation_mode`, `association_checksum`,
//! `components`, `succeeded`, `report`, `reviewed_evidence`.
//! Used by the installation bootstrap writer.
//!
//! **Authored validator (A-path, Q1 only)** — 12 fields:
//! `format`, `evidence_id`, `kind`, `validation_mode`,
//! `submission_id`, `subject_checksum`, `validator_selection_checksum`,
//! `class_code`, `task_handle`, `succeeded`, `report`, `reviewed_evidence`.
//! Produced exclusively by `persist_validator_q1_evidence` for `kind = "q1"`.
//! The corresponding `reborn_validator_q1_runs` row must exist with
//! `succeeded = true`.
//!
//! ## Evidence producer contracts by stage kind
//!
//! | Kind | Format | Producer | `reborn_validator_q1_runs` row |
//! |------|--------|----------|-------------------------------|
//! | `q1` | Authored (12-field) | `persist_validator_q1_evidence` | Required, `succeeded = true` |
//! | `behavior` | Legacy (9-field, `association_checksum`) | Trusted pure/workflow-Q2 writer (B1-06 remaining work) | None written |
//! | `human_q2` | Legacy (9-field, `association_checksum`) | Trusted pure/workflow-Q2 writer (B1-06 remaining work) | None written |
//!
//! ## Authored path status — **closed until producers are qualified**
//!
//! The trusted behavioral and human-Q2 evidence writers for the pure
//! PythonCode review path are not yet implemented. `persist_behavioral_reviews`
//! produces evidence for selected Tool usages in a Recipe execution and is not
//! qualified for standalone class-22 PythonCode review. No qualified authored
//! pure-behavior or Q2 producer exists that emits evidence bound to a
//! `reborn_component_review_submissions` subject. `record_pure_component_approval`
//! therefore rejects all `authored` mode declarations with an explicit error.
//! This guard is removed when the trusted writers are implemented and their
//! evidence binding is verifiable end-to-end. Implementing those writers is
//! remaining B1-06 work; B3-09 owns their presentation and client binding.
//!
//! The Q1 subject-binding infrastructure (join of `reborn_validator_q1_runs`
//! with `reborn_component_review_submissions`, all five authored field checks,
//! and the component/version identity checks) is implemented in full and will
//! be activated when the authored path is re-opened.
//!
//! ## Subject binding design for `q1`-kind authored evidence (implemented, inactive)
//!
//! When the authored path is re-opened, the Q1 evidence is verified by joining
//! `reborn_validator_q1_runs` with `reborn_component_review_submissions`:
//!
//! - The run row matches all five authored fields
//!   (`submission_id`, `validator_selection_checksum`, `class_code`,
//!   `task_handle`, `succeeded = true`).
//! - The submission's `checksum` equals the evidence `subject_checksum`.
//! - The submission's `component_id` and `candidate_version` equal the
//!   declaration's `root.uuid` / `root.version`.
//!
//! This prevents Q1 evidence for subject A from authorising subject B.
//!
//! **Source-only vs full Q1:** `reborn_validator_q1_runs` rows are written by
//! both `persist_validator_q1_evidence` (full validator run) and
//! `persist_validator_source_evidence` (source-only prerequisite). Both set
//! `succeeded = true`; `succeeded = true` alone is therefore not sufficient to
//! establish full Q1. The activation commit must require a **positive
//! qualification record** produced and owned by the Validator owner — not a
//! negative scope check. The validator selection stored in
//! `reborn_component_validator_selections` carries either scope
//! `"preparation-only"` (format `/1`) or `"qualified-pure-source-prerequisite"`
//! (format `/2`); neither scope value represents full Q1. Full Q1 requires a
//! separate, trusted qualification decision from the Validator owner, expressed
//! as an evidence record whose kind, producer and subject binding have been
//! reviewed and accepted by that owner before this writer may rely on it.

use brassclaw_pg::PgPool;
use brassclaw_skills::{
    association_contract::ValidationMode,
    component_revision::REVISION_LIMITS,
    pure_component_contract::{PureComponentApprovalDeclaration, PureComponentContractError},
    revision_store::{PgComponentRevisionStore, RevisionStoreError},
    value_contract::{ContractLimits, strict_json},
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum PureApprovalError {
    #[error("pure component approval: {0}")]
    Invalid(&'static str),
    #[error("pure component approval database operation failed")]
    Database(#[source] tokio_postgres::Error),
    #[error("pure component approval database connection failed")]
    Connection(#[source] deadpool_postgres::PoolError),
    #[error("pure component approval selected revision read failed")]
    Revisions(#[from] RevisionStoreError),
}
fn invalid(reason: &'static str) -> PureApprovalError {
    PureApprovalError::Invalid(reason)
}
fn db(error: tokio_postgres::Error) -> PureApprovalError {
    PureApprovalError::Database(error)
}

/// Which evidence format was detected for a given record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvidenceFormat {
    /// Legacy bootstrap format with `association_checksum` field.
    LegacyBootstrap,
    /// Authored validator format with `submission_id` / `task_handle` fields.
    AuthoredValidator,
}

/// Read, verify and store one pure-component approval declaration.
///
/// Returns the stored root UUID on success. This identity alone must never
/// authorize catalogue preparation or independent execution.
///
/// # Invariants
///
/// - All evidence UUIDs must exist in `reborn_component_review_evidence` with
///   `succeeded=true` in the stored JSON.
/// - All component UUIDs/versions/checksums must match `reborn_component_revisions`.
/// - The component class_code for every closure member must be 22 (PythonCode).
/// - The root must be present in the closure.
/// - An existing stored approval with the same `approval_id` must be byte-identical.
/// - Evidence checksums, IDs, distinct stage kinds, modes and exact component
///   coverage agree; human Q2 covers the exact selected Q1/behavior set.
/// - For `authored` mode: **not yet accepted**. The qualified behavioral and
///   human-Q2 producers for the authored pure path are not connected; the
///   writer rejects all authored declarations with an explicit error.
///   This guard is removed when both producers are qualified and their
///   evidence binding is verifiable end-to-end.
/// - These mechanical checks do not authenticate a behavioral execution or
///   human interaction by themselves. Only the internal owning producer writes;
///   public declarations cannot create approval or select provenance.
pub(crate) async fn record_pure_component_approval(
    pool: &PgPool,
    declaration: &PureComponentApprovalDeclaration,
) -> Result<Uuid, PureApprovalError> {
    let bytes = declaration.exact_bytes();
    let checksum = format!("{:x}", Sha256::digest(bytes.as_bytes()));
    let root = declaration.root();
    let components = declaration.components();
    let mode = declaration.mode();
    let q1 = declaration.q1_reference();
    let q2 = declaration.q2_reference();
    let behaviors = declaration.behavioral_references();

    // The authored pure-component path requires behavioral and human-Q2 evidence
    // from their qualified producers. Those producers are not yet connected:
    //
    //   - behavioral evidence: persist_behavioral_reviews produces evidence for
    //     selected Tool usages in a Recipe execution, not for standalone
    //     pure PythonCode review. No qualified authored pure-behavior producer
    //     exists that emits evidence bound to a reborn_component_review_submissions
    //     subject for class-22 PythonCode.
    //   - human Q2: no qualified authored Q2 producer is connected.
    //
    // Accepting unverified legacy-format records for these stages in authored
    // mode would create a path that appears complete without actual qualification.
    // The authored path is closed here until the qualified producers are connected
    // and their evidence binding is verifiable end-to-end.
    //
    // system_seed approvals are unaffected: their bootstrap producers are
    // qualified via the controlled seed/integrity path.
    if mode == ValidationMode::Authored {
        return Err(invalid(
            "authored pure-component approvals are not yet accepted: \
             qualified behavioral and human-Q2 producers are not connected",
        ));
    }

    // Stage identity is not supplied by a positive flag. Each reference has
    // exactly one role and must cover the same exact immutable subject graph.
    let mut evidence_kinds = BTreeMap::from([(q1, "q1")]);
    if let Some(q2_id) = q2 {
        evidence_kinds.insert(q2_id, "human_q2");
    }
    for id in behaviors {
        evidence_kinds.insert(*id, "behavior");
    }

    // This write is also used by closed installation publication under the
    // acknowledged store deadline. A cancelled/unknown COMMIT connection must
    // not return to a size-one pool before original readback.
    let mut client = crate::bootstrap_evidence::io::EvidenceClient::from_client(
        pool.get().await.map_err(PureApprovalError::Connection)?,
    );
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(db)?;

    let ids: Vec<_> = evidence_kinds.keys().copied().collect();
    let evidence_rows = tx
        .query(
            "SELECT evidence_id,evidence_bytes,checksum FROM reborn_component_review_evidence
         WHERE evidence_id=ANY($1::uuid[])",
            &[&ids],
        )
        .await
        .map_err(db)?;
    if evidence_rows.len() != evidence_kinds.len() {
        return Err(invalid("referenced evidence record is missing"));
    }
    // Parse every evidence record.
    //
    // Q1 evidence for authored approvals uses the authored (12-field) format
    // produced by persist_validator_q1_evidence; collect its fields for the
    // subject-binding check below.
    //
    // Behavioral and human-Q2 evidence use the legacy (9-field) format produced
    // by persist_behavioral_reviews and their respective unqualified producers.
    // They carry association_checksum + components, not submission_id / task_handle,
    // and do NOT have reborn_validator_q1_runs rows. Their format is verified
    // by validate_evidence_declaration / validate_legacy_evidence_components;
    // cross-stage subject binding for them is deferred to their qualified producers.
    struct Q1SubjectFields {
        submission_id: Uuid,
        subject_checksum: String,
        validator_selection_checksum: String,
        class_code: i64,
        task_handle: Uuid,
    }
    let mut q1_authored_fields: Option<Q1SubjectFields> = None;

    for row in &evidence_rows {
        let id: Uuid = row.get(0);
        let evidence_bytes: &str = row.get(1);
        if format!("{:x}", Sha256::digest(evidence_bytes.as_bytes())) != row.get::<_, &str>(2) {
            return Err(invalid("referenced evidence checksum differs"));
        }
        let value = strict_json(evidence_bytes, REVISION_LIMITS)
            .map_err(|_| invalid("evidence bytes violate the strict data contract"))?;
        let expected_kind = evidence_kinds
            .get(&id)
            .ok_or_else(|| invalid("unexpected evidence identity"))?;
        let evidence_format =
            validate_evidence_declaration(&value, id, expected_kind, declaration)?;

        // Extract the Q1 authored fields for the subject-binding check that
        // follows. Only q1-kind authored evidence is routed here; behavior and
        // human_q2 evidence use the legacy format and have no run row.
        if evidence_format == EvidenceFormat::AuthoredValidator && *expected_kind == "q1" {
            let sub_id = value["submission_id"]
                .as_str()
                .ok_or_else(|| invalid("authored Q1 evidence submission_id missing"))?;
            q1_authored_fields = Some(Q1SubjectFields {
                submission_id: Uuid::parse_str(sub_id)
                    .map_err(|_| invalid("authored Q1 evidence submission_id invalid UUID"))?,
                subject_checksum: value["subject_checksum"]
                    .as_str()
                    .ok_or_else(|| invalid("authored Q1 evidence subject_checksum missing"))?
                    .to_owned(),
                validator_selection_checksum: value["validator_selection_checksum"]
                    .as_str()
                    .ok_or_else(|| {
                        invalid("authored Q1 evidence validator_selection_checksum missing")
                    })?
                    .to_owned(),
                class_code: value["class_code"]
                    .as_i64()
                    .ok_or_else(|| invalid("authored Q1 evidence class_code missing"))?,
                task_handle: {
                    let raw = value["task_handle"]
                        .as_str()
                        .ok_or_else(|| invalid("authored Q1 evidence task_handle missing"))?;
                    Uuid::parse_str(raw)
                        .map_err(|_| invalid("authored Q1 evidence task_handle invalid UUID"))?
                },
            });
        }
    }

    // Q1 subject binding for authored approvals.
    //
    // persist_validator_q1_evidence is the only trusted writer that produces
    // both a reborn_component_review_evidence row (kind = "q1", authored format)
    // and a matching reborn_validator_q1_runs index row. We join them here and
    // verify all five authored fields plus the submission's candidate identity:
    //
    //   run.submission_id               == evidence.submission_id
    //   run.validator_selection_checksum == evidence.validator_selection_checksum
    //   run.candidate_class_code        == evidence.class_code
    //   run.task_handle                 == evidence.task_handle
    //   run.succeeded                   == true
    //   submission.checksum             == evidence.subject_checksum
    //   submission.component_id         == declaration.root.uuid
    //   submission.candidate_version    == declaration.root.version
    //
    // system_seed approvals use bootstrap evidence (no run row); skip entirely.
    // Behavioral and human-Q2 stages use the legacy format and have no validator
    // run row; their subject binding is deferred to their qualified producers
    // (open points per module-level doc).
    if mode == ValidationMode::Authored {
        let fields = q1_authored_fields.ok_or_else(|| {
            invalid("authored approval Q1 evidence must use the authored (12-field) format")
        })?;
        let root_version =
            i64::try_from(root.version).map_err(|_| invalid("root version exceeds i64 range"))?;
        let row = tx
            .query_opt(
                "SELECT r.submission_id,
                        r.validator_selection_checksum,
                        r.candidate_class_code,
                        r.task_handle,
                        r.succeeded,
                        s.checksum,
                        s.component_id,
                        s.candidate_version
                 FROM reborn_validator_q1_runs r
                 JOIN reborn_component_review_submissions s
                   ON s.submission_id = r.submission_id
                 WHERE r.evidence_id = $1",
                &[&q1],
            )
            .await
            .map_err(db)?
            .ok_or_else(|| invalid("authored Q1 evidence has no matching validator run record"))?;

        let run_submission: Uuid = row.get(0);
        let run_validator_checksum: &str = row.get(1);
        let run_class_code: i16 = row.get(2);
        let run_task_handle: Uuid = row.get(3);
        let run_succeeded: bool = row.get(4);
        let sub_checksum: &str = row.get(5);
        let sub_component: Uuid = row.get(6);
        let sub_version: i64 = row.get(7);

        // All five authored Q1 fields must match the trusted run row exactly.
        if run_submission != fields.submission_id {
            return Err(invalid(
                "authored Q1 submission_id does not match the validator run record",
            ));
        }
        if run_validator_checksum != fields.validator_selection_checksum {
            return Err(invalid(
                "authored Q1 validator_selection_checksum does not match the run record",
            ));
        }
        if i64::from(run_class_code) != fields.class_code {
            return Err(invalid(
                "authored Q1 class_code does not match the validator run record",
            ));
        }
        if run_task_handle != fields.task_handle {
            return Err(invalid(
                "authored Q1 task_handle does not match the validator run record",
            ));
        }
        if !run_succeeded {
            return Err(invalid("authored Q1 validator run did not succeed"));
        }

        // The submission's checksum must equal the evidence subject_checksum.
        if sub_checksum != fields.subject_checksum {
            return Err(invalid(
                "authored Q1 subject_checksum does not match the review submission",
            ));
        }

        // The submission's candidate must match the declaration root exactly.
        if sub_component != root.uuid {
            return Err(invalid(
                "authored Q1 submission is for a different component than the approval root",
            ));
        }
        if sub_version != root_version {
            return Err(invalid(
                "authored Q1 submission is for a different version than the approval root",
            ));
        }
    }

    let selected: Vec<_> = components.values().copied().collect();
    let snapshot = PgComponentRevisionStore::read_exact_in_transaction(
        &tx,
        &[root.uuid],
        &selected,
        brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
    )
    .await?;
    declaration
        .require_selected_snapshot(&snapshot)
        .map_err(|_| invalid("component closure differs from the complete immutable graph"))?;

    // The revision owner above validates exact bytes and the complete graph;
    // keep only checksum strings needed by the immutable closure index.
    let checksum_strings: Vec<_> = components
        .values()
        .map(|reference| hex::encode(reference.checksum))
        .collect();

    // 3. Verify the lengths satisfy the contract limits.
    if bytes.len() > REVISION_LIMITS.max_bytes {
        return Err(invalid("approval bytes exceed technical capacity"));
    }

    let root_hex: String = root.checksum.iter().map(|b| format!("{b:02x}")).collect();
    let mode_str = match mode {
        brassclaw_skills::association_contract::ValidationMode::Authored => "authored",
        brassclaw_skills::association_contract::ValidationMode::SystemSeed => "system_seed",
    };

    // 4. Insert the approval record (idempotent on conflict).
    tx.execute(
        "INSERT INTO reborn_pure_component_approvals
         (approval_id, root_id, root_version, root_checksum, validation_mode,
          q1_ref, q2_ref, approval_bytes, checksum)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (approval_id) DO NOTHING",
        &[
            &declaration.approval_id(),
            &root.uuid,
            &i64::try_from(root.version).map_err(|_| invalid("root version exceeds i64 range"))?,
            &root_hex,
            &mode_str,
            &q1,
            &q2,
            &bytes,
            &checksum,
        ],
    )
    .await
    .map_err(db)?;

    // Verify idempotency: stored bytes must be byte-identical to what we computed.
    let stored = tx
        .query_one(
            "SELECT approval_bytes, checksum FROM reborn_pure_component_approvals WHERE approval_id=$1",
            &[&declaration.approval_id()],
        )
        .await
        .map_err(db)?;
    if stored.get::<_, &str>(0) != bytes || stored.get::<_, &str>(1) != checksum {
        return Err(invalid(
            "stored pure-component approval has conflicting bytes",
        ));
    }

    // 5. Insert closure rows (idempotent on conflict).
    for (reference, hex_checksum) in components.values().zip(checksum_strings.iter()) {
        let version = i64::try_from(reference.version)
            .map_err(|_| invalid("component version exceeds i64 range"))?;
        tx.execute(
            "INSERT INTO reborn_pure_component_approval_closure
             (approval_id, component_id, component_version, component_checksum)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (approval_id, component_id) DO NOTHING",
            &[
                &declaration.approval_id(),
                &reference.uuid,
                &version,
                &hex_checksum,
            ],
        )
        .await
        .map_err(db)?;
    }

    let stored_closure = tx
        .query(
            "SELECT component_id,component_version,component_checksum
         FROM reborn_pure_component_approval_closure WHERE approval_id=$1",
            &[&declaration.approval_id()],
        )
        .await
        .map_err(db)?;
    if stored_closure.len() != components.len() {
        return Err(invalid("stored approval closure membership differs"));
    }
    for row in stored_closure {
        let id: Uuid = row.get(0);
        let reference = components
            .get(&id)
            .ok_or_else(|| invalid("stored approval closure has a foreign component"))?;
        if row.get::<_, i64>(1) as u64 != reference.version
            || row.get::<_, &str>(2) != hex::encode(reference.checksum)
        {
            return Err(invalid("stored approval closure revision differs"));
        }
    }

    tx.commit().await.map_err(db)?;
    client.recycle_after_commit();
    Ok(root.uuid)
}

/// Read an existing approved pure-component root UUID by approval ID.
/// Returns `None` if not found. Does not re-verify evidence; that happens at
/// `record_pure_component_approval` time. The caller is responsible for
/// matching the exact approval ID against the expected declaration.
pub async fn read_pure_component_approval_root(
    pool: &PgPool,
    approval_id: Uuid,
) -> Result<Option<Uuid>, PureApprovalError> {
    let client = pool.get().await.map_err(PureApprovalError::Connection)?;
    let row = client
        .query_opt(
            "SELECT root_id FROM reborn_pure_component_approvals WHERE approval_id=$1",
            &[&approval_id],
        )
        .await
        .map_err(db)?;
    Ok(row.map(|r| r.get(0)))
}

/// Resolve an exact already recorded declaration. This compatibility operation
/// performs no INSERT, evidence production, Q2 or provenance selection.
pub async fn resolve_existing_pure_component_approval(
    pool: &PgPool,
    json_bytes: &str,
) -> Result<Uuid, PureApprovalError> {
    let limits = ContractLimits {
        max_depth: REVISION_LIMITS.max_depth,
        max_nodes: REVISION_LIMITS.max_nodes,
        max_bytes: REVISION_LIMITS.max_bytes,
    };
    let declaration = PureComponentApprovalDeclaration::from_json(json_bytes, limits).map_err(
        |error| match error {
            PureComponentContractError::Invalid(reason) => invalid(reason),
            PureComponentContractError::Data(_) => {
                invalid("approval bytes exceed technical data contract limits")
            }
        },
    )?;
    let client = pool.get().await.map_err(PureApprovalError::Connection)?;
    let row = client.query_opt(
        "SELECT approval_bytes,checksum,root_id FROM reborn_pure_component_approvals WHERE approval_id=$1",
        &[&declaration.approval_id()],
    ).await.map_err(db)?.ok_or_else(|| invalid("approval requires an existing trusted review decision"))?;
    let stored: &str = row.get(0);
    if stored != declaration.exact_bytes()
        || format!("{:x}", Sha256::digest(stored.as_bytes())) != row.get::<_, &str>(1)
        || row.get::<_, Uuid>(2) != declaration.root().uuid
    {
        return Err(invalid(
            "stored approval differs from the requested original",
        ));
    }
    Ok(declaration.root().uuid)
}

/// Detect the evidence format variant from the stored JSON object.
///
/// Returns `LegacyBootstrap` for the 9-field bootstrap format with
/// `association_checksum`, or `AuthoredValidator` for the 12-field validator
/// format with `submission_id` / `task_handle`. All other shapes are rejected.
fn detect_evidence_format(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<EvidenceFormat, PureApprovalError> {
    // Distinguish by the presence of the mutually exclusive context field.
    // The two formats cannot overlap: exactly one of these sets of fields is present.
    let has_legacy_field = object.contains_key("association_checksum");
    let has_authored_fields = object.contains_key("submission_id")
        && object.contains_key("subject_checksum")
        && object.contains_key("validator_selection_checksum")
        && object.contains_key("class_code")
        && object.contains_key("task_handle");

    match (has_legacy_field, has_authored_fields) {
        (true, false) => {
            if object.len() != LEGACY_EVIDENCE_FIELDS.len()
                || LEGACY_EVIDENCE_FIELDS
                    .iter()
                    .any(|f| !object.contains_key(*f))
            {
                return Err(invalid(
                    "legacy evidence fields differ from the declared format",
                ));
            }
            Ok(EvidenceFormat::LegacyBootstrap)
        }
        (false, true) => {
            if object.len() != AUTHORED_EVIDENCE_FIELDS.len()
                || AUTHORED_EVIDENCE_FIELDS
                    .iter()
                    .any(|f| !object.contains_key(*f))
            {
                return Err(invalid(
                    "authored evidence fields differ from the declared format",
                ));
            }
            Ok(EvidenceFormat::AuthoredValidator)
        }
        _ => Err(invalid(
            "evidence format is unrecognised; must be legacy bootstrap or authored validator",
        )),
    }
}

/// Required fields for the legacy bootstrap (S-path) evidence format.
const LEGACY_EVIDENCE_FIELDS: &[&str] = &[
    "format",
    "evidence_id",
    "kind",
    "validation_mode",
    "association_checksum",
    "components",
    "succeeded",
    "report",
    "reviewed_evidence",
];

/// Required fields for the authored validator (A-path) evidence format.
const AUTHORED_EVIDENCE_FIELDS: &[&str] = &[
    "format",
    "evidence_id",
    "kind",
    "validation_mode",
    "submission_id",
    "subject_checksum",
    "validator_selection_checksum",
    "class_code",
    "task_handle",
    "succeeded",
    "report",
    "reviewed_evidence",
];

/// Shape and exact-stage checks are necessary, not proof that a producer ran
/// behavior or received human input. Only the internal review/installation owner
/// may write approval; the public legacy declaration route resolves originals.
///
/// Returns the detected evidence format so the caller can determine whether
/// the Q1 subject-binding check applies.
///
/// # Format routing by kind
///
/// - `q1`: accepts both authored (12-field) and legacy (9-field) formats;
///   authored format is required for authored-mode approvals (enforced after
///   the loop by checking `q1_authored_fields.is_some()`).
/// - `behavior`: **legacy format only** — `persist_behavioral_reviews` writes
///   `association_checksum` + `components`, not `submission_id` / `task_handle`.
/// - `human_q2`: **legacy format only** — the Q2 producer is currently
///   unqualified and writes the legacy format when it exists.
fn validate_evidence_declaration(
    value: &serde_json::Value,
    id: Uuid,
    kind: &str,
    declaration: &PureComponentApprovalDeclaration,
) -> Result<EvidenceFormat, PureApprovalError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("evidence must be an object"))?;

    let evidence_format = detect_evidence_format(object)?;

    // behavior and human_q2 evidence must always use the legacy format.
    // Their producers write association_checksum + components. Rejecting the
    // authored format here prevents an authored-format record from being
    // accepted as behavioral or human-Q2 evidence.
    if (kind == "behavior" || kind == "human_q2")
        && evidence_format == EvidenceFormat::AuthoredValidator
    {
        return Err(invalid(
            "behavior and human_q2 evidence must use the legacy bootstrap format",
        ));
    }

    // Validate the association_checksum for legacy format.
    if evidence_format == EvidenceFormat::LegacyBootstrap
        && !value["association_checksum"].as_str().is_some_and(|text| {
            text.len() == 64
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    {
        return Err(invalid("evidence context checksum invalid"));
    }

    // Validate submission_id for the authored format: must be a canonical non-nil UUID.
    if evidence_format == EvidenceFormat::AuthoredValidator {
        let sub = value["submission_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| invalid("authored evidence submission_id invalid"))?;
        if sub.is_nil() || sub.to_string() != value["submission_id"].as_str().unwrap_or("") {
            return Err(invalid(
                "authored evidence submission_id must be a canonical non-nil UUID",
            ));
        }
        // subject_checksum must be a 64-char lowercase hex string.
        if !value["subject_checksum"].as_str().is_some_and(|text| {
            text.len() == 64
                && text
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) {
            return Err(invalid("authored evidence subject_checksum invalid"));
        }
        // validator_selection_checksum must be a 64-char lowercase hex string.
        if !value["validator_selection_checksum"]
            .as_str()
            .is_some_and(|text| {
                text.len() == 64
                    && text
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        {
            return Err(invalid(
                "authored evidence validator_selection_checksum invalid",
            ));
        }
        // class_code must be a non-negative integer.
        if value["class_code"].as_i64().is_none() {
            return Err(invalid("authored evidence class_code must be an integer"));
        }
        // task_handle must be a canonical non-nil UUID.
        let task_handle = value["task_handle"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| invalid("authored evidence task_handle invalid"))?;
        if task_handle.is_nil()
            || task_handle.to_string() != value["task_handle"].as_str().unwrap_or("")
        {
            return Err(invalid(
                "authored evidence task_handle must be a canonical non-nil UUID",
            ));
        }
    }

    let mode = match declaration.mode() {
        ValidationMode::Authored => "authored",
        ValidationMode::SystemSeed => "system_seed",
    };
    if value.get("format").and_then(|v| v.as_str()) != Some("component-review-evidence/1")
        || value.get("evidence_id").and_then(|v| v.as_str()) != Some(id.to_string().as_str())
        || value.get("kind").and_then(|v| v.as_str()) != Some(kind)
        || value.get("validation_mode").and_then(|v| v.as_str()) != Some(mode)
        || value.get("succeeded").and_then(|v| v.as_bool()) != Some(true)
        || !value
            .get("report")
            .is_some_and(serde_json::Value::is_object)
    {
        return Err(invalid("evidence identity, stage, mode or result differs"));
    }

    // For the legacy format, verify the component coverage list.
    // For the authored format, components are not embedded in the evidence; the
    // approval closure already carries the exact component set and
    // require_selected_snapshot verifies it.
    if evidence_format == EvidenceFormat::LegacyBootstrap {
        validate_legacy_evidence_components(value, declaration)?;
    }

    let reviewed = value
        .get("reviewed_evidence")
        .and_then(|v| v.as_array())
        .ok_or_else(|| invalid("evidence reviewed-stage set missing"))?;
    let expected: BTreeSet<_> = if kind == "human_q2" {
        std::iter::once(declaration.q1_reference())
            .chain(declaration.behavioral_references().iter().copied())
            .collect()
    } else {
        BTreeSet::new()
    };
    let mut actual = BTreeSet::new();
    for item in reviewed {
        let text = item
            .as_str()
            .ok_or_else(|| invalid("reviewed evidence identity invalid"))?;
        let parsed =
            Uuid::parse_str(text).map_err(|_| invalid("reviewed evidence identity invalid"))?;
        if parsed.is_nil() || parsed.to_string() != text || !actual.insert(parsed) {
            return Err(invalid(
                "reviewed evidence identity is duplicate or invalid",
            ));
        }
    }
    if actual != expected {
        return Err(invalid(
            "human review covers a different evidence selection",
        ));
    }
    Ok(evidence_format)
}

/// Validate the `components` array in legacy bootstrap evidence.
/// Not required for the authored format (components are in the approval closure).
fn validate_legacy_evidence_components(
    value: &serde_json::Value,
    declaration: &PureComponentApprovalDeclaration,
) -> Result<(), PureApprovalError> {
    let rows = value
        .get("components")
        .and_then(|v| v.as_array())
        .ok_or_else(|| invalid("evidence exact component coverage missing"))?;
    let mut seen = BTreeSet::new();
    for row in rows {
        let fields = row
            .as_object()
            .ok_or_else(|| invalid("evidence component must be an object"))?;
        let id = fields
            .get("uuid")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| invalid("evidence component identity invalid"))?;
        let reference = declaration
            .components()
            .get(&id)
            .ok_or_else(|| invalid("evidence covers a different component"))?;
        if fields.len() != 4
            || !seen.insert(id)
            || fields.get("uuid").and_then(|v| v.as_str())
                != Some(reference.uuid.to_string().as_str())
            || fields.get("class_code").and_then(|v| v.as_i64())
                != Some(i64::from(reference.class_code))
            || fields.get("version").and_then(|v| v.as_u64()) != Some(reference.version)
            || fields.get("checksum").and_then(|v| v.as_str())
                != Some(hex::encode(reference.checksum).as_str())
        {
            return Err(invalid("evidence exact revision coverage differs"));
        }
    }
    if seen.len() != declaration.components().len() {
        return Err(invalid("evidence dependency coverage is incomplete"));
    }
    Ok(())
}

/// Record an immutable link between a review submission and a pure-component
/// approval, using the new `reborn_pure_evidence_submission_links` table (V201).
///
/// This is the write path for pure-component-review cutover contract item 1:
/// it ties the original `submission_id` (from `reborn_component_review_submissions`)
/// to the `approval_id` (from `reborn_pure_component_approvals`) and the exact
/// `component_id`/`candidate_version` pair from the submission. Doing so lets
/// the trusted review owner verify that the pure approval was derived from the
/// correct exact review subject without reparsing `subject_bytes`.
///
/// Idempotent: a repeated call with the same `link_id` succeeds if the stored
/// row is byte-identical. Conflicting bytes (same `link_id`, different content)
/// are rejected.
///
/// # Invariants
///
/// - All three IDs are non-nil UUIDs.
/// - `submission_id` must exist in `reborn_component_review_submissions`.
/// - `approval_id` must exist in `reborn_pure_component_approvals`.
/// - `component_id`/`candidate_version` must exist in `reborn_component_revisions`
///   and must match the submission's own `component_id`/`candidate_version`.
/// - This function does not verify evidence or grant any approval; those are
///   separate gates in the trusted review owner.
pub async fn record_pure_evidence_submission_link(
    pool: &PgPool,
    link_id: Uuid,
    submission_id: Uuid,
    approval_id: Uuid,
    component_id: Uuid,
    candidate_version: u64,
) -> Result<(), PureApprovalError> {
    if link_id.is_nil() || submission_id.is_nil() || approval_id.is_nil() || component_id.is_nil() {
        return Err(invalid(
            "link, submission, approval and component IDs must be non-nil",
        ));
    }
    let version = i64::try_from(candidate_version)
        .map_err(|_| invalid("candidate version exceeds i64 range"))?;
    let mut client = pool.get().await.map_err(PureApprovalError::Connection)?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(db)?;

    // Verify the submission exists and the component_id/candidate_version match.
    let sub_row = tx
        .query_opt(
            "SELECT component_id, candidate_version
             FROM reborn_component_review_submissions
             WHERE submission_id = $1",
            &[&submission_id],
        )
        .await
        .map_err(db)?
        .ok_or_else(|| invalid("submission_id not found in review submissions"))?;
    let stored_component: Uuid = sub_row.get(0);
    let stored_version: i64 = sub_row.get(1);
    if stored_component != component_id || stored_version != version {
        return Err(invalid(
            "component_id/candidate_version do not match the submission's subject",
        ));
    }

    // Independent FKs do not bind the approval's root to the subject. Compare
    // the exact immutable identity and digest before retaining this link.
    let approval = tx
        .query_opt(
            "SELECT p.root_id,p.root_version,p.root_checksum,r.checksum
             FROM reborn_pure_component_approvals p
             JOIN reborn_component_revisions r ON r.component_id=$2 AND r.version=$3 AND r.class_code=22
             WHERE p.approval_id=$1",
            &[&approval_id, &component_id, &version],
        )
        .await
        .map_err(db)?
        .ok_or_else(|| invalid("approval_id not found in pure component approvals"))?;
    if approval.get::<_, Uuid>(0) != component_id
        || approval.get::<_, i64>(1) != version
        || approval.get::<_, &str>(2) != approval.get::<_, &str>(3)
    {
        return Err(invalid(
            "approval root differs from the submission candidate",
        ));
    }

    // Insert idempotently.
    tx.execute(
        "INSERT INTO reborn_pure_evidence_submission_links
         (link_id, submission_id, approval_id, component_id, candidate_version)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (link_id) DO NOTHING",
        &[
            &link_id,
            &submission_id,
            &approval_id,
            &component_id,
            &version,
        ],
    )
    .await
    .map_err(db)?;

    // Verify idempotency: if a row already exists it must have identical fields.
    let stored = tx
        .query_one(
            "SELECT submission_id, approval_id, component_id, candidate_version
             FROM reborn_pure_evidence_submission_links WHERE link_id = $1",
            &[&link_id],
        )
        .await
        .map_err(db)?;
    if stored.get::<_, Uuid>(0) != submission_id
        || stored.get::<_, Uuid>(1) != approval_id
        || stored.get::<_, Uuid>(2) != component_id
        || stored.get::<_, i64>(3) != version
    {
        return Err(invalid(
            "stored pure evidence submission link has conflicting fields",
        ));
    }

    tx.commit().await.map_err(db)
}

#[cfg(all(test, feature = "skills-db"))]
#[path = "pg_pure_component_approval_tests.rs"]
mod tests;
