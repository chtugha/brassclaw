//! Unified intent system — `__resolve_intent__` host function.
//!
//! Replaces the 8 intent-detection functions (`signals_tool_intent`,
//! `signals_execution_intent`, `score_skill`, `extract_explicit_skills`,
//! `llm_signals_tool_intent`, `user_signals_execution_intent`,
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
//! # DB-less mode
//!
//! When no pool is available, `resolve_intent` returns
//! `IntentResolution::DbLessFallback` immediately.  The orchestrator falls back
//! to keyword retrieval via `RamSource`.
//!
//! # Feature gate
//!
//! DB-path functions require the `skills-db` feature (same pool as skill loading).
//! The pure-Rust helpers (`classify_query`, `match_order`) are always available.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
}

/// Result of an intent resolution call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IntentResolution {
    /// Unambiguous single match.  Caller should score+1 and return the component.
    Match {
        component_id: Uuid,
        component_class_code: i32,
    },
    /// Multiple candidates within a 2-point score spread.  The WebUI must show
    /// a disambiguation message with clickable buttons (Q11).
    Disambiguation { candidates: Vec<IntentCandidate> },
    /// No match found in the intent table.  Orchestrator should emit a
    /// "reformulate" message (or silent fallback if "AI before User" is ON).
    NoMatch,
    /// The intent system is in DB-less mode.  Orchestrator falls back to
    /// keyword retrieval via `RamSource`.
    DbLessFallback,
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
        11 => "plan",
        16 => "action",
        21 => "recipe",
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
}

// ---------------------------------------------------------------------------
// DB functions — compiled only with `skills-db`
// ---------------------------------------------------------------------------

/// Scope for intent-system queries — must match the `reborn_skills` scope tuple.
#[cfg(feature = "skills-db")]
#[derive(Debug, Clone)]
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
    use tracing::debug;

    let query_class = classify_query(query);
    let order = match_order(query_class);
    // Vec<i16> implements ToSql for `= ANY($n)` in tokio_postgres.
    let order_vec: Vec<i16> = order.to_vec();

    // PERF-02: single query with CASE WHEN ordering.
    // The query scans rows for the exact input_text across all three preferred
    // classes, ordered by preference position, then score DESC.
    let client = pool.get().await.map_err(|e| IntentSystemError::Db(e.to_string()))?;

    let rows = client
        .query(
            "SELECT id, component_id, component_class_code, input_class, score
             FROM reborn_intent_inputs
             WHERE tenant_id   = $1
               AND user_id     = $2
               AND agent_id    = $3
               AND project_id  = $4
               AND input_text  = $5
               AND input_class = ANY($6)
             ORDER BY
               CASE input_class
                 WHEN $7 THEN 0
                 WHEN $8 THEN 1
                 WHEN $9 THEN 2
                 ELSE 3
               END,
               score DESC
             LIMIT 10",
            &[
                &scope.tenant_id as &(dyn tokio_postgres::types::ToSql + Sync),
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &query,
                &order_vec,
                &order[0],
                &order[1],
                &order[2],
            ],
        )
        .await
        .map_err(|e| IntentSystemError::Db(e.to_string()))?;

    if rows.is_empty() {
        debug!(query = %query, class = %query_class, "intent: no match");
        return Ok(IntentResolution::NoMatch);
    }

    let top_score: i32 = rows[0].get::<_, i32>(4);
    // Deduplicate by component_id, keeping the first (highest-score) occurrence.
    let mut seen_components = std::collections::HashSet::<Uuid>::new();
    let mut candidates: Vec<IntentCandidate> = Vec::new();

    for row in &rows {
        let score: i32 = row.get(4);
        if top_score - score > DISAMBIGUATION_SPREAD {
            break;
        }
        let component_id: Uuid = row.get(1);
        if seen_components.insert(component_id) {
            let component_class_code: i32 = row.get(2);
            candidates.push(IntentCandidate {
                row_id: row.get(0),
                component_id,
                component_class_code,
                input_class: row.get::<_, i16>(3),
                score,
                class_label: class_label(component_class_code),
            });
        }
    }

    if candidates.len() == 1 {
        let c = &candidates[0];
        // Atomic score increment (PERF-03); capped at SCORE_CAP (SEC-05).
        increment_score(&client, c.row_id).await?;
        debug!(
            component_id = %c.component_id,
            score = c.score,
            "intent: unambiguous match"
        );
        return Ok(IntentResolution::Match {
            component_id: c.component_id,
            component_class_code: c.component_class_code,
        });
    }

    // Multiple candidates within spread → disambiguation (Q11).
    debug!(
        count = candidates.len(),
        query = %query,
        "intent: disambiguation required"
    );
    Ok(IntentResolution::Disambiguation {
        candidates: candidates
            .into_iter()
            .take(MAX_DISAMBIGUATION_CANDIDATES)
            .collect(),
    })
}

/// Record the user's disambiguation choice: atomically increment the chosen
/// row's score and return the winning component.
#[cfg(feature = "skills-db")]
pub async fn record_disambiguation_choice(
    pool: &brassclaw_pg::PgPool,
    row_id: Uuid,
    component_id: Uuid,
    component_class_code: i32,
) -> Result<IntentResolution, IntentSystemError> {
    use tracing::debug;
    let client = pool.get().await.map_err(|e| IntentSystemError::Db(e.to_string()))?;
    increment_score(&client, row_id).await?;
    debug!(
        row_id = %row_id,
        component_id = %component_id,
        "intent: disambiguation choice recorded"
    );
    Ok(IntentResolution::Match {
        component_id,
        component_class_code,
    })
}

/// Seed (or update) an intent input row, typically called on component validation
/// to populate `intent_examples` into `reborn_intent_inputs` (spec §1.5).
///
/// Uses `INSERT … ON CONFLICT DO UPDATE` so re-seeding a component is idempotent.
#[cfg(feature = "skills-db")]
pub async fn seed_intent_input(
    pool: &brassclaw_pg::PgPool,
    scope: &IntentScope,
    input_text: &str,
    input_class: InputClass,
    component_id: Uuid,
    component_class_code: i32,
    source: IntentSource,
) -> Result<(), IntentSystemError> {
    let client = pool.get().await.map_err(|e| IntentSystemError::Db(e.to_string()))?;
    client
        .execute(
            "INSERT INTO reborn_intent_inputs
                 (tenant_id, user_id, agent_id, project_id,
                  input_text, input_class, component_id, component_class_code,
                  score, source, needs_review)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,1,$9,$10)
             ON CONFLICT (tenant_id, user_id, agent_id, project_id,
                          input_text, input_class, component_id)
             DO UPDATE SET
                 source       = EXCLUDED.source,
                 needs_review = EXCLUDED.needs_review,
                 updated_at   = now()",
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
    let client = pool.get().await.map_err(|e| IntentSystemError::Db(e.to_string()))?;
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
#[cfg(feature = "skills-db")]
async fn increment_score(
    client: &brassclaw_pg::PgClient,
    row_id: Uuid,
) -> Result<i32, IntentSystemError> {
    let row = client
        .query_one(
            "UPDATE reborn_intent_inputs
             SET score      = LEAST(score + 1, $1),
                 updated_at = now()
             WHERE id = $2
             RETURNING score",
            &[&SCORE_CAP, &row_id],
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
        assert_eq!(classify_query("list open github issues"), InputClass::Partial);
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
            (1_i16, InputClass::Word),
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
        assert_eq!(class_label(0), "tool");
        assert_eq!(class_label(1), "skill_rusty");
        assert_eq!(class_label(16), "action");
        assert_eq!(class_label(21), "recipe");
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
