//! Unified intent system — `__resolve_intent__` host function.
//!
//! Replaces the legacy intent-detection functions (`signals_tool_intent`,
//! `signals_execution_intent`, `score_skill`, `extract_explicit_skills`,
//! `format_docs`, `append_system_append`) with a single DB-backed
//! routing lookup.
//!
//! # Spec references
//!
//! - §3.12 — Intent system design
//! - §4 — `reborn_intent_inputs` table (V028)
//! - §6.1 — SEC-05 (score hard cap + needs_review), PERF-01 (B-tree index),
//!   PERF-02 (single query with CASE WHEN), PERF-03 (atomic score increment),
//!   PERF-04 (normalised schema)
//! - §7 Q10 (query classification), Q11 (disambiguation message), Q12
//!   ("try it with AI" Rust-side fallback), Q16 (pg_trgm), Q18 ("AI before User")
//!
//! # Query classification (Q10 — 4 classes)
//!
//! | Class | Rule |
//! |-------|------|
//! | 1 | Single word (no spaces, no terminal punctuation) |
//! | 2 | 2–4 words, no terminal `.`/`!`/`?` |
//! | 3 | ≥5 words OR ends with `.`, `!`, or `?` (includes the `?`-rule) |
//! | 4 | Keyword fallback — created by RetrievalEngine only, never by classifier |
//!
//! # Match order (rules a–c)
//!
//! | Query class | Try in order |
//! |-------------|-------------|
//! | 3 (sentence) | 3 → 2 → 1 |
//! | 2 (partial)  | 2 → 3 → 1 |
//! | 1 (word)     | 1 → 2 → 3 |
//! | 4 (fallback) | 1 → 2 → 3 |
//!
//! All classes are resolved in a **single SQL query** (PERF-02): one
//! `WHERE input_class = ANY(…) ORDER BY CASE … END, score DESC`.
//!
//! # Scoring (rules d–f, PERF-03)
//!
//! Increments use `UPDATE … SET score = LEAST(score + 1, 100) RETURNING score`
//! — atomic; no SELECT-then-UPDATE race.
//!
//! # Feature gate
//!
//! DB-path functions require the `skills-db` feature (same pool as skill loading).
//! The pure-Rust helpers (`classify_query`, `match_order`) are always available.

#[cfg(feature = "skills-db")]
use std::collections::HashMap;
use std::fmt;
#[cfg(feature = "skills-db")]
use std::sync::Mutex;
#[cfg(feature = "skills-db")]
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// In-process rate-limit bucket for score increments (SEC-05)
// ---------------------------------------------------------------------------

/// Per-scope increment state tracked in a process-local token bucket.
///
/// Bucket key is the exact 4-part scope tuple so
/// the limit applies to the whole scope, not just a single row (spec §6.1
/// SEC-05: "50 increments per scope per hour").
#[cfg(feature = "skills-db")]
struct IncrementBucket {
    /// Number of increments in the current hour window.
    count: u32,
    /// Start of the current hour window.
    window_start: Instant,
}

/// Global in-process token-bucket map (scope_key → bucket).
///
/// `Option` wrapper means `None` = uninitialised; `HashMap::new()` is created
/// on first use via `get_or_insert_with`. New scopes reclaim all expired
/// entries. Capacity exhaustion skips scoring; it does not reject a match.
#[cfg(feature = "skills-db")]
static SCORE_RATE_BUCKETS: Mutex<Option<HashMap<IntentScope, IncrementBucket>>> = Mutex::new(None);

#[cfg(feature = "skills-db")]
const MAX_SCORE_RATE_SCOPES: usize = 4096;

// ---------------------------------------------------------------------------
// Public types — always compiled
// ---------------------------------------------------------------------------

/// The input class assigned to a user query (spec §3.12 Q10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum InputClass {
    Word = 1,
    Partial = 2,
    Sentence = 3,
    /// Created by `RetrievalEngine` keyword fallback only — never by the
    /// query classifier.
    KeywordFallback = 4,
}

impl InputClass {
    /// Parse from a DB `smallint` value.
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Word),
            2 => Some(Self::Partial),
            3 => Some(Self::Sentence),
            4 => Some(Self::KeywordFallback),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

impl fmt::Display for InputClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Word => write!(f, "word"),
            Self::Partial => write!(f, "partial"),
            Self::Sentence => write!(f, "sentence"),
            Self::KeywordFallback => write!(f, "keyword_fallback"),
        }
    }
}

