# Subplan — H.5 O2.5 delete the legacy `brassclaw_gateway` crate

Parent: `subplan_problem_stepH5_obsolescence_of_saved_plan_to_v3.md` (Phase H.5
obsolescence reconciliation, surfaced from the O2.4 follow-up).
Parent plan: `saved_plan_to_v3.md` (Recipe System Finalisation Plan — v3), Phase H.5.
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Inserted as a **sub-substep** under the Zenflow Phase H.5 substep
`2ae5b518-4984-40ab-91c9-479dda6f88da` (after it, before resuming O3).

This subplan was opened because the O2.4 follow-up (removing the dead
`MissionThreadSpawned` debug-panel handler + 3 orphaned i18n keys inside
`crassclaw_gateway`) surfaced a much larger pre-existing dead surface: the entire
`brassclaw_gateway` crate is a **legacy v1 debug-panel UI** with **zero dependents** —
it is not compiled into the `brassclaw` binary (no crate's `[dependencies]` lists it;
root `[dependencies]` does not mention it), only pulled in by `--workspace` checks.
`brassclaw_webui_v2` is the current UI. The mission backend was removed in O2.2/O2.3,
so the gateway's Missions Tab (and the rest of its surfaces) call dead endpoints — but
the crate is not served and the Rust compile is just `include_str!` embedding, so it
does not break `--workspace` checks today.

The user chose **option 2 — delete the entire `brassclaw_gateway` crate now** (legacy v1,
zero dependents; this also fixes the duplicate workspace-member entry). This subplan
executes that deletion completely: the crate, its gateway-only tooling/scripts, its
gateway-only CI job, its gateway-only e2e test, and all doc/forbidden-list references.

---

## 1. Grounding — the full `brassclaw_gateway` footprint

A workspace-wide grep for `brassclaw_gateway` (and `brassclaw_gateway/static`) found the
crate has **zero runtime/compile dependents** and a bounded tooling/doc/test surface.

### 1.1 The crate itself (delete entirely)
- `crates/brassclaw_gateway/` — `Cargo.toml`, `src/{lib,bundle,layout,widget,assets}.rs`,
  `AGENTS.md`, `static/` (index.html, js/, styles/, i18n/, admin/, debug-*.js, theme-init.js).

### 1.2 Workspace manifest (root `Cargo.toml`)
- Line 3 `members = [...]` lists `"crates/brassclaw_gateway"` **TWICE** (a pre-existing
  duplicate anomaly — both removed).
- Line 150 `[workspace.dependencies]` declares `brassclaw_gateway = { path = ... }` —
  unused (no member `[dependencies]` consumes it); removed.
- Root package `[dependencies]` does **not** mention `brassclaw_gateway` (confirmed).

### 1.3 Release tooling
- `release-plz.toml` `[[package]] name = "brassclaw_gateway"` (lines 25–28) — removed.

### 1.4 Boundary-test dead config (forbidden lists — vacuous once the crate is gone)
- `crates/brassclaw_architecture/tests/reborn_dependency_boundaries.rs` — 13
  `"brassclaw_gateway",` entries inside `forbidden: vec![...]` lists. These are
  "crate X must not depend on brassclaw_gateway" rules. The test
  `reborn_boundary_rules_active_crates_are_workspace_members` (line 10) only validates
  rule **subjects** are members (not `forbidden` entries) and tolerates absent crates,
  so leaving them is vacuous; removing them is the clean choice (less bloat).
- `crates/brassclaw_product_adapters/tests/product_adapter_contract.rs:43` —
  `"brassclaw_gateway",` in `FORBIDDEN_DEPENDENCIES` — removed.
- No exhaustive workspace-members assertion exists (only the subset check at line 10),
  so removing the member cannot break a membership test.

### 1.5 Gateway-only scripts (delete / trim)
- `scripts/check-i18n-parity.sh` — **entirely** gateway-specific (hardcoded
  `I18N_DIR="$REPO_ROOT/crates/brassclaw_gateway/static/i18n"`). Deleted. (webui_v2 i18n
  was never covered by this script; a future webui_v2 parity check is a separate
  enhancement, not in scope here.)
- `scripts/pre-commit-safety.sh` — has gateway-specific blocks: the i18n-parity trigger
  (lines 56–89) + the gateway-JS `node --check` block (lines 91–130) + the
  `GATEWAY_APP_JS_TMP` var (31) + trap refs (107, 154) + comment (19–20). These blocks
  become dead once the crate is gone. Removed; the general `.rs` safety checks stay.
  Note: check #7 ("Gateway/CLI handlers bypassing ToolDispatcher", scans
  `src/channels/web/handlers/`) refers to the v1 root `src/` web channel, NOT the
  `brassclaw_gateway` crate — left untouched (separate concern).

