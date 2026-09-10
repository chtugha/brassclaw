# Subplan P.1 — Migrate on-disk system skills to full v3 component stacks

**Parent:** `saved_plan_to_v3.md` Phase P.1  
**Status:** [x] DONE — all steps complete. See `saved_plan_to_v3.md` Phase P.1 for the final summary.

## Architecture rule (from CLAUDE.md)

Each SKILL.md is a **use-case** that must be **dissected** into the full v3 component stack:
- **Tool (class 0)** — Rust capability, if a new one is needed
- **ToolSkill (class 13)** — executor-facing binding for each Tool verb
- **PythonCode (class 22)** — the actual executor: calls `host.<tool>(...)`, one dispatch per body
- **Leaf Skill (class 1)** — orchestrator-facing narrative for ONE approach to ONE tool
- **Domain/Cross-domain Skill (class 2/3)** — narrative that references leaf skills by name, overview of the workflow
- **Recipe (class 21)** — intent examples + step_descriptions JSONB + step_link; Tier 0 where deterministic, Tier 1 where LLM needed
- **ExtensionCatalogue (class 23)** — groups the full stack, one per domain

**The SKILL.md body text becomes Skill (class 2/3) narrative AND Recipe step bodies.**  
Tool calls in the skill body become PythonCode executors + Recipes.  
Intent patterns in the frontmatter become `intent_examples` on Recipe rows.

---

## Audit: what each SKILL.md actually does

### `coding` — tool-use best practices, no new tool needed

Describes how to use `read_file`, `apply_patch`, `glob`, `grep`, `list_dir`, `shell`.
→ **No new Tool rows.** Only new components: a domain Skill (class 2) carrying the best-practices
  narrative, and a set of intent examples wired to existing recipes.  
→ The `coding` domain Skill becomes the carrier for the "load coding best practices context" step
  inside complex Tier-1 code-editing recipes.

**New components:**
- `skill-coding` (class 2) — the best-practices narrative body (referencing existing leaf skills by name)
- `ext-coding` (class 23) — ExtensionCatalogue grouping the coding domain skill

**New Recipes using existing components:**
- None needed independently: coding best practices load as a domain Skill step inside existing
  Tier-1 recipes (`file-write`, `file-patch`, `shell-run`, `shell-script`). No new standalone Recipe.

---

### `commit` — orchestrated git commit workflow

Steps: `shell git status` → `shell git diff --cached` → `shell git log --oneline -5` → LLM drafts message → user confirms → `shell git add <files>` → `shell git commit`.

→ **No new Tool rows.** Uses `builtin.shell` (already seeded).  
→ The `shell-git-commit` Recipe in `builtin_stuff_v3.md` Step 1.x.18 already covers the commit
  dispatch. What's missing: the **full orchestrated commit workflow** recipe that loads context
  first (status + diff + log) then hands to LLM for message drafting.

**New components:**
- `skill-commit-workflow` (class 2) — narrative: check staged files, check diff, check log style, draft message, confirm, commit
- Recipe `commit-workflow` (class 21, Tier 1) — loads `shell-git-status`, `shell-git-diff-stat`,
  `shell-git-log` steps, then LLM step for message drafting, then `shell-git-add` + `shell-git-commit`

**Intent examples for `commit-workflow`:**
"commit my changes", "create a git commit", "git commit", "commit staged files", "commit with a good message", "make a commit", "commit and push", "write a commit message for my changes", "git commit -m", "stage and commit"

---

### `github` — GitHub API via http tool with credential injection

Describes how to call `https://api.github.com` endpoints: issues, PRs, repo, search, authenticated user.
→ **No new Tool rows.** Uses `builtin.http` (already seeded).  
→ Needs leaf skills per operation pattern + a domain skill + Tier-0 recipes for deterministic GETs.

**New components:**