/// The 2-point spread within which multiple matches trigger disambiguation (Q11).
pub const DISAMBIGUATION_SPREAD: i32 = 2;

/// Maximum candidates surfaced in a disambiguation message.
pub const MAX_DISAMBIGUATION_CANDIDATES: usize = 3;

/// Score hard cap (SEC-05).
pub const SCORE_CAP: i32 = 100;

/// Rate limit: at most this many score increments per scope per hour (SEC-05).
#[cfg(feature = "skills-db")]
const SCORE_RATE_LIMIT_PER_HOUR: u32 = 50;

/// A single candidate match returned by the resolver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentCandidate {
    pub row_id: Uuid,
    pub component_id: Uuid,
    pub component_class_code: i32,
    pub input_class: i16,
    pub score: i32,
    /// Short human-readable label for disambiguation UX.
    pub class_label: String,
    /// Preserve distinct workflows of one Recipe in disambiguation. The row ID
    /// identifies the choice; a step link alone is not an immutable variant ID.
    #[serde(default)]
    pub step_link: Option<String>,
}

/// Result of an intent resolution call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IntentResolution {
    /// Unambiguous single match.  Caller should score+1 and return the component.
    Match {
        component_id: Uuid,
        component_class_code: i32,
        /// `step_link` formula for the matched Recipe variant (§0.6). `None`
        /// for legacy / non-variant intents — the caller uses the existing
        /// `fetch_component_by_id` path. Populated for class-21 Recipe variant
        /// intents so Phase E can run IBS `build_instruction(step_link, …)`
        /// synchronously inside `fetch_for_turn`. Phase D (V054) adds the column.
        step_link: Option<String>,
        /// The matched intent row's `input_text` — the literal expression for a
        /// Path 0 exact match, or the template expression (containing `%`) for a
        /// Path 1/2/3 template match (§0.17.1). Phase M.3 adds the field so the
        /// caller (`fetch_for_turn`) can run `extract_template_slots(template,
        /// user_text)` when `is_template` is true.
        input_text: String,
        /// Whether the matched row is a template (`input_text` contains `%`).
        /// When true, `input_text` is the template and the caller extracts slot
        /// values; when false, the match is an exact literal (Path 0).
        is_template: bool,
    },
    /// Multiple candidates within a 2-point score spread.  The WebUI must show
    /// a disambiguation message with clickable buttons (Q11).
    Disambiguation { candidates: Vec<IntentCandidate> },
    /// No match found in the intent table.  Orchestrator should emit a
    /// "reformulate" message (or silent fallback if "AI before User" is ON).
    NoMatch,
}

/// Source tag for learned intent inputs (SEC-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentSource {
    Seeded,
    LearnedUser,
    LearnedLlm,
    LearnedFallback,
}

impl IntentSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Seeded => "seeded",
            Self::LearnedUser => "learned_user",
            Self::LearnedLlm => "learned_llm",
            Self::LearnedFallback => "learned_fallback",
        }
    }

    /// `learned_llm` inputs are flagged `needs_review = true` (SEC-05).
    pub fn needs_review(self) -> bool {
        self == Self::LearnedLlm
    }
}

// ---------------------------------------------------------------------------
// Query classification — always compiled (no feature gate)
// ---------------------------------------------------------------------------

/// Classify a raw query string into an [`InputClass`] (spec §3.12 Q10).
///
/// Rules (in priority order):
/// 1. **Class 3** — ≥5 whitespace-separated tokens OR ends with `.`/`!`/`?`.
/// 2. **Class 2** — 2–4 tokens, no terminal punctuation.
/// 3. **Class 1** — exactly 0 or 1 tokens (empty/single-word treated as word).
///
/// Class 4 is **never** produced by this function; it is only assigned by
/// `RetrievalEngine` for keyword-fallback inputs.
pub fn classify_query(query: &str) -> InputClass {
    let trimmed = query.trim();
    // Terminal punctuation test (the `?`-rule: "why this fails?" → class 3).
    let has_terminal = trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?');

    if has_terminal {
        return InputClass::Sentence;
    }

    let word_count = trimmed.split_whitespace().count();
    match word_count {
        0 | 1 => InputClass::Word,
        2..=4 => InputClass::Partial,
        _ => InputClass::Sentence,
    }
}

