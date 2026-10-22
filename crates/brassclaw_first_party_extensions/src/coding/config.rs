use glob::MatchOptions;

pub(super) const DEFAULT_SCOPED_ROOT: &str = "/workspace";

pub(super) const WORKSPACE_FILES: &[&str] = &[
    "HEARTBEAT.md",
    "MEMORY.md",
    "IDENTITY.md",
    "SOUL.md",
    "AGENTS.md",
    "USER.md",
];

pub(super) const GLOB_MATCH_OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: false,
};

pub(super) const DEFAULT_EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    "coverage",
    ".venv",
    "venv",
    "__pycache__",
];