Leaf Skills (class 1):
- `skill-github-list-issues` — GET /repos/{owner}/{repo}/issues
- `skill-github-get-pr` — GET /repos/{owner}/{repo}/pulls/{number}
- `skill-github-get-pr-diff` — GET with Accept: application/vnd.github.v3.diff
- `skill-github-list-prs` — GET /repos/{owner}/{repo}/pulls
- `skill-github-create-issue` — POST /repos/{owner}/{repo}/issues
- `skill-github-add-comment` — POST /repos/{owner}/{repo}/issues/{number}/comments
- `skill-github-get-file` — GET /repos/{owner}/{repo}/contents/{path}?ref={sha} with raw Accept
- `skill-github-search-issues` — GET /search/issues?q=...
- `skill-github-authenticated-user` — GET /user
- `skill-github-create-pr` — POST /repos/{owner}/{repo}/pulls

PythonCode (class 22) — one executor per Tier-0 operation (GET-only, fixed URL shape):
- `pc-github-list-issues` — `result = host.http_fetch(method="GET", url="https://api.github.com/repos/{{vars.slot0}}/{{vars.slot1}}/issues?state=open&per_page=30")`
- `pc-github-list-prs` — similar GET for pulls
- `pc-github-get-authenticated-user` — `result = host.http_fetch(method="GET", url="https://api.github.com/user")`
- `pc-github-search-issues` — `result = host.http_fetch(method="GET", url="https://api.github.com/search/issues?q={{vars.slot0}}")`

Domain Skill (class 2):
- `skill-github` — overview narrative: credential injection, base URL, response envelope, all endpoint patterns, common mistakes

ExtensionCatalogue (class 23):
- `ext-github` — groups all github leaf skills + domain skill + PC executors + recipes

Recipes (class 21):
- `github-list-issues` (Tier 0) — rust: ts-http-fetch, orchestrator: pc-github-list-issues
- `github-list-prs` (Tier 0) — rust: ts-http-fetch, orchestrator: pc-github-list-prs
- `github-get-authenticated-user` (Tier 0) — rust: ts-http-fetch, orchestrator: pc-github-get-authenticated-user
- `github-search-issues` (Tier 1) — LLM constructs query string, then GET via pc-github-search-issues
- `github-create-issue` (Tier 1) — LLM fills title/body, then POST
- `github-create-pr` (Tier 1) — LLM fills head/base/title/body, then POST
- `github-add-comment` (Tier 1) — LLM writes comment body, then POST

---

### `code-review` — paranoid architect review, local or GitHub PR

Two code paths: local (`shell git diff`) and GitHub PR (http to api.github.com). Reads every changed file. LLM performs 6-lens review. Posts findings to GitHub as review comments.

→ **No new Tool rows.** Uses `builtin.shell` + `builtin.http` + `builtin.read_file`.  
→ Heavily LLM-driven (Tier 1 throughout). The key value is the structured prompt context (review instructions) + the pre-steps (load diff, load files).

**New components:**

PythonCode (class 22):
- `pc-git-diff-unstaged` — `result = host.shell(command="git diff")`
- `pc-git-diff-staged` — `result = host.shell(command="git diff --cached")`
- `pc-git-diff-head` — `result = host.shell(command="git diff HEAD~1")`
- `pc-github-get-pr-diff` — `result = host.http_fetch(method="GET", url="https://api.github.com/repos/{{vars.slot0}}/{{vars.slot1}}/pulls/{{vars.slot2}}", headers=[{"name":"Accept","value":"application/vnd.github.v3.diff"}])`
- `pc-github-get-pr-files` — `result = host.http_fetch(method="GET", url="https://api.github.com/repos/{{vars.slot0}}/{{vars.slot1}}/pulls/{{vars.slot2}}/files?per_page=100")`
- `pc-github-post-pr-line-comment` — `result = host.http_fetch(method="POST", url="https://api.github.com/repos/{{vars.slot0}}/{{vars.slot1}}/pulls/{{vars.slot2}}/comments", body={{vars.slot3}})`
- `pc-github-post-pr-issue-comment` — `result = host.http_fetch(method="POST", url="https://api.github.com/repos/{{vars.slot0}}/{{vars.slot1}}/issues/{{vars.slot2}}/comments", body={{vars.slot3}})`