### 1.6 Gateway-only CI (delete job + aggregator refs)
- `.github/workflows/code_style.yml` `gateway-js-syntax` job (lines 112–129) — runs
  `find crates/brassclaw_gateway/static/js -name '*.js' -exec node --check`. Deleted.
- The `code-style` aggregator job (254–301) lists `gateway-js-syntax` in `needs:`
  (262) and in the result loop (278). Deleting the job **forces** removing those two
  refs (else the workflow references a non-existent job). Removed.
- **Out of scope (flagged, NOT touched):** the `gateway-boundaries` job (236–252) runs
  `scripts/check_gateway_boundaries.py`, which walks `src/channels/web/platform/` —
  the **v1 root `src/` web-gateway layer**, NOT the `brassclaw_gateway` crate. That dir
  was removed in Phase 6 (`src/` is now only the `main.rs` shim), so this job is a
  **separate Phase-6-dead surface**. It is left intact here; flagged in §4 for a
  separate user decision. Its aggregator refs (268, 281) + `changes` path-filter refs
  (57, 64) are left untouched.

### 1.7 Gateway-only e2e test + mock scenarios (delete)
- `tests/e2e/scenarios/test_widget_customization.py` — already module-level
  `pytest.skip` ("Disabled during legacy CI cleanup: v1-era scenario not confirmed
  against Reborn"). Drives the gateway UI (`.system/gateway/*`, `#app`/`.tab-bar` DOM
  from `crates/brassclaw_gateway/static/index.html`). Deleted.
- `tests/e2e/mock_llm.py` customization scenarios (lines 633–771: the
  `# ---- Frontend customization via chat ----` comment + 2 `TOOL_CALL_PATTERNS`
  tuples "customize: move tab bar to left" + "customize: install skills viewer widget")
  — consumed **only** by `test_widget_customization.py` (grep confirms no other e2e
  test sends `customize:` triggers). Removed; the `TOOL_CALL_PATTERNS` list closes at
  the `]` on the following line.

### 1.8 Doc references (update current-architecture docs; leave historical archives)
Update (state removed): `crates/AGENTS.md` (table row 141 + UI-presentation line 159),
`crates/README.md` (table row 93 + UI line 116), `crates/brassclaw_webui_v2/CLAUDE.md`
(forbidden-deps prose line 164), `crates/brassclaw_product_workflow/CLAUDE.md`
("Must NOT depend on" prose line 47), `docs/brassclaw-architecture.md`
(`### brassclaw_gateway` section 550–552 + tree entry 1336).
Leave as historical archives (do not rewrite history): `CHANGELOG.md`,
`docs/plans/2026-03-22-crate-extraction-and-cleanup.md`,
`docs/plans/2026-05-22-reborn-budgets-followups.md`,
`docs/superpowers/specs/2026-04-23-projects-tab-control-room-design.md` (a spec for the
now-deleted gateway projects tab — historical design context; left as archive).

### 1.9 Local-only tooling index (gitignored — local hygiene, not committed)
`.sweepfix/codebase.toml` has ~40 `crates/brassclaw_gateway/...` entries + the
`test_widget_customization.py` entry. The file is gitignored (confirmed via
`git check-ignore`), so editing it is local-only and will not be in the commit. Cleaned
locally for hygiene.

### 1.10 `Cargo.lock`
Regenerated automatically by `cargo check` after the member is removed.

---

## 2. Steps (executed one after another)

**G1** — `git rm -r crates/brassclaw_gateway/` (staged deletion of the whole crate).

**G2** — root `Cargo.toml`: remove both `"crates/brassclaw_gateway"` entries from the
`members` array (line 3) AND the `brassclaw_gateway = { path = ... }` line (150) from
`[workspace.dependencies]`.

**G3** — `release-plz.toml`: remove the `[[package]] name = "brassclaw_gateway"` block
(lines 25–28).

**G4** — `crates/brassclaw_architecture/tests/reborn_dependency_boundaries.rs`: remove
all 13 `"brassclaw_gateway",` forbidden-list lines.

**G5** — `crates/brassclaw_product_adapters/tests/product_adapter_contract.rs`: remove
the `"brassclaw_gateway",` line (43) from `FORBIDDEN_DEPENDENCIES`.

**G6** — `git rm scripts/check-i18n-parity.sh` (gateway-only).

