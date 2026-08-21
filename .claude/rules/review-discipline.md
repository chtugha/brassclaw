---
paths:
  - "crates/**/*.rs"
  - "tests/**"
---
# Review & Fix Discipline

Hard-won lessons from code review — follow these when fixing bugs or addressing review feedback.

**Fix the pattern, not just the instance:** When a reviewer flags a bug, search the entire codebase for all instances of that same pattern. A fix in one store that doesn't also fix the same pattern in a sibling store is half a fix.

**Propagate architectural fixes to related types:** If a core type changes its concurrency model or interface, every type that interacts with it must also be updated. Grep for the old type across the codebase.

**Zero clippy warnings policy:** Fix ALL clippy warnings before committing, including pre-existing ones in files you didn't change. Never leave warnings behind.

**Regression test with every fix:** Every bug fix must include a test that would have caught the bug. Add a `#[test]` or `#[tokio::test]` that reproduces the original failure. Use `[skip-regression-check]` in commit message only if genuinely not feasible.

**Transaction safety:** Multi-step database operations (INSERT+INSERT, UPDATE+DELETE, read-then-write) MUST be wrapped in a transaction. Never assume sequential calls are atomic.

**UTF-8 string safety:** Never use byte-index slicing (`&s[..n]`) on user-supplied or external strings — it panics on multi-byte characters. Use `is_char_boundary()` or `char_indices()`.

**Case-insensitive comparisons:** When comparing user-supplied strings (file paths, media types, extension names), normalize to lowercase with `.to_ascii_lowercase()`.

**Sensitive data in logs & events:** Tool parameters and outputs MUST be redacted before logging or broadcasting via SSE/WebSocket. Use `redact_params()` or the `brassclaw_safety` redaction pipeline before any `tracing::info!` or event emission that includes tool call data. Note: `info!` and `warn!` output appears in the Reborn REPL and corrupts the terminal UI — use `debug!` for internal diagnostics.

**Decorator/wrapper trait delegation:** When adding a new method to a trait with decorator wrappers (e.g., `LlmProvider`), update ALL wrapper types to delegate. Grep for `impl <Trait> for` to find all implementations.

**Mechanical verification before committing:**
- `cargo clippy --all --benches --tests --examples --all-features -- -D warnings` — zero warnings
- `grep -rnE '\.unwrap\(|\.expect\(' <files>` — no panics in production
- `grep -rn 'super::' <files>` — prefer `crate::` for cross-module imports (`super::` OK in tests/intra-module)
- If you fixed a pattern bug, `grep` for other instances across `crates/`

## PR Scope Discipline

A PR's title and body must match its diff.

- If the title describes one change but the diff spans multiple layers, retitle, split, or explicitly call out the scope expansion in the body.
- **Move-only refactors** must state "no behavior change" in the body and file a follow-up issue for every pre-existing correctness/perf concern surfaced during the move. Don't silently fix things mid-move — it's unreviewable.
- After a refactor that relocates or renames code, grep for `.md` and `AGENTS.md`/`CLAUDE.md` references to the moved paths and update them in the same PR.

## Guardrail Scripts Are Code

Lint/boundary/safety scripts under `scripts/` are enforcement infrastructure. They must:

- **Have regression tests** exercising every documented exemption.
- **Be included in CI** that gates required checks — a guardrail that isn't run on changes to itself can be weakened without anyone noticing.
- **Actually enforce their documented skips** — if the exemption says "skips `#[cfg(test)]` blocks", the scanner must track brace nesting, not match a regex on the first line.

## Stale Comments After Refactors

Doc strings and inline comments are part of the contract. When you change behavior in a function, re-read its docstring and adjacent comments — update or delete them in the same change.