Leaf Skills (class 1):
- `skill-code-review-local` — narrative: run git diff, read each changed file, apply 6-lens review, present findings table
- `skill-code-review-pr` — narrative: fetch PR metadata + diff + files via GitHub API, read each changed file at head_sha, apply 6-lens review
- `skill-code-review-post-comments` — narrative: post line-level and PR-level comments using head_sha, format as severity-tagged markdown

Domain Skill (class 2):
- `skill-code-review` — overview: two modes (local/PR), 6 review lenses, severity scale, output format rules

ExtensionCatalogue (class 23):
- `ext-code-review` — groups the full code-review stack

Recipes (class 21):
- `code-review-local` (Tier 1) — steps: shell git diff (pc-git-diff-unstaged or pc-git-diff-head), read changed files, LLM 6-lens review, present findings
- `code-review-pr` (Tier 1) — steps: http get PR meta, http get PR diff, http get PR files, LLM 6-lens review, present findings, optionally post comments
- `code-review-pr-post-comments` (Tier 1) — step: for each finding, POST line comment or issue comment

Intent examples for `code-review-local`:
"review my changes", "review local changes", "review the diff", "check my code", "paranoid review of current changes", "code review locally"

Intent examples for `code-review-pr`:
"review PR 42", "review owner/repo #42", "review this pull request", "code review github.com/.../pull/42", "review the PR", "check the pull request for issues"

---

### `qa-review` — QA and test coverage analysis

Identifies changed code, finds test files, checks coverage gaps, edge cases, regression risks. Produces a structured test plan.

→ Uses `builtin.shell` (git diff), `builtin.glob` (find test files), `builtin.grep` (find tests for functions), `builtin.read_file`.

**New components:**

PythonCode (class 22):
- `pc-glob-test-files-rust` — `result = host.glob(pattern="**/*_test.rs")` (Tier 0)
- `pc-glob-test-files-ts` — `result = host.glob(pattern="**/*.test.ts")` (Tier 0)
- `pc-glob-test-files-py` — `result = host.glob(pattern="**/*_test.py")` (Tier 0)
- `pc-grep-fn-tests` — `result = host.grep(pattern="{{vars.slot0}}", include="*test*")` (Tier 0)

Leaf Skills (class 1):
- `skill-qa-coverage-analysis` — narrative: identify changed functions, find test files for them, flag uncovered paths
- `skill-qa-edge-cases` — narrative: boundary values, type boundaries, concurrency, state transitions, external failures checklist
- `skill-qa-test-plan` — narrative: generate structured test plan in the canonical format
- `skill-qa-regression-risk` — narrative: identify implicit dependencies, check integration test coverage

Domain Skill (class 2):
- `skill-qa-review` — overview: when to run, methodology, output format, fix-first model

ExtensionCatalogue (class 23):
- `ext-qa-review`

Recipes (class 21):
- `qa-review-local` (Tier 1) — steps: shell git diff stat, glob test files, grep for function tests, LLM coverage analysis, LLM test plan generation
- `qa-review-generate-test-plan` (Tier 1) — steps: read changed files, LLM produces structured test plan markdown

Intent examples:
"qa review", "test coverage analysis", "what tests am I missing", "generate a test plan", "check test coverage", "review test quality", "edge case review", "QA check", "regression test review", "find missing tests"

---

### `security-review` — OWASP security audit

Reviews code for injection, auth/authz, data exposure, crypto, supply chain, secrets. Produces severity-categorized findings with concrete fixes. Tracks findings as signals.

→ Uses `builtin.shell` (git diff), `builtin.grep` (scan for patterns), `builtin.read_file`.

**New components:**

PythonCode (class 22):
- `pc-grep-hardcoded-secrets` — `result = host.grep(pattern="(?i)(password|api_key|secret|token|credential)\\s*[:=]\\s*['\"][^'\"]{8,}")` (Tier 0)
- `pc-grep-injection-patterns` — `result = host.grep(pattern="(?i)(eval\\(|exec\\(|subprocess|shell_exec|format!.*sql|query.*format)")` (Tier 0)
- `pc-grep-env-files` — `result = host.glob(pattern="**/.env*")` (Tier 0)