/// Return the ordered list of `input_class` values to try, given a query's class.
///
/// PERF-02: used to build the `CASE WHEN` ordering in the single resolution query.
pub fn match_order(query_class: InputClass) -> [i16; 3] {
    use InputClass::*;
    match query_class {
        Sentence => [3, 2, 1],
        Partial => [2, 3, 1],
        Word | KeywordFallback => [1, 2, 3],
    }
}

/// Map a component `class_code` to a short human-readable label for disambiguation UX.
///
/// Authoritative table (spec §4):
///   0=tool, 1=skill_rusty, 2=skill_monty, 3=skill_llm, 4-9=extensions,
///   10=orchestrator, 11=reserved, 12=spec, 13=tool_skill, 14=plan,
///   15=summary, 16=action, 17=docu, 18=lesson, 19=issue, 20=note,
///   21=recipe, 22=python_code, 23=extension_catalogue, 50=scaffold.
pub fn class_label(class_code: i32) -> String {
    match class_code {
        0 => "tool",
        1 => "skill_rusty",
        2 => "skill_monty",
        3 => "skill_llm",
        4 => "extension_worker",
        5 => "extension_cron",
        6 => "extension_trigger",
        7 => "extension_webhook",
        8 => "extension_plan",
        9 => "extension_revision",
        10 => "orchestrator",
        11 => "reserved",
        12 => "spec",
        13 => "tool_skill",
        14 => "plan",
        15 => "summary",
        16 => "action",
        17 => "docu",
        18 => "lesson",
        19 => "issue",
        20 => "note",
        21 => "recipe",
        22 => "python_code",
        23 => "extension_catalogue",
        50 => "scaffold",
        _ => "component",
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Error type — always compiled
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum IntentSystemError {
    #[error("intent DB error: {0}")]
    Db(String),
    #[error("intent: invalid input class {0}")]
    InvalidClass(i16),
    #[error("intent choice does not identify a matching row in this scope")]
    InvalidChoice,
    #[error("intent selection requires repeatable-read or serializable isolation")]
    SnapshotIsolation,
    #[error("intent catalogue eligibility is inconsistent or exceeds technical capacity")]
    InvalidEligibility,
}

// ---------------------------------------------------------------------------
// DB functions — compiled only with `skills-db`
// ---------------------------------------------------------------------------

/// Scope for intent-system queries — must match the `reborn_skills` scope tuple.
#[cfg(feature = "skills-db")]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntentScope {
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub project_id: String,
}

/// Resolve an intent query against the `reborn_intent_inputs` table.
///
/// Uses a **single SQL query** (PERF-02) with `CASE WHEN` ordering so the
/// preferred class is tried first without multiple round-trips.
///
/// Returns [`IntentResolution`] which the orchestrator dispatches on.
#[cfg(feature = "skills-db")]
pub async fn resolve_intent(
    pool: &brassclaw_pg::PgPool,
    scope: &IntentScope,
    query: &str,
) -> Result<IntentResolution, IntentSystemError> {
    let client = pool
        .get()
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?;
    let (resolution, selected) = lookup_intent(&**client, scope, query, None).await?;
    if let Some(row_id) = selected {
        increment_score(&**client, scope, row_id).await?;
    }
    Ok(resolution)
}

/// Read matching from the caller's coherent catalogue view. This prepares only
/// selection: it does not commit, increment scores, approve components, dispatch
/// Tools or replace errors with No-Match. Mutable legacy intent rows do not yet
/// establish an activated generation; the caller must pin the exact Recipe,
/// embedded variant/layout, dependencies and review evidence in this same view.
#[cfg(feature = "skills-db")]
pub async fn resolve_intent_in_transaction(
    tx: &tokio_postgres::Transaction<'_>,
    scope: &IntentScope,
    query: &str,
) -> Result<IntentResolution, IntentSystemError> {
    let coherent: bool = tx
        .query_one(
            "SELECT current_setting('transaction_isolation') IN ('repeatable read','serializable')",
            &[],
        )
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?
        .get(0);
    if !coherent {
        return Err(IntentSystemError::SnapshotIsolation);
    }
    Ok(lookup_intent(tx, scope, query, None).await?.0)
}

/// Exact retained workflow membership supplied by the catalogue owner. This
/// is a routing filter, not approval evidence or an activation API. Production
/// builds it only from the already approved, active consistent generation.
/// Filtering happens before ranking, so a draft or stale intent cannot hide an
/// eligible match or turn it into disambiguation. No source is executed here.
#[cfg(feature = "skills-db")]
pub struct RetainedIntentEligibility {
    ids: Vec<Uuid>,
    links: Vec<String>,
    texts: Vec<String>,
}
#[cfg(feature = "skills-db")]
impl RetainedIntentEligibility {
    pub fn from_instructions(
        instructions: &[&super::retained_instruction::RetainedRecipeInstruction],
    ) -> Result<Self, IntentSystemError> {
        use std::collections::{BTreeMap, BTreeSet};
        let mut references = BTreeMap::new();
        let mut rows = BTreeSet::new();
        let mut bytes = 0usize;
        let mut visited = 0usize;
        if instructions.len() > 4096 {
            return Err(IntentSystemError::InvalidEligibility);
        }
        for instruction in instructions {
            for revision in instruction.snapshot().revisions().values() {
                visited += 1;
                if visited > 65536 {
                    return Err(IntentSystemError::InvalidEligibility);
                }
                let reference = revision.reference();
                if references
                    .insert(reference.uuid, reference)
                    .is_some_and(|old| old != reference)
                    || references.len() > 4096
                {
                    return Err(IntentSystemError::InvalidEligibility);
                }
            }
            let link = instruction
                .variant()
                .step_link
                .as_deref()
                .filter(|link| !link.is_empty())
                .ok_or(IntentSystemError::InvalidEligibility)?;
            for example in &instruction.variant().intent_examples {
                if example.is_empty() || example.chars().count() > 2048 || rows.len() >= 8192 {
                    return Err(IntentSystemError::InvalidEligibility);
                }
                bytes = bytes
                    .checked_add(link.len())
                    .and_then(|n| n.checked_add(example.len()))
                    .filter(|n| *n <= 64 * 1024 * 1024)
                    .ok_or(IntentSystemError::InvalidEligibility)?;
                rows.insert((instruction.recipe().uuid, link.to_owned(), example.clone()));
            }
        }
        let mut selected = Self {
            ids: Vec::new(),
            links: Vec::new(),
            texts: Vec::new(),
        };
        for (id, link, text) in rows {
            selected.ids.push(id);
            selected.links.push(link);
            selected.texts.push(text);
        }
        Ok(selected)
    }
}

/// The catalogue owner's eligible workflows restrict the existing actual
/// matcher in the same repeatable-read/serializable view used by IBS. An empty
/// eligible set is a real NoMatch, even if legacy draft rows exist. A read or
/// integrity failure remains an error, never NoMatch. This neutral consumer
/// does not determine eligibility from mutable status/source labels.
#[cfg(feature = "skills-db")]
pub async fn resolve_catalogue_intent_in_transaction(
    tx: &tokio_postgres::Transaction<'_>,
    scope: &IntentScope,
    query: &str,
    eligible: &RetainedIntentEligibility,
) -> Result<IntentResolution, IntentSystemError> {
    let coherent: bool = tx
        .query_one(
            "SELECT current_setting('transaction_isolation') IN ('repeatable read','serializable')",
            &[],
        )
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?
        .get(0);
    if !coherent {
        return Err(IntentSystemError::SnapshotIsolation);
    }
    Ok(lookup_intent(tx, scope, query, Some(eligible)).await?.0)
}

#[cfg(feature = "skills-db")]
async fn lookup_intent<C: tokio_postgres::GenericClient>(
    client: &C,
    scope: &IntentScope,
    query: &str,
    eligible: Option<&RetainedIntentEligibility>,
) -> Result<(IntentResolution, Option<Uuid>), IntentSystemError> {
    use tokio_postgres::types::ToSql;
    use tracing::debug;
    let query_class = classify_query(query);
    let order = match_order(query_class);
    let order_vec: Vec<i16> = order.to_vec();

    let eligible_ids = eligible.map(|e| &e.ids);
    let eligible_links = eligible.map(|e| &e.links);
    let eligible_texts = eligible.map(|e| &e.texts);
    let rows = client
        .query(
            // Limit distinct workflows only after ranking and spread filtering.
            // Limiting raw template rows can hide an equally eligible Recipe.
            "WITH matching AS (
               SELECT ii.*,
                 CASE WHEN ii.input_text = $5 THEN 0 ELSE 1 END AS text_rank,
                 CASE ii.input_class WHEN $7 THEN 0 WHEN $8 THEN 1
                   WHEN $9 THEN 2 ELSE 3 END AS class_rank
               FROM reborn_intent_inputs ii
               WHERE ii.tenant_id = $1 AND ii.user_id = $2
                 AND ii.agent_id = $3 AND ii.project_id = $4
                 AND ii.input_class = ANY($6)
                 AND ($12::uuid[] IS NULL OR (
                   ii.component_class_code=21 AND ii.needs_review=false
                   AND EXISTS (SELECT 1 FROM unnest($12::uuid[],$13::text[],$14::text[])
                     eligible(component_id,step_link,input_text)
                     WHERE eligible.component_id=ii.component_id
                       AND eligible.step_link=ii.step_link
                       AND eligible.input_text=ii.input_text)
                 ))
                 AND (
                   ii.input_text = $5
                   OR (ii.is_template = true AND ii.template_prefix != ''
                       AND left($5, length(ii.template_prefix)) = ii.template_prefix
                       AND $5 LIKE replace(replace(ii.input_text, '!', '!!'), '_', '!_') ESCAPE '!')
                   OR (ii.is_template = true AND ii.template_prefix = ''
                       AND ii.template_suffix != ''
                       AND right($5, length(ii.template_suffix)) = ii.template_suffix
                       AND $5 LIKE replace(replace(ii.input_text, '!', '!!'), '_', '!_') ESCAPE '!')
                 )
             ), ranked AS (
               SELECT matching.*, row_number() OVER (
                 PARTITION BY component_id, component_class_code, step_link
                 ORDER BY text_rank, class_rank, score DESC, id
               ) AS workflow_rank FROM matching
             ), distinct_workflows AS (
               SELECT ranked.*, first_value(score) OVER (
                 ORDER BY text_rank, class_rank, score DESC, id
               ) AS top_score FROM ranked WHERE workflow_rank = 1
             )
             SELECT id, component_id, component_class_code, input_class, score,
                    step_link, input_text, is_template
             FROM distinct_workflows
             WHERE top_score - score <= $10
             ORDER BY text_rank, class_rank, score DESC, id
             LIMIT $11",
            &[
                &scope.tenant_id as &(dyn ToSql + Sync),
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &query,
                &order_vec,
                &order[0],
                &order[1],
                &order[2],
                &DISAMBIGUATION_SPREAD,
                &(MAX_DISAMBIGUATION_CANDIDATES as i64),
                &eligible_ids,
                &eligible_links,
                &eligible_texts,
            ],
        )
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?;

    if rows.is_empty() {
        debug!(query = %query, class = %query_class, "intent: no match");
        return Ok((IntentResolution::NoMatch, None));
    }

    let candidates: Vec<IntentCandidate> = rows
        .iter()
        .map(|row| {
            let component_class_code = row.get(2);
            IntentCandidate {
                row_id: row.get(0),
                component_id: row.get(1),
                component_class_code,
                input_class: row.get(3),
                score: row.get(4),
                class_label: class_label(component_class_code),
                step_link: row.get(5),
            }
        })
        .collect();

    if candidates.len() == 1 {
        let c = &candidates[0];
        debug!(
            component_id = %c.component_id,
            score = c.score,
            "intent: unambiguous match"
        );
        // step_link (col 5) + input_text (col 6) + is_template (col 7) from the
        // top row (FIND-P10-01). `c` corresponds to rows[0] (highest score,
        // first dedup-inserted). Phase M.3 adds input_text/is_template so the
        // caller can run `extract_template_slots` on template matches (§0.17.1).
        return Ok((
            IntentResolution::Match {
                component_id: c.component_id,
                component_class_code: c.component_class_code,
                step_link: rows[0].get::<_, Option<String>>(5),
                input_text: rows[0].get::<_, String>(6),
                is_template: rows[0].get::<_, bool>(7),
            },
            Some(c.row_id),
        ));
    }

    // Multiple candidates within spread → disambiguation (Q11).
    debug!(
        count = candidates.len(),
        query = %query,
        "intent: disambiguation required"
    );
    Ok((IntentResolution::Disambiguation { candidates }, None))
}

