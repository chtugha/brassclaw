# Contributing

## Getting Started

```bash
git clone https://github.com/chtugha/brassclaw.git
cd brassclaw
./scripts/dev-setup.sh
```

This installs the Rust toolchain, WASM targets, git hooks, and runs initial checks.

## How to Contribute

- Bug fixes, docs improvements, and focused cleanup tied to a concrete problem are welcome.
- Search existing issues and PRs before opening a new one to avoid duplicates.
- Keep changes scoped. One bug, one feature, or one documentation improvement per PR.

### Creating Issues

Open an issue when you are reporting a bug, proposing a feature, or documenting a gap in behavior.

For bug reports, include:

- What you expected to happen
- What actually happened
- Clear reproduction steps
- Relevant logs, screenshots, or error output
- Environment details when they matter (OS, database backend, feature flags, commit/branch)

For feature requests:

- Open an issue first before writing code
- Explain the problem being solved, not just the implementation idea
- Wait for maintainer feedback before investing in a large PR

We require an issue for new features so maintainers can prioritize the work and confirm it fits the roadmap before anyone spends time implementing it.

### Fixing Bugs

- Small, targeted bug-fix PRs are welcome
- If there is already an issue, link it in your PR
- If the bug is non-trivial, security-sensitive, or changes behavior across subsystems, open or confirm an issue first so the approach can be aligned before implementation

### Refactor-Only PRs

Refactor-only PRs are not accepted from contributors outside the core team. If a refactor is necessary to land a bug fix or approved feature, keep it minimal and clearly tied to that change.

## Development Workflow

```bash
cargo fmt                                                              # format
cargo clippy --all --benches --tests --examples --all-features         # lint (zero warnings)
cargo test                                                             # unit tests
cargo test --features integration                                      # + PostgreSQL tests

# Build the Reborn binary
cargo build --release -p brassclaw_reborn_cli --bin brassclaw-reborn 
```

These commands are for day-to-day iteration while you are developing locally. The pre-submission checks below are intentionally stricter and use CI-style flags so you can catch formatting drift and clippy warnings before requesting review.

## Before You Open a PR

Run the local validation checks required before requesting a review:

```bash
cargo fmt --all -- --check
cargo clippy --all --benches --tests --examples --all-features -- -D warnings
cargo build
cargo test
```

Also run this when your change touches database-backed or integration behavior:

```bash
cargo test --features integration
```

Before asking for review:

- Build and exercise the changed path locally, not just the narrowest unit test
- Keep the PR focused and avoid mixing unrelated concerns
- Fill out the PR template with a clear summary, validation notes, and impact assessment
- If your change affects tracked behavior, update `FEATURE_PARITY.md` in the same branch
- If onboarding or setup behavior changes, update the relevant setup docs in the same branch

## Review Follow-Through

Review conversations are author-owned.

- Address each review comment with a code change or a clear explanation
- Resolve conversations you have handled; leave them open only when reviewer judgment is still needed
- Do not leave review cleanup for maintainers when the follow-through belongs to the author

If a PR is stale for more than 48 hours after review feedback is posted, maintainers may take over the follow-up work and land the changes needed to accomplish the original PR or issue intent.

## Code Style

- Zero clippy warnings policy
- No `.unwrap()` or `.expect()` in production code (tests are fine)
- Use `thiserror` for error types, map errors with context
- Prefer `crate::` for cross-module imports
- No `pub use` re-exports unless exposing to downstream consumers
- Use strong types and enums over stringly-typed control flow
- Comments for non-obvious logic only

See `CLAUDE.md` for full style guidelines.

## Architecture Boundaries

The codebase has two distinct runtimes:

- **Reborn** (`crates/`): the new architecture. New features and Reborn-owned surfaces belong here.
- **v1** (`src/`): the legacy runtime. Do not modify v1 files unless your task explicitly targets v1 behavior.

Do not mix patterns between the two runtimes. See `CLAUDE.md` for the four-layer Reborn model.

## Database Changes

BrassClaw uses dual-backend persistence (PostgreSQL + libSQL). All new persistence features must support both backends.

- Add new DB operations to the shared `Database` trait first, then implement both backends.
- Do not collapse bootstrap config, DB-backed settings, and encrypted secrets into each other.

See `src/db/CLAUDE.md` and `.claude/rules/database.md`.

## Adding Dependencies

Run `cargo deny check` before adding new dependencies to verify license compatibility and check for known advisories.

## Feature Parity Requirement

When your change affects a tracked capability, update `FEATURE_PARITY.md` in the same branch.

### Required before opening a PR

1. Review the relevant parity rows in `FEATURE_PARITY.md`.
2. Update status/notes if behavior changed.
3. Include the `FEATURE_PARITY.md` diff in your commit when applicable.

## Review Tracks

All PRs follow a risk-based review process:

| Track | Scope | Requirements |
|-------|-------|-------------|
| **A** | Docs, tests, chore, dependency bumps | 1 approval + CI green |
| **B** | Features, maintainer-requested refactors, new tools/channels | 1 approval + CI green + test evidence |
| **C** | Security (`crates/brassclaw_safety/`, `src/secrets/`), runtime (`crates/brassclaw_reborn/`, `src/agent/`, `src/worker/`), database schema, CI workflows | 2 approvals + rollback plan documented |

Select the appropriate track in the PR template based on what your changes touch.

## Skills Documentation

SKILL.md files extend the agent's prompt with domain-specific instructions. When contributing a new skill or modifying an existing one:

- Trusted skills live in `~/.brassclaw/skills/` or the workspace `skills/` directory.
- Installed skills come from the registry and are subject to read-only tool attenuation.
- Skills are selected by a scoring pipeline: gating -> scoring -> budget (2,048 tokens) -> attenuation.
- Read `.claude/rules/skills.md` before authoring a new skill.

## Documenting Your Changes

- The `docs/` folder contains user-facing documentation built with Mintlify.
- For features, update the relevant capability doc in `docs/capabilities/`.
- For channels, update the relevant channel doc in `docs/channels/`.
- For extensions and tools, update `docs/extensions/`.
- Internal reference documentation goes in `docs/internal/`.
- Plans and design records go in `docs/plans/`.

### Testing Documentation

```bash
cd docs
mint dev           # preview
mint broken-links  # check internal links
```

Read `.claude/skills/mintlify-docs` for Mintlify authoring guidelines.