Leaf Skills (class 1):
- `skill-security-injection` — narrative: trace user input to DB/shell/template, check parameterized queries, scan for interpolation patterns
- `skill-security-auth` — narrative: session tokens, password hashing, IDOR, API key exposure
- `skill-security-data-exposure` — narrative: error messages, logging, API over-fetching, CORS
- `skill-security-crypto` — narrative: TLS, encryption algorithms, key management, RNG
- `skill-security-supply-chain` — narrative: CVE check for new deps, lock files, build pipeline
- `skill-security-secrets` — narrative: grep patterns, .env gitignore, log scanning

Domain Skill (class 2):
- `skill-security-review` — overview: categories, output format, fix-first model, tracking pattern

ExtensionCatalogue (class 23):
- `ext-security-review`

Recipes (class 21):
- `security-review-scan` (Tier 0) — steps: pc-grep-hardcoded-secrets, pc-grep-injection-patterns, pc-grep-env-files; returns raw scan results
- `security-review-full` (Tier 1) — steps: shell git diff, scan PCs, read changed files, LLM 6-category analysis, produce findings output
- `security-review-local` (Tier 1) — variant of full for local working tree

Intent examples:
"security review", "security audit", "check for vulnerabilities", "is this code secure", "OWASP check", "check for injection", "security check", "scan for secrets", "find vulnerabilities", "CVE check new dependencies"

---

### `plan-mode` — structured task planning

Creates plans as memory docs, tracks step progress, executes steps sequentially. References `memory_search`, `memory_write`, `memory_read`.

NOTE: `plan-mode` references `mission_create` and `mission_fire` which are v1-only tools that do NOT exist in v3. The mission/keeper machinery is out of scope. The v3 replacement is: plan = memory doc, execution = orchestrated recipe steps tracked via `memory_write` patch.

→ Uses `builtin.memory_search`, `builtin.memory_write`, `builtin.memory_read`.

**New components:**

PythonCode (class 22):
- `pc-plan-create` — formats a plan document body from slot vars (title, goal, steps list) and calls memory_write; pure formatter + one `host.memory_write(...)` call
- `pc-plan-read` — `result = host.memory_read(path="plans/{{vars.slot0}}.md")`
- `pc-plan-search` — `result = host.memory_search(query="plan {{vars.slot0}}")`
- `pc-plan-status-update` — reads plan from memory, updates a step marker ([ ] → [x]), writes back; this is two operations so it must be split into two recipe steps: one read + one write (step isolation invariant)

Leaf Skills (class 1):
- `skill-plan-create` — narrative: format plan doc with slug, goal, success criteria, steps list, risks; write to plans/<slug>.md
- `skill-plan-track-progress` — narrative: read plan, mark current step done, mark next step in-progress, write back
- `skill-plan-list` — narrative: memory_search for "plan " prefix, list plans with status
- `skill-plan-revise` — narrative: read plan, apply feedback, reset failed steps to pending, rewrite

Domain Skill (class 2):
- `skill-plan-mode` — overview: when to create a plan, plan format, step tracking convention, execution discipline

ExtensionCatalogue (class 23):
- `ext-plan-mode`

Recipes (class 21):
- `plan-create` (Tier 1) — LLM decomposes task into steps, then pc-plan-create writes to memory
- `plan-read` (Tier 0) — pc-plan-read fetches plan doc
- `plan-list` (Tier 0) — pc-plan-search returns matching plan docs
- `plan-update-step` (Tier 1) — read plan (Tier 0 step), LLM identifies step to update, write updated plan
- `plan-revise` (Tier 1) — read plan, LLM applies revision, write back

Intent examples for `plan-create`:
"create a plan", "make a plan", "plan mode", "[PLAN MODE] create", "plan out how to do this", "execution plan for this task", "step by step plan", "plan before executing"

Intent examples for `plan-list`:
"list my plans", "show plans", "[PLAN MODE] list all plans", "what plans do I have"

---

