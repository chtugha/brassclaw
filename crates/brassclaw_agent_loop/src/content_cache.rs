//! Semantic content cache for large tool outputs.
//!
//! When a tool result exceeds `content_cache_threshold` tokens, the executor
//! caches the full content here and replaces the context view with a compact
//! stub. The model retrieves full content or filtered sections via the
//! first-party tool `fetch_cached_content`.
//!
//! ## Architecture
//!
//! `ContentCacheState` is stored in `LoopExecutionState` for checkpoint
//! persistence. A `ContentCacheBridge` (`Arc<Mutex<ContentCacheState>>`) is
//! shared between the `ContentCacheCapabilityPortDecorator` (writer) and the
//! `FetchCachedContentHandler` (reader). The decorator intercepts completed
//! capability results at the port level; the handler reads from the bridge
//! during the same turn.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

// ── CachedEntry ──────────────────────────────────────────────────────────────

/// A single cached tool output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedEntry {
    /// Stable cache key (auto-generated: `<tool_id>-<iteration>-<counter>`).
    pub key: String,
    /// Tool capability ID that produced this output.
    pub tool_id: String,
    /// Semantic categories inferred from `tool_id` (e.g. `["filesystem", "read"]`).
    pub categories: Vec<String>,
    /// Full original content.
    pub full_content: String,
    /// Estimated token count for `full_content`.
    pub token_estimate: usize,
    /// First 100 characters of `full_content`, used in stub line.
    pub preview: String,
    /// Loop iteration number when the entry was created.
    pub iteration: usize,
    /// Number of times the model has fetched this entry via `fetch_cached_content`.
    pub fetch_count: u32,
}

impl CachedEntry {
    pub fn new(
        key: String,
        tool_id: String,
        full_content: String,
        iteration: usize,
    ) -> Self {
        let categories = tool_id_to_categories(&tool_id);
        let token_estimate = estimate_tokens(&full_content);
        let preview = full_content.chars().take(100).collect();
        Self {
            key,
            tool_id,
            categories,
            full_content,
            token_estimate,
            preview,
            iteration,
            fetch_count: 0,
        }
    }

    /// Build the compact stub string injected into the model context.
    pub fn stub(&self) -> String {
        let categories = self.categories.join(",");
        format!(
            "[CACHED:{}|type:{}|iter:{}|tokens:{}|categories:{}|preview:{}]",
            self.key,
            self.tool_id,
            self.iteration,
            self.token_estimate,
            categories,
            self.preview
        )
    }
}

// ── ContentCacheState ─────────────────────────────────────────────────────────

/// Serializable/checkpointable content cache for one agent run.
///
/// Stored as `#[serde(default)]` in `LoopExecutionState` so existing
/// checkpoints deserialize correctly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentCacheState {
    /// All cached entries for this run, keyed by cache key.
    pub entries: HashMap<String, CachedEntry>,
    /// When `true`, subtask 5 post-turn hook has processed this cache.
    pub cleared: bool,
    /// Running counter used for unique key generation within a turn.
    #[serde(default)]
    pub entry_counter: u32,
}

impl ContentCacheState {
    /// Insert a new entry. Returns the stub string to replace the raw content with.
    pub fn insert(&mut self, entry: CachedEntry) -> String {
        let stub = entry.stub();
        self.entries.insert(entry.key.clone(), entry);
        stub
    }

    /// Fetch the full content (or filtered lines) for a key. Increments `fetch_count`.
    pub fn fetch(&mut self, key: &str, filter: Option<&str>) -> String {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.fetch_count += 1;
            if let Some(filter_str) = filter {
                let filtered: Vec<&str> = entry
                    .full_content
                    .lines()
                    .filter(|line| line.contains(filter_str))
                    .collect();
                if filtered.is_empty() {
                    format!(
                        "No lines matching '{}' in cached content for '{}'.",
                        filter_str, key
                    )
                } else {
                    filtered.join("\n")
                }
            } else {
                entry.full_content.clone()
            }
        } else {
            let available: Vec<&str> = self.entries.keys().map(String::as_str).collect();
            if available.is_empty() {
                format!("No cached content for '{}'. Cache is empty.", key)
            } else {
                format!(
                    "No cached content for '{}'. Available keys: [{}]",
                    key,
                    available.join(", ")
                )
            }
        }
    }

    /// Generate the next unique cache key for the given tool and iteration.
    pub fn next_key(&mut self, tool_id: &str, iteration: usize) -> String {
        self.entry_counter += 1;
        // Strip namespace prefix for brevity (e.g. "builtin.shell" → "shell")
        let short_id = tool_id.split('.').next_back().unwrap_or(tool_id);
        format!("{}-iter{}-{}", short_id, iteration, self.entry_counter)
    }
}