/// Record the user's disambiguation choice: atomically increment the chosen
/// row's score and return the winning component.
#[cfg(feature = "skills-db")]
pub async fn record_disambiguation_choice(
    pool: &brassclaw_pg::PgPool,
    scope: &IntentScope,
    row_id: Uuid,
    component_id: Uuid,
    component_class_code: i32,
) -> Result<IntentResolution, IntentSystemError> {
    use tracing::debug;
    let mut client = pool
        .get()
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?;
    let tx = client
        .transaction()
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?;
    // Hold the selected row against reseeding/deletion through the score write.
    // Caller-provided IDs never substitute for the actual selected workflow.
    let row = tx
        .query_opt(
            "SELECT component_id, component_class_code, step_link, input_text, is_template
         FROM reborn_intent_inputs
         WHERE id = $1 AND tenant_id = $2 AND user_id = $3
           AND agent_id = $4 AND project_id = $5
           AND component_id = $6 AND component_class_code = $7
         FOR UPDATE",
            &[
                &row_id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &component_id,
                &component_class_code,
            ],
        )
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?
        .ok_or(IntentSystemError::InvalidChoice)?;
    increment_score(&*tx, scope, row_id).await?;
    let result = IntentResolution::Match {
        component_id: row.get(0),
        component_class_code: row.get(1),
        step_link: row.get(2),
        input_text: row.get(3),
        is_template: row.get(4),
    };
    tx.commit()
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?;
    debug!(row_id = %row_id, component_id = %component_id,
        "intent: disambiguation choice recorded");
    Ok(result)
}