### `web-browse` — Playwright MCP browser (**BLOCKED — no first-party browser tool in v3**)

Uses `browser_navigate`, `browser_get_text`, `browser_screenshot`, `browser_click`, `browser_type` —
all from the Playwright MCP server. In v3, **MCP tool binding is a separate future phase** (no
`builtin.browser` Tool exists, and the MCP bridge that would expose Playwright as first-class host
callables is not implemented yet).

**Decision: BLOCKED/DEFERRED.** Author component specs here as a design record; do NOT seed in
`builtin_bootstrap.rs` until the browser capability is implemented as a first-party `builtin.browser`
Tool (or the MCP bridge phase lands). The v1 SKILL.md content can remain on disk for reference.

What the v3 stack would look like once unblocked:
- Tool (class 0): `builtin.browser` with operations `navigate`, `get_text`, `screenshot`, `click`, `type`, `select`
- ToolSkill rows: `ts-browser-navigate`, `ts-browser-get-text`, `ts-browser-screenshot`, `ts-browser-click`, `ts-browser-type`
- PythonCode executors: one per operation (§shell-safe-fixed style, Tier 0 for navigate/screenshot/get_text)
- Leaf Skills: `skill-browser-navigate`, `skill-browser-extract-text`, `skill-browser-interact`, `skill-browser-screenshot`, `skill-browser-search`
- Domain Skill: `skill-web-browse`
- Recipes: `browser-navigate` (Tier 0), `browser-get-text` (Tier 0), `browser-screenshot` (Tier 0), `browser-search-web` (Tier 1), `browser-interactive-session` (Tier 1)
- ExtensionCatalogue: `ext-web-browse`

---

### `portfolio` — DeFi portfolio + NEAR Intents

Uses a custom `portfolio` Tool (class 0) that is NOT yet in v3. It has operations: `scan`, `propose`, `build_intent`, `format_widget`, `progress`. Also uses `memory_write`, `memory_read`, `time`.

→ **Requires a new Tool row + ToolSkills for the `portfolio` Tool.**  
→ The `portfolio` tool is domain-specific: it calls Dune Sim + FastNEAR+Intear backends.
  It must be implemented as a first-party Rust tool in `crates/brassclaw_host_runtime/first_party_tools/`.
→ This is a **significant new Rust capability** — NOT a trivial seeding task. The tool
  implementation is **out of scope for Phase P.1** (which is only about migrating existing
  skills to DB rows). The portfolio skill seeding is **blocked on the portfolio tool implementation**.

**Decision: author the full v3 component spec for portfolio but mark it as BLOCKED/DEFERRED
until the `portfolio` Rust Tool is implemented. Do NOT attempt to seed it in builtin_bootstrap.rs
until the Rust backing exists.**

For Phase P.1 purposes: author the component specs in this subplan, add a
`plans/portfolio_tool_implementation.md` stub, and proceed with the other 8 skills.

---

## Seeding strategy for `builtin_bootstrap.rs`

All 8 unblocked skills (excluding portfolio) are seeded as new groups in `builtin_bootstrap.rs`:

### Pass 8 — `coding` domain skill + ext-coding catalogue
Only a domain Skill (class 2) + ExtensionCatalogue (class 23). Extremely lightweight.

### Pass 9 — `commit` workflow (new recipe)
New: `skill-commit-workflow` (class 2) + `commit-workflow` Recipe (class 21, Tier 1).
Uses existing `shell-git-*` PCs already seeded in Pass 4.

### Pass 10 — `github` full stack
New: 10 leaf Skills + 4 PythonCode executors + `skill-github` domain Skill + 7 Recipes + `ext-github` catalogue.

### Pass 11 — `code-review` stack
New: 7 PythonCode + 3 leaf Skills + `skill-code-review` domain Skill + 3 Recipes + `ext-code-review` catalogue.
Reuses: github PCs/skills from Pass 10; shell/http PCs from Passes 1+2+4.

### Pass 12 — `qa-review` stack
New: 4 PythonCode + 4 leaf Skills + `skill-qa-review` domain Skill + 2 Recipes + `ext-qa-review` catalogue.