// ── ContentCacheBridge ────────────────────────────────────────────────────────

/// Shared mutable handle to the live content cache for the current run.
///
/// Created once per session and cloned into both the capability port decorator
/// (writer path) and the `fetch_cached_content` first-party handler (reader path).
#[derive(Debug, Clone, Default)]
pub struct ContentCacheBridge(pub Arc<Mutex<ContentCacheState>>);

impl ContentCacheBridge {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(ContentCacheState::default())))
    }

    /// Lock and apply a closure to the cache state.
    pub fn with_lock<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut ContentCacheState) -> R,
    {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut guard)
    }

    /// Snapshot the current state (for checkpoint serialisation).
    pub fn snapshot(&self) -> ContentCacheState {
        self.with_lock(|s| s.clone())
    }
}

// ── Token estimator ───────────────────────────────────────────────────────────

/// Rough token count: 4 bytes ≈ 1 token (standard heuristic).
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() / 4).max(1)
}

// ── Category classifier ───────────────────────────────────────────────────────

/// Map a tool capability ID to semantic categories.
///
/// Pure function — no LLM, no I/O.
pub fn tool_id_to_categories(tool_id: &str) -> Vec<String> {
    let lower = tool_id.to_ascii_lowercase();
    let id = lower.as_str();

    // Common first-party and builtin tool IDs
    if id.contains("shell") || id.contains("process") || id.contains("exec") {
        return vec!["shell".to_string(), "output".to_string()];
    }
    if id.contains("read_file") || id.contains("readfile") || id.contains("get_file") {
        return vec!["filesystem".to_string(), "read".to_string()];
    }
    if id.contains("write_file") || id.contains("writefile") || id.contains("put_file") {
        return vec!["filesystem".to_string(), "write".to_string()];
    }
    if id.contains("list") || id.contains("ls") || id.contains("dir") {
        return vec!["filesystem".to_string(), "list".to_string()];
    }
    if id.contains("search") || id.contains("grep") || id.contains("find") {
        return vec!["search".to_string()];
    }
    if id.contains("web") || id.contains("http") || id.contains("fetch") {
        return vec!["web".to_string(), "content".to_string()];
    }
    if id.contains("memory") || id.contains("recall") {
        return vec!["memory".to_string()];
    }
    if id.contains("git") {
        return vec!["git".to_string()];
    }
    vec!["tool".to_string(), "output".to_string()]
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_entry_stub_format() {
        let entry = CachedEntry::new(
            "shell-iter0-1".to_string(),
            "builtin.shell".to_string(),
            "hello world output".to_string(),
            0,
        );
        let stub = entry.stub();
        assert!(stub.starts_with("[CACHED:shell-iter0-1|"));
        assert!(stub.contains("type:builtin.shell"));
        assert!(stub.contains("iter:0"));
    }

    #[test]
    fn content_cache_fetch_increments_count() {
        let mut cache = ContentCacheState::default();
        let entry = CachedEntry::new("k1".to_string(), "builtin.shell".to_string(), "data".to_string(), 0);
        cache.insert(entry);

        let _ = cache.fetch("k1", None);
        assert_eq!(cache.entries["k1"].fetch_count, 1);
    }

    #[test]
    fn content_cache_fetch_missing_key() {
        let mut cache = ContentCacheState::default();
        let result = cache.fetch("nonexistent", None);
        assert!(result.contains("Cache is empty"));
    }

    #[test]
    fn content_cache_fetch_with_filter() {
        let mut cache = ContentCacheState::default();
        let entry = CachedEntry::new(
            "k1".to_string(),
            "builtin.shell".to_string(),
            "line one\nline two\nthird line".to_string(),
            0,
        );
        cache.insert(entry);
        let result = cache.fetch("k1", Some("two"));
        assert_eq!(result.trim(), "line two");
    }

    #[test]
    fn token_estimate_basic() {
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("a".repeat(400).as_str()), 100);
    }

    #[test]
    fn tool_id_to_categories_shell() {
        assert_eq!(tool_id_to_categories("builtin.shell"), vec!["shell", "output"]);
    }

    #[test]
    fn tool_id_to_categories_web() {
        assert_eq!(tool_id_to_categories("web.get_content"), vec!["web", "content"]);
    }

    #[test]
    fn content_cache_state_round_trips_json() {
        let mut cache = ContentCacheState::default();
        let entry = CachedEntry::new("k1".to_string(), "builtin.shell".to_string(), "output".to_string(), 1);
        cache.insert(entry);
        let json = serde_json::to_string(&cache).expect("serialize");
        let restored: ContentCacheState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, cache);
    }
}
