//! Subagent direction prompts loaded from the DB at boot.
//!
//! The four direction prompts (`general`, `researcher`, `explorer`, `coder`)
//! are seeded as class-10 `source='system'` rows in `reborn_skills` by
//! `builtin_bootstrap::seed_orchestrator()`.  They are loaded into process
//! memory by `init_directions()`, called from `webui.rs` after the content
//! integrity check passes.
//!
//! The public API `direction_prompt(id)` is unchanged — callers need no
//! modification.

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DirectionId {
    General,
    Researcher,
    Explorer,
    Coder,
}

impl DirectionId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Researcher => "researcher",
            Self::Explorer => "explorer",
            Self::Coder => "coder",
        }
    }
}

static DIRECTION_GENERAL: OnceLock<String> = OnceLock::new();
static DIRECTION_RESEARCHER: OnceLock<String> = OnceLock::new();
static DIRECTION_EXPLORER: OnceLock<String> = OnceLock::new();
static DIRECTION_CODER: OnceLock<String> = OnceLock::new();

/// Returns the direction prompt for the given `DirectionId`.
///
/// # Panics
///
/// Panics if [`init_directions`] was not called before this function.
/// This will happen in test contexts unless the test calls `init_directions`
/// first; the panic message identifies the fix.
pub fn direction_prompt(id: DirectionId) -> &'static str {
    match id {
        DirectionId::General => DIRECTION_GENERAL.get().expect(
            "direction prompts not initialised; \
                 call brassclaw_reborn::subagent::directions::init_directions() \
                 at boot (after run_content_integrity_check in webui.rs)",
        ),
        DirectionId::Researcher => DIRECTION_RESEARCHER
            .get()
            .expect("direction prompts not initialised; call init_directions() at boot"),
        DirectionId::Explorer => DIRECTION_EXPLORER
            .get()
            .expect("direction prompts not initialised; call init_directions() at boot"),
        DirectionId::Coder => DIRECTION_CODER
            .get()
            .expect("direction prompts not initialised; call init_directions() at boot"),
    }
}

/// Initialise all direction prompts from DB rows loaded at boot.
/// Called once from `webui.rs` after `run_content_integrity_check` passes.
/// Subsequent calls after first initialisation are silently ignored.
pub fn init_directions(general: String, researcher: String, explorer: String, coder: String) {
    let _ = DIRECTION_GENERAL.set(general);
    let _ = DIRECTION_RESEARCHER.set(researcher);
    let _ = DIRECTION_EXPLORER.set(explorer);
    let _ = DIRECTION_CODER.set(coder);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_prompts_are_non_empty() {
        // Test-local seed — no compiled-in fallback exists after this migration.
        const GENERAL_SEED: &str = include_str!("general.md");
        const RESEARCHER_SEED: &str = include_str!("researcher.md");
        const EXPLORER_SEED: &str = include_str!("explorer.md");
        const CODER_SEED: &str = include_str!("coder.md");

        init_directions(
            GENERAL_SEED.to_string(),
            RESEARCHER_SEED.to_string(),
            EXPLORER_SEED.to_string(),
            CODER_SEED.to_string(),
        );
        assert!(!direction_prompt(DirectionId::General).trim().is_empty());
        assert!(!direction_prompt(DirectionId::Researcher).trim().is_empty());
        assert!(!direction_prompt(DirectionId::Explorer).trim().is_empty());
        assert!(!direction_prompt(DirectionId::Coder).trim().is_empty());
    }

    #[test]
    fn direction_id_as_str_is_stable() {
        assert_eq!(DirectionId::General.as_str(), "general");
        assert_eq!(DirectionId::Researcher.as_str(), "researcher");
        assert_eq!(DirectionId::Explorer.as_str(), "explorer");
        assert_eq!(DirectionId::Coder.as_str(), "coder");
    }
}