### Pass 13 — `security-review` stack
New: 3 PythonCode + 6 leaf Skills + `skill-security-review` domain Skill + 3 Recipes + `ext-security-review` catalogue.

### Pass 14 — `plan-mode` stack
New: 4 PythonCode + 4 leaf Skills + `skill-plan-mode` domain Skill + 5 Recipes + `ext-plan-mode` catalogue.
Reuses: memory_write/read/search PCs from Pass 3.

### Pass 15 — `web-browse` — BLOCKED
Deferred until `builtin.browser` first-party Tool is implemented (no MCP bridge in v3 yet).

---

## Infrastructure removal steps (after all 8 skills seeded)

### Step 3 — Remove `embed_migrated_skills_catalog()` from `build.rs`
Dead output: `migrated_skills_catalog.json` is produced but never consumed
(the `crate::migrated_skills` module referenced in the comment does not exist).

Remove: `MIGRATED_SKILL_NAMES`, `MIGRATED_SKILLS_CATALOG_PATH`, `embed_migrated_skills_catalog()`
call + fn, and the `fs::write(out_dir.join(MIGRATED_SKILLS_CATALOG_PATH), "[]")?;` in the
skills_db fast path.

### Step 4 — Remove `embed_reborn_skills()` and clean up `build.rs`
Remove: `embed_reborn_skills()` fn + its call + the `println!("cargo:rerun-if-changed=...")` for
`skills_dir`/`archive_skills_dir`. Remove the empty `embedded_reborn_skill_*.json` stub writes from
the skills_db fast path. If `main()` is now empty, remove the entire `build.rs`.

Helper fns to remove only if exclusively used by `embed_reborn_skills()`:
`collect_skill_files`, `collect_files_recursive`, `skill_file_json`, `path_is_real_dir`,
`path_is_real_file`, `non_symlink_file_type`.

### Step 5 — Delete `bundled_skills.rs` and all call sites
- Delete `crates/brassclaw_reborn_composition/src/bundled_skills.rs`
- `lib.rs`: remove `#[cfg(not(feature = "skills-db"))] mod bundled_skills;`
- `factory.rs:786`: remove the `#[cfg(not(feature = "skills-db"))] crate::bundled_skills::ensure_bundled_reborn_skills_installed(&root).await?;` block
- `skill_listing.rs`: remove the `#[cfg(not(feature = "skills-db"))]` block (lines ~30-50) + the
  test `local_skill_list_prefers_embedded_bundled_summary_over_storage_system_skill` + helpers only used by that test

### Step 6 — Remove `SkillSource::System` disk-load from `management.rs`
- Remove `const SYSTEM_SKILLS_ROOT` (line 34)
- Remove `list_skill_root(context, SYSTEM_SKILLS_ROOT, SkillSource::System)` from `list_skills()`
- Remove the first `collect_matching_skill_root(context, SYSTEM_SKILLS_ROOT, SkillSource::System, ...)` from `search_skills()`
- Check if `SkillSource::System` variant is still used elsewhere; keep variant if so, remove only the disk-load sites

### Step 7 — Full build + clippy
```bash
df -h /Users/ollama/brassclaw-target
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo build --release --bin brassclaw
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo clippy --all --all-features -- -D warnings
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo test -p brassclaw_reborn_composition --features skills-db,postgres
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo test -p brassclaw_skills
```

### Step 8 — Update `saved_plan_to_v3.md` Phase P.1 → `[x] Done`

---

## Portfolio tool stub

File: `plans/portfolio_tool_implementation.md` (gitignored — one-shot plan)

The `portfolio` tool requires a new Rust first-party Tool in
`crates/brassclaw_host_runtime/first_party_tools/portfolio.rs` with operations:
`scan`, `propose`, `build_intent`, `format_widget`, `progress`.

Backends: Dune Sim API (EVM addresses), FastNEAR + Intear API (NEAR addresses).

This is a significant new capability. Out of scope for Phase P.1. Track as a separate
phase (candidate: Phase P.2 or a new Phase W — domain extensions).