**G7** — `scripts/pre-commit-safety.sh`: remove the gateway i18n-parity block (56–89),
the gateway-JS-syntax block (91–130), the `GATEWAY_APP_JS_TMP` var declaration (31),
its trap refs (107, 154 → drop the var from the trap, keep `TEST_BOUNDARIES_FILE`),
and the comment refs (19–20). Leave check #7 + all general `.rs` checks intact.

**G8** — `git rm tests/e2e/scenarios/test_widget_customization.py`; remove the
customization scenarios (lines 633–771) from `tests/e2e/mock_llm.py`.

**G9** — `.github/workflows/code_style.yml`: delete the `gateway-js-syntax` job
(112–129); remove `- gateway-js-syntax` (262) from the `code-style` `needs:` and
`"gateway-js-syntax=..."` (278) from its result loop. Leave `gateway-boundaries`
intact (out of scope — see §1.6/§4).

**G10** — doc updates per §1.8.

**G11** — verify (see §3).

**G12** — selective-stage (exclude the user's concurrent prefix-cache WIP — see §5) +
commit + push; mark this subplan doc DONE + the Zenflow sub-substep Completed; resume
H.5 at O3.

---

## 3. Verification

- `df -h /Users/ollama/brassclaw-target` first (clean if Avail < 15GB / Capacity > 90%).
- `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo metadata --no-deps` succeeds
  (manifest still valid after member removal).
- `cargo check --workspace --all-targets` (NVMe target) — the deleted crate is gone;
  the user's `system_bundle_source`/prefix-cache WIP blockage is pre-existing and
  absent in a clean index, so verify on the selectively-staged index at commit time.
- `cargo clippy -p brassclaw_architecture -p brassclaw_product_adapters --all-targets
  -- -D warnings` (touched test crates) clean.
- `cargo test -p brassclaw_architecture -p brassclaw_product_adapters` GREEN
  (boundary/contract tests still pass with the dead forbidden entries removed).
- `python3 -m py_compile tests/e2e/mock_llm.py` clean (customization section removed
  cleanly, `TOOL_CALL_PATTERNS` list still closes).
- `bash -n scripts/pre-commit-safety.sh` (syntax check after trimming gateway blocks).
- `git grep -n brassclaw_gateway` → only historical archive docs + this subplan doc +
  the flagged `gateway-boundaries`/`check_gateway_boundaries.py` (v1 src/ web-gateway,
  out of scope) remain.

---

## 4. Surfaced follow-ups (needs user design decision — NOT decided here)

1. **`gateway-boundaries` CI job + `scripts/check_gateway_boundaries.py`** — dead since
   Phase 6 removed `src/channels/web/platform/` (the v1 root `src/` web-gateway layer;
   `src/` is now only the `main.rs` shim). This is a **separate** Phase-6-cleanup
   decision (delete the job + script + its `changes` path-filter refs + aggregator
   refs), not part of the `brassclaw_gateway` crate deletion. Flagged for the user.
2. **`brassclaw_tui`** — the crate dir exists (`crates/brassclaw_tui/`) but is **not**
   in the workspace `members` list, yet is referenced in `crates/AGENTS.md`,
   `crates/README.md`, `release-plz.toml`, and forbidden-lists. A separate
   register-vs-deregister anomaly. Flagged for the user.

---

## 5. Commit-staging note (user concurrent WIP)

The working tree carries the user's separate prefix-cache/basic-prompt-store WIP
(`system_bundle_source` on `DefaultPlannedRuntimeParts`, `pg_basic_prompt_store.rs`,
`prefix-tab.js`/`usePrefixes.js`, `prefix_V3.md`, and a set of tracked-unstaged files
across `brassclaw_loop_support`/`brassclaw_product_workflow`/`brassclaw_reborn`/
`brassclaw_reborn_composition`/`brassclaw_webui_v2*` + `saved_plan_to_v3.md`). This
subplan's edits do NOT overlap that WIP set (gateway crate / root Cargo.toml members+
deps / release-plz / architecture+product_adapter tests / 2 scripts / code_style.yml /
mock_llm.py / 5 doc files). The `git rm` deletions are staged explicitly; modified
files are `git add`-ed explicitly by path. `saved_plan_to_v3.md` (user WIP) gets only
the one-line subplan-ref hunk via selective staging (`git add -p`/patch filter) so the
user's unstaged WIP in that file is NOT committed. `.sweepfix/codebase.toml` is
gitignored → local-only, not committed.

---

## 6. DONE record

(filled in on completion)