/// Seed (or update) an intent input row, typically called on component validation
/// to populate `intent_examples` into `reborn_intent_inputs` (spec §1.5).
///
/// Uses `INSERT … ON CONFLICT DO UPDATE` so re-seeding a component is idempotent.
#[cfg(feature = "skills-db")]
// 8 params: the `step_link` arg is plan-mandated by FIND-NEW-03 (Phase D) and
// cannot be folded into a struct without deviating from the specified seeder
// signature and rewriting every call site.
#[allow(clippy::too_many_arguments)]
pub async fn seed_intent_input(
    pool: &brassclaw_pg::PgPool,
    scope: &IntentScope,
    input_text: &str,
    input_class: InputClass,
    component_id: Uuid,
    component_class_code: i32,
    source: IntentSource,
    // `step_link` formula for Recipe (class 21) variant intents; `None` for
    // non-Recipe inputs (FIND-NEW-03). Stored in
    // `reborn_intent_inputs.step_link` (V054) so `resolve_intent` can return
    // it for the IBS `build_instruction` path (Phase E). NOT part of the
    // conflict key; updated via `SET` on re-seed.
    step_link: Option<&str>,
) -> Result<(), IntentSystemError> {
    // Phase M (§0.17): detect `%` slot markers and populate the template anchor
    // columns (V076) so resolve_intent's three-path dispatch can index-match
    // template intents. Plain exact-match intents keep is_template = false +
    // NULL anchors and ride the existing Path 0.
    let (is_template, template_prefix, template_suffix) =
        match crate::memory::template_extractor::parse_template(input_text) {
            Some((prefix, suffix)) => (true, Some(prefix), Some(suffix)),
            None => (false, None, None),
        };

    let client = pool
        .get()
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?;
    client
        .execute(
            "INSERT INTO reborn_intent_inputs
                 (tenant_id, user_id, agent_id, project_id,
                  input_text, input_class, component_id, component_class_code,
                  score, source, needs_review, step_link,
                  is_template, template_prefix, template_suffix)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,1,$9,$10,$11,$12,$13,$14)
             ON CONFLICT (tenant_id, user_id, agent_id, project_id,
                          input_text, input_class, component_id)
             DO UPDATE SET
                 source          = EXCLUDED.source,
                 needs_review    = EXCLUDED.needs_review,
                 step_link       = EXCLUDED.step_link,
                 is_template     = EXCLUDED.is_template,
                 template_prefix = EXCLUDED.template_prefix,
                 template_suffix = EXCLUDED.template_suffix,
                 updated_at      = now()",
            &[
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &input_text,
                &input_class.as_i16(),
                &component_id,
                &component_class_code,
                &source.as_str(),
                &source.needs_review(),
                &step_link,
                &is_template,
                &template_prefix,
                &template_suffix,
            ],
        )
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?;

    Ok(())
}

