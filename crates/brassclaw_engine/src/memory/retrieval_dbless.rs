//! DB-less keyword retrieval helpers.
//!
//! These functions implement the pre-intent-system keyword-based retrieval path
//! that runs when no database is available (`RamSource` / DB-less mode). In a
//! full DB-backed deployment the intent system (`__resolve_intent__`, §3.12)
//! resolves queries to component IDs before the `PostgresSource` fetches them
//! by ID; this file is not used in that path.
//!
//! # Spec references
//! - §3.4 — PlanA-Memory as the universal retrieval connector
//! - §3.4 (DB-less fallback) — keyword retrieval over the fallback-content file
//! - §3.12 rule f-fallback — "try it with AI" fallback reuses `extract_keywords`

use crate::types::memory::{DocType, MemoryDoc};

/// Extract lowercase keywords from a query, filtering out stop words.
///
/// Used by the DB-less keyword-retrieval path and the "try it with AI" fallback
/// (§3.12 rule f-fallback). Not called in DB-backed intent-driven retrieval.
pub(crate) fn extract_keywords(query: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "shall", "can",
        "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "into", "about", "it",
        "its", "this", "that", "these", "those", "i", "you", "he", "she", "we", "they", "what",
        "which", "who", "how", "when", "where", "why", "and", "or", "but", "not", "no", "if",
        "then", "so", "up", "out", "just",
    ];

    query
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 2 && !STOP_WORDS.contains(&w.as_str()))
        .collect()
}

/// Score how well a doc matches the given keywords (0.0 to 1.0).
///
/// Title matches count double relative to content matches. Used only in the
/// DB-less keyword-retrieval path.
pub(crate) fn keyword_match_score(doc: &MemoryDoc, keywords: &[String]) -> f64 {
    if keywords.is_empty() {
        return 0.0;
    }

    let title_lower = doc.title.to_lowercase();
    let content_lower = doc.content.to_lowercase();

    let mut matched = 0usize;
    for kw in keywords {
        // Title matches are worth more
        if title_lower.contains(kw.as_str()) {
            matched += 2;
        } else if content_lower.contains(kw.as_str()) {
            matched += 1;
        }
    }

    // Normalize: max possible score is keywords.len() * 2 (all in title)
    let max_score = keywords.len() * 2;
    matched as f64 / max_score as f64
}

/// Priority weight by doc type. Higher = more useful for context injection.
///
/// Used only in the DB-less keyword-retrieval path. In DB-backed mode the
/// intent system resolves routing directly to component IDs.
pub(crate) fn doc_type_weight(doc_type: DocType) -> f64 {
    match doc_type {
        DocType::Spec => 0.5,      // Missing capability info is highest priority
        DocType::Skill => 0.45,    // Skills with activation metadata and code snippets
        DocType::Lesson => 0.4,    // Lessons prevent repeating mistakes
        DocType::Issue => 0.2,     // Known problems
        DocType::Summary => 0.1,   // Background context
        DocType::Note => 0.05,     // Scratch notes, lowest priority
        DocType::Plan => 0.3,      // Execution plans with structured steps
        DocType::Recipe => 0.35,   // Recipes chain ToolSkills — useful past Skill
        DocType::ToolSkill => 0.4, // ToolSkills describe how to call a tool
    }
}

/// Priority weight by class code for fallback-content file keyword search.
///
/// Mirrors `doc_type_weight` but uses class codes for the new component tables.
/// Used by `RamSource::search_fallback_entries`.
/// Class codes per spec §3.7:
///   0=tool, 1-3=skills, 4-9=extensions, 10=orchestrator, 12=spec,
///   13=tool_skill, 14=plan, 15=summary, 16=action, 17=docu, 18=lesson,
///   19=issue, 20=note, 21=recipe, 50=scaffold.
pub(crate) fn doc_type_weight_by_class(class_code: i32) -> f64 {
    match class_code {
        50 => 0.55, // Scaffold — highest priority, shapes prompt structure
        10 => 0.52, // Orchestrator — core execution logic
        0  => 0.50, // Tool — Rusty, minimal content
        1..=3 => 0.45, // Skills (Rusty / Monty / LLM)
        4..=9 => 0.42, // Extensions
        21 => 0.38, // Recipe — chained solutions
        13 => 0.40, // ToolSkill
        12 => 0.50, // Spec — missing capability docs
        14 => 0.30, // Plan
        18 => 0.40, // Lesson — prevent repeating mistakes
        19 => 0.20, // Issue
        15 => 0.10, // Summary
        17 => 0.25, // Docu
        20 => 0.05, // Note
        16 => 0.35, // Action
        _  => 0.10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::memory::DocType;
    use crate::types::project::ProjectId;

    #[test]
    fn extract_keywords_filters_stop_words() {
        let kws = extract_keywords("what is the latest news about Iran war");
        assert!(kws.contains(&"latest".to_string()));
        assert!(kws.contains(&"news".to_string()));
        assert!(kws.contains(&"iran".to_string()));
        assert!(kws.contains(&"war".to_string()));
        assert!(!kws.contains(&"the".to_string()));
        assert!(!kws.contains(&"is".to_string()));
    }

    #[test]
    fn extract_keywords_handles_special_chars() {
        let kws = extract_keywords("web_search web-fetch tool");
        assert!(kws.contains(&"web_search".to_string()));
        assert!(kws.contains(&"web-fetch".to_string()));
        assert!(kws.contains(&"tool".to_string()));
    }

    #[test]
    fn keyword_match_title_beats_content() {
        let doc = MemoryDoc::new(
            ProjectId::new(),
            "test-user",
            DocType::Lesson,
            "Lesson about web_search errors",
            "The tool was not found during execution.",
        );

        let keywords = vec!["web_search".to_string()];
        let score = keyword_match_score(&doc, &keywords);
        // Title match = 2/2 = 1.0
        assert!((score - 1.0).abs() < f64::EPSILON);

        let keywords2 = vec!["execution".to_string()];
        let score2 = keyword_match_score(&doc, &keywords2);
        // Content-only match = 1/2 = 0.5
        assert!((score2 - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn doc_type_weight_ordering() {
        assert!(doc_type_weight(DocType::Spec) > doc_type_weight(DocType::Lesson));
        assert!(doc_type_weight(DocType::Lesson) > doc_type_weight(DocType::Issue));
        assert!(doc_type_weight(DocType::Issue) > doc_type_weight(DocType::Summary));
        assert!(doc_type_weight(DocType::Summary) > doc_type_weight(DocType::Note));
    }
}
