//! Context retrieval engine.
//!
//! Builds context for thread steps by retrieving relevant memory docs
//! from the project. Uses keyword matching against doc title + content,
//! with priority scoring by doc type (Lessons and Specs rank higher
//! than Summaries for context injection).
//!
//! The keyword-matching helpers (`extract_keywords`, `keyword_match_score`,
//! `doc_type_weight`) live in [`super::retrieval_dbless`] — they are the
//! DB-less fallback path and should not be duplicated here.

use std::sync::Arc;

use crate::traits::store::Store;
use crate::types::error::EngineError;
use crate::types::memory::MemoryDoc;
use crate::types::project::ProjectId;
use super::retrieval_dbless::{doc_type_weight, extract_keywords, keyword_match_score};

/// Retrieves relevant memory docs for a thread's context.
pub struct RetrievalEngine {
    store: Arc<dyn Store>,
}

impl RetrievalEngine {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    /// Retrieve relevant memory docs for the given query within a project.
    ///
    /// Loads all docs for the project, scores them by keyword relevance and
    /// doc-type priority, and returns the top `max_docs` results.
    pub async fn retrieve_context(
        &self,
        project_id: ProjectId,
        user_id: &str,
        query: &str,
        max_docs: usize,
    ) -> Result<Vec<MemoryDoc>, EngineError> {
        if max_docs == 0 {
            return Ok(Vec::new());
        }

        // Include both user-owned and shared system docs for context retrieval.
        let all_docs = self
            .store
            .list_memory_docs_with_shared(project_id, user_id)
            .await?;
        if all_docs.is_empty() {
            return Ok(Vec::new());
        }

        let keywords = extract_keywords(query);
        if keywords.is_empty() {
            // No meaningful keywords — return by doc-type priority alone
            let mut scored: Vec<(f64, MemoryDoc)> = all_docs
                .into_iter()
                .map(|doc| (doc_type_weight(doc.doc_type), doc))
                .collect();
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(max_docs);
            return Ok(scored.into_iter().map(|(_, doc)| doc).collect());
        }

        let mut scored: Vec<(f64, MemoryDoc)> = all_docs
            .into_iter()
            .map(|doc| {
                let keyword_score = keyword_match_score(&doc, &keywords);
                let type_weight = doc_type_weight(doc.doc_type);
                // Combined score: keyword relevance (0.0-1.0) + type priority bonus
                let score = keyword_score + type_weight;
                (score, doc)
            })
            .filter(|(score, _)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_docs);
        Ok(scored.into_iter().map(|(_, doc)| doc).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::memory::DocType;
    use crate::types::project::ProjectId;

    fn make_store(docs: Vec<MemoryDoc>) -> Arc<crate::tests::InMemoryStore> {
        Arc::new(crate::tests::InMemoryStore::with_docs(docs))
    }

    #[tokio::test]
    async fn retrieve_returns_relevant_docs_by_keyword() {
        let project = ProjectId::new();
        let store = make_store(vec![
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Lesson,
                "web_search tool alias",
                "Use web_search",
            ),
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Summary,
                "weather query",
                "Fetched weather data",
            ),
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Issue,
                "API timeout",
                "External API timed out",
            ),
        ]);
        let engine = RetrievalEngine::new(store);

        let docs = engine
            .retrieve_context(project, "test-user", "web_search error", 5)
            .await
            .unwrap();
        assert!(!docs.is_empty());
        // The lesson about web_search should rank first (keyword + type weight)
        assert_eq!(docs[0].doc_type, DocType::Lesson);
        assert!(docs[0].title.contains("web_search"));
    }

    #[tokio::test]
    async fn retrieve_respects_project_scoping() {
        let project_a = ProjectId::new();
        let project_b = ProjectId::new();
        let store = make_store(vec![
            MemoryDoc::new(
                project_a,
                "test-user",
                DocType::Lesson,
                "Lesson for project A",
                "Some lesson",
            ),
            MemoryDoc::new(
                project_b,
                "test-user",
                DocType::Lesson,
                "Lesson for project B",
                "Other lesson",
            ),
        ]);
        let engine = RetrievalEngine::new(store);

        let docs_a = engine
            .retrieve_context(project_a, "test-user", "lesson", 5)
            .await
            .unwrap();
        assert_eq!(docs_a.len(), 1);
        assert!(docs_a[0].title.contains("project A"));

        let docs_b = engine
            .retrieve_context(project_b, "test-user", "lesson", 5)
            .await
            .unwrap();
        assert_eq!(docs_b.len(), 1);
        assert!(docs_b[0].title.contains("project B"));
    }

    #[tokio::test]
    async fn retrieve_respects_max_docs_limit() {
        let project = ProjectId::new();
        let store = make_store(vec![
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Lesson,
                "Lesson 1",
                "Content 1",
            ),
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Lesson,
                "Lesson 2",
                "Content 2",
            ),
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Lesson,
                "Lesson 3",
                "Content 3",
            ),
        ]);
        let engine = RetrievalEngine::new(store);

        let docs = engine
            .retrieve_context(project, "test-user", "lesson", 2)
            .await
            .unwrap();
        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn retrieve_empty_store_returns_empty() {
        let project = ProjectId::new();
        let store = make_store(vec![]);
        let engine = RetrievalEngine::new(store);

        let docs = engine
            .retrieve_context(project, "test-user", "anything", 5)
            .await
            .unwrap();
        assert!(docs.is_empty());
    }

    #[tokio::test]
    async fn retrieve_spec_ranks_above_summary() {
        let project = ProjectId::new();
        let store = make_store(vec![
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Summary,
                "Summary of search",
                "searched the web",
            ),
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Spec,
                "Missing search tool",
                "Use web_search for the search tool",
            ),
        ]);
        let engine = RetrievalEngine::new(store);

        let docs = engine
            .retrieve_context(project, "test-user", "search", 5)
            .await
            .unwrap();
        assert_eq!(docs.len(), 2);
        // Spec should rank first due to higher type weight
        assert_eq!(docs[0].doc_type, DocType::Spec);
    }
}