/// Delete all intent inputs for a component (e.g. on component wipe/Q4).
#[cfg(feature = "skills-db")]
pub async fn purge_component_inputs(
    pool: &brassclaw_pg::PgPool,
    scope: &IntentScope,
    component_id: Uuid,
) -> Result<u64, IntentSystemError> {
    let client = pool
        .get()
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?;
    let result = client
        .execute(
            "DELETE FROM reborn_intent_inputs
             WHERE tenant_id   = $1
               AND user_id     = $2
               AND agent_id    = $3
               AND project_id  = $4
               AND component_id = $5",
            &[
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &component_id,
            ],
        )
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?;

    Ok(result)
}

// ---------------------------------------------------------------------------
// Internal helpers (skills-db only)
// ---------------------------------------------------------------------------

/// Atomically increment a row's score, capped at `SCORE_CAP` (PERF-03, SEC-05).
///
/// Rate-limited to `SCORE_RATE_LIMIT_PER_HOUR` increments **per scope** per
/// hour (spec §6.1 SEC-05 token-bucket, in-process).  The bucket key is the
/// exact 4-part tuple so that the limit applies across all rows within a
/// tenant/user/agent/project tuple, not just a single row.
///
/// Returns the updated score.  If the rate limit is exhausted for this
/// window, the DB update is skipped and the current score is returned.
#[cfg(feature = "skills-db")]
async fn increment_score<C: tokio_postgres::GenericClient>(
    client: &C,
    scope: &IntentScope,
    row_id: Uuid,
) -> Result<i32, IntentSystemError> {
    // This bounds only score telemetry, never task execution or token use.
    let allow = {
        let mut guard = SCORE_RATE_BUCKETS
            .lock()
            .map_err(|_| IntentSystemError::Db("rate-bucket lock poisoned".into()))?;
        let map = guard.get_or_insert_with(HashMap::new);
        let now = Instant::now();
        let window = Duration::from_secs(3600);
        if !map.contains_key(scope) {
            map.retain(|_, bucket| now.duration_since(bucket.window_start) < window);
        }
        if !map.contains_key(scope) && map.len() >= MAX_SCORE_RATE_SCOPES {
            false
        } else {
            let bucket = map.entry(scope.clone()).or_insert(IncrementBucket {
                count: 0,
                window_start: now,
            });
            if now.duration_since(bucket.window_start) >= window {
                bucket.count = 0;
                bucket.window_start = now;
            }
            if bucket.count < SCORE_RATE_LIMIT_PER_HOUR {
                bucket.count += 1;
                true
            } else {
                false
            }
        }
    };

    if !allow {
        // Rate limit exhausted: return the current score without a DB write.
        use tracing::debug;
        debug!(row_id = %row_id, "intent: score increment skipped (SEC-05 rate limit)");
        let row = client
            .query_one(
                "SELECT score FROM reborn_intent_inputs WHERE id = $1
                 AND tenant_id = $2 AND user_id = $3 AND agent_id = $4 AND project_id = $5",
                &[
                    &row_id,
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                ],
            )
            .await
            .map_err(|e| IntentSystemError::Db(e.to_string()))?;
        return Ok(row.get::<_, i32>(0));
    }

    let row = client
        .query_one(
            "UPDATE reborn_intent_inputs
             SET score      = LEAST(score + 1, $1),
                 updated_at = now()
             WHERE id = $2 AND tenant_id = $3 AND user_id = $4
               AND agent_id = $5 AND project_id = $6
             RETURNING score",
            &[
                &SCORE_CAP,
                &row_id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
            ],
        )
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?;

    Ok(row.get::<_, i32>(0))
}

// ---------------------------------------------------------------------------
// Unit tests — always compiled (pure logic, no DB)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Query classification ---

    #[test]
    fn classify_single_word() {
        assert_eq!(classify_query("github"), InputClass::Word);
    }

    #[test]
    fn classify_two_words_no_punct() {
        assert_eq!(classify_query("fetch issues"), InputClass::Partial);
    }

    #[test]
    fn classify_four_words_no_punct() {
        assert_eq!(
            classify_query("list open github issues"),
            InputClass::Partial
        );
    }

    #[test]
    fn classify_five_words_is_sentence() {
        assert_eq!(
            classify_query("list all open github issues"),
            InputClass::Sentence
        );
    }

    #[test]
    fn classify_three_words_question_mark() {
        // The ?-rule: a 3-word question is class 3.
        assert_eq!(classify_query("why this fails?"), InputClass::Sentence);
    }

    #[test]
    fn classify_terminal_period() {
        assert_eq!(classify_query("fetch the issues."), InputClass::Sentence);
    }

    #[test]
    fn classify_terminal_exclamation() {
        assert_eq!(classify_query("do it now!"), InputClass::Sentence);
    }

    #[test]
    fn classify_empty_query_is_word() {
        assert_eq!(classify_query(""), InputClass::Word);
    }

    #[test]
    fn classify_whitespace_only_is_word() {
        assert_eq!(classify_query("   "), InputClass::Word);
    }

    #[test]
    fn classify_exactly_four_words_is_partial() {
        assert_eq!(classify_query("one two three four"), InputClass::Partial);
    }

    // --- Match order ---

    #[test]
    fn match_order_sentence() {
        assert_eq!(match_order(InputClass::Sentence), [3, 2, 1]);
    }

    #[test]
    fn match_order_partial() {
        assert_eq!(match_order(InputClass::Partial), [2, 3, 1]);
    }

    #[test]
    fn match_order_word() {
        assert_eq!(match_order(InputClass::Word), [1, 2, 3]);
    }

    #[test]
    fn match_order_keyword_fallback() {
        assert_eq!(match_order(InputClass::KeywordFallback), [1, 2, 3]);
    }

    // --- InputClass round-trips ---

    #[test]
    fn input_class_from_i16_roundtrip() {
        for (v, expected) in [
            (1i16, InputClass::Word), // discriminant values match the protobuf/DB encoding
            (2, InputClass::Partial),
            (3, InputClass::Sentence),
            (4, InputClass::KeywordFallback),
        ] {
            assert_eq!(InputClass::from_i16(v), Some(expected));
            assert_eq!(expected.as_i16(), v);
        }
    }

    #[test]
    fn input_class_from_i16_invalid_returns_none() {
        assert_eq!(InputClass::from_i16(0), None);
        assert_eq!(InputClass::from_i16(5), None);
        assert_eq!(InputClass::from_i16(-1), None);
    }

    // --- IntentSource ---

    #[test]
    fn intent_source_learned_llm_needs_review() {
        assert!(IntentSource::LearnedLlm.needs_review());
        assert!(!IntentSource::Seeded.needs_review());
        assert!(!IntentSource::LearnedUser.needs_review());
        assert!(!IntentSource::LearnedFallback.needs_review());
    }

    #[test]
    fn intent_source_as_str_all_variants() {
        assert_eq!(IntentSource::Seeded.as_str(), "seeded");
        assert_eq!(IntentSource::LearnedUser.as_str(), "learned_user");
        assert_eq!(IntentSource::LearnedLlm.as_str(), "learned_llm");
        assert_eq!(IntentSource::LearnedFallback.as_str(), "learned_fallback");
    }

    // --- class_label ---

    #[test]
    fn class_label_known_codes() {
        // Core classes 0-3
        assert_eq!(class_label(0), "tool");
        assert_eq!(class_label(1), "skill_rusty");
        assert_eq!(class_label(2), "skill_monty");
        assert_eq!(class_label(3), "skill_llm");
        // Extensions 4-9
        assert_eq!(class_label(4), "extension_worker");
        assert_eq!(class_label(5), "extension_cron");
        assert_eq!(class_label(6), "extension_trigger");
        assert_eq!(class_label(7), "extension_webhook");
        assert_eq!(class_label(8), "extension_plan");
        assert_eq!(class_label(9), "extension_revision");
        // System classes
        assert_eq!(class_label(10), "orchestrator");
        assert_eq!(class_label(11), "reserved");
        // Former-doctype classes 12-20 (spec §4)
        assert_eq!(class_label(12), "spec");
        assert_eq!(class_label(13), "tool_skill");
        assert_eq!(class_label(14), "plan");
        assert_eq!(class_label(15), "summary");
        assert_eq!(class_label(16), "action");
        assert_eq!(class_label(17), "docu");
        assert_eq!(class_label(18), "lesson");
        assert_eq!(class_label(19), "issue");
        assert_eq!(class_label(20), "note");
        assert_eq!(class_label(21), "recipe");
        assert_eq!(class_label(22), "python_code");
        assert_eq!(class_label(23), "extension_catalogue");
        assert_eq!(class_label(50), "scaffold");
        assert_eq!(class_label(99), "component"); // unknown → generic
    }

    // --- Constants ---

    #[test]
    fn disambiguation_spread_constant_is_two() {
        assert_eq!(DISAMBIGUATION_SPREAD, 2);
    }

    #[test]
    fn score_cap_is_100() {
        assert_eq!(SCORE_CAP, 100);
    }
}
