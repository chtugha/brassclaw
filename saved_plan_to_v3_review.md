# `saved_plan_to_v3.md` — Pre-Implementation Review

> **Reviewed artifact:** `./saved_plan_to_v3.md` (Recipe System Finalisation — v3, now 4302
> lines — grew from 3924 as resolutions were applied: Phase E.0 + Phase H.0 subsections were
> added, and every review-note block was expanded with a ✅ RESOLVED explanation)
> **Method:** every adjacency/security/correctness claim was checked against the *current*
> codebase (file + line). Each finding was written back into the plan as a
> `> **Review note (pre-v3 audit):** …` block at the relevant section, so the plan now
> carries its own audit. This file is the consolidated, severity-ordered index of those notes.
> **Scope of the review:** started read-only against code; per the user's "address all the
> issues" instruction the constraint was then reversed and findings were driven to a
> code-verified resolution in the plan. `saved_plan_to_v3.md` and this index were edited; **one
> Rust code file was also modified** — the H2 hardcoded-scope bug
> (`./crates/brassclaw_engine/src/executor/orchestrator.rs:2573-2591`) was fixed in a prior
> session and is **verified** (`cargo clippy -p brassclaw_engine --all-targets -- -D warnings`
> clean; `cargo test -p brassclaw_engine --lib` → 610 passed, 0 failed). No `git commit` was
> run (per the standing constraint).
> **Status:** **all 18 notes are RESOLVED in the plan.** C1–C4 (CRITICAL) and H1–H5 (HIGH)
> each have an applied, code-verified resolution written back into the plan (Phase E.0 wires
> `PostgresSource`; Phase H.0 adds the `LoopRetrievalPort` + `resolve_message_text` host ports;
> Phase A store-round-trip; Phase F fallback-preservation; Phase K.3 wire-then-delete split;
> Phase J.1 reword; §2 C1 third-path resolution). M1–M5 (MEDIUM) are resolved/struck-through
> in-body (M3 line-cite corrected, M4 FINDING D marked superseded, M5 positional-re-index
> note added). L1–L4 (LOW) are marked OBSOLETE in the plan (`doc_type_weight_by_class` is
> gone; the `22=>0.42`/`23=>0.38` arms and their tests are struck through). This document is
> the summary; the authoritative resolutions live in `saved_plan_to_v3.md`.

---

## 1. Executive Summary

The v3 plan is architecturally coherent and its per-phase designs are largely correct
against the current code. The review surfaced **18 findings**, of which **4 are
CRITICAL** (would break production or block implementation as written), **5 HIGH**
(correctness/isolation bugs or shape mismatches that must be fixed), **5 MEDIUM**
(stale/wrong premises or precision issues), and **4 LOW** (already-resolved historical
items that the plan still references as live work).

The dominant theme is a **single cross-cutting ordering hazard: `PostgresSource` is not
wired in production today** (`./crates/brassclaw_engine/src/runtime/manager.rs:383` wires
`RamSource`). Phases E, F, G, H, and K all assume the intent-driven `fetch_for_turn` path
is live; it is dormant. Several phases are only safe if `PostgresSource` wiring is treated
as a **prerequisite** (done early and verified) rather than a Phase K afterthought.
Treating wiring as a Phase K item — and deleting `RamSource` in the same phase — leaves
the engine with no retrieval backend between the deletion and the wiring.

The second theme is a **migration-ordering contradiction** in §2: new component classes 22
and 23 (V051/V052, Phases B/C) are told to use the central `reborn_validation_queue`
"from day one", but that table is created in V058/Phase N — ~12 phases later. Between
those migrations the new classes have neither per-table Q1/Q2 columns nor the central
queue, so the §0.5 snippet→PythonCode promotion flow is unreachable.

One plan finding (Phase N "FINDING D") is **factually wrong** and would cause an
implementer to "fix" a correct trigger and risk introducing a real bug.

### Findings by severity

> **All 18 findings below are RESOLVED in `saved_plan_to_v3.md`.** CRITICAL (C1–C4) and
> HIGH (H1–H5) have applied, code-verified resolutions (see §2/§3 cross-cutting sections
> and the per-phase blocks in §5); MEDIUM (M1–M5) are corrected/struck-through in-body
> (§4); LOW (L1–L4) are marked OBSOLETE (§4.2). The one code fix (H2) is applied + verified.

| Sev | # | Phase/§ | One-line |
|-----|---|---------|----------|
| CRITICAL | C1 | §2 (line 3657) | V058 validation queue lands ~12 phases after V051/V052; classes 22/23 have no Q1/Q2 path meanwhile; snippet promotion dead until Phase N |
| CRITICAL | C2 | Phase E (line 2263) | `PostgresSource` not wired; Phase E code is correct but dormant. Wiring is an E/H prerequisite, not a Phase K item |
| CRITICAL | C3 | Phase F (line 2374) | Legacy `retrieve_context` fallback (`orchestrator.rs:2620-2637`) is the *actual* production retrieval path; Phase F is silent on it — dropping it breaks all retrieval pre-wiring |
| CRITICAL | C4 | Phase K.3 (line 3024) | Do not delete `RamSource` until `PostgresSource` is wired; split K.3 into wire-then-delete sub-steps |
| HIGH | H1 | Phase A (line 1995) | Engine `Recipe` struct ≠ runtime `RecipeMatchDto`; `step_descriptions` store round-trip (`PgRecipe`/`RECIPE_SELECT`/`decode_recipe_row`/`NewPgRecipe`) missing from Phase A file list |
| HIGH | H2 | Phase F (line 2360) | Hardcoded `tenant_id:"default"`/`agent_id:""` scope bug; `Thread` has no `tenant_id`/`agent_id` fields — fix is larger than implied (multi-tenant isolation) |
| HIGH | H3 | Phase H (line 2514) | COMP-01 resolved: `LoopInput::UserMessage` holds an opaque `message_ref`, not text — `consume_drainable_inputs` needs a host fetch to populate `last_user_text` |
| HIGH | H4 | Phase H (line 2564) | `RecipeStage` (in `brassclaw_agent_loop`) cannot import `PostgresSource` (lives in `brassclaw_engine`); must reach `fetch_for_turn` via a host port — specify/add it |
| HIGH | H5 | Phase J.1 (line 2858) | V054 `intent_examples` add is a NO-OP (already in V027 as `{input,class}`); `SkillManifest` `Vec<String>` is shape-incompatible; the real missing wiring is `auto_passed → seed_intent_input` |
| MEDIUM | M1 | §0.3 (line 139) | Intent-driven retrieval dormant in production (RamSource wired, PostgresSource not) — ordering consequence for H/K |
| MEDIUM | M2 | §0.8 (line 738) | `PostgresSource` correct but not the live backend; `IntentResolution::Match` has no `step_link` today (confirms Phase D greenfield) |
| MEDIUM | M3 | §0.9 (line 963) | FINDING F verified: `formatted_content` is shape-polymorphic (JSON at ~2706-2710, prose override at ~2677); plan's "2674" cite is imprecise |
| MEDIUM | M4 | Phase N / FINDING D (line 3448) | STALE and factually wrong: `upsert` IS `INSERT…ON CONFLICT`; N.1 trigger already uses that form; V034 schema supports it — do NOT "fix" |
| MEDIUM | M5 | Phase N.4 (line 3584) | Struct audit verified; `decode_recipe_row` is positional, so column drops require re-indexing every later `row.get(N)` |
| LOW | L1 | §0.8 (line 1078) | `doc_type_weight_by_class` removed entirely (Goal-pre-v3 Step 12) — no weight arms to extend |
| LOW | L2 | Phase B (line 2062) | `22 => 0.42` arm obsolete (function gone) |
| LOW | L3 | Phase C (line 2118) | `23 => 0.38` arm obsolete (function gone) |
| LOW | L4 | Phase K (line 3013) | `doc_type_weight_by_class` already deleted; only `extract_keywords`/`keyword_match_score` + `RamSource` + `retrieval_dbless.rs` remain for K |

---

## 2. Critical Cross-Cutting Hazard — `PostgresSource` Wiring Spans E/F/G/H/K

This is the single most important result of the audit. Verified fact:

- `./crates/brassclaw_engine/src/runtime/manager.rs:383` constructs `RamSource` and passes it
  to `with_retrieval_source(...)`. `PostgresSource` is **not** constructed/wired anywhere in
  the composition path. (A `TODO(Phase K)` marker was added by `Goals_pre_v3_review.md`
  Step 8.)
- `./crates/brassclaw_engine/src/executor/orchestrator.rs:2552-2637`
  `handle_assemble_prior_knowledge` takes `retrieval_source: Option<&Arc<dyn RetrievalSource>>`
  and calls `source.fetch_for_turn(...)` (line 2582) **only if** the source is `Some`.
  When it is `None` or `fetch_for_turn` errors, control falls through to the legacy
  `retrieval.retrieve_context(...)` block at lines 2620-2637 — the `RetrievalEngine` /
  MemoryDoc path. **That fallback is the actual production retrieval path today.**

Consequences the plan must reconcile:

1. **Phase E** (line 2263): all edits target `PostgresSource::fetch_for_turn`. The
   `SplitResult`/`ActionShortCircuit` variants and the IBS call are dormant at deploy time —
   unit/integration tests pass but no live turn takes the path until the composition layer
   calls `with_retrieval_source(PostgresSource)`. **Treat wiring as an E/H prerequisite.**
2. **Phase F** (line 2374): the plan says the handler "already calls `fetch_for_turn` via
   `PostgresSource`". That is imprecise — it calls the `RetrievalSource` trait object, which
   is `RamSource`/`None` in production. Phase F must **preserve the `retrieve_context`
   fallback unchanged**; an implementer restructuring around the four
   `FetchForTurnResult` variants could delete it and break all retrieval before wiring.
   Removal belongs in Phase K, alongside `handle_retrieve_docs`.
3. **Phase H** (line 2564): `RecipeStage` lives in `brassclaw_agent_loop`, which does **not**
   depend on `brassclaw_engine` (where `RetrievalSource`/`PostgresSource` live). The
   retrieval source is exposed to stages via the host port (`ctx.host.…`), not a direct
   import. Phase H must specify/add the host-port method that exposes `fetch_for_turn`.
4. **Phase K.3** (line 3024): deleting `RamSource` before wiring `PostgresSource` leaves
   the engine with **no** retrieval backend — every turn's `__assemble_prior_knowledge__`
   returns empty. K.3 must be split: (1) wire `PostgresSource` into `manager.rs`, ship and
   verify live turns take the intent path; (2) *then* delete `RamSource` +
   `retrieval_dbless.rs`. This matches `Goals_pre_v3_review.md` Step 14's ordering
   constraint.

**Net recommendation:** insert a **"Phase E.0 — wire `PostgresSource` in composition and
verify a live turn takes `fetch_for_turn`"** before any phase that consumes its variants.
Phases E, F, G, H then operate on a live path; Phase K becomes pure deletion. This is the
cleanest resolution of C2/C3/C4 and the transitive dependency in H4.

> **✅ ADOPTED.** Phase E.0 ("Wire `PostgresSource` in Composition") was added to
> `saved_plan_to_v3.md` before Phase E, with the `ThreadManager::with_retrieval_source`
> builder + composition injection of `PostgresSource::new(pg_pool)` + no-pool→hard-error
> acceptance criteria. C2/C3/C4 + the H4 transitive dependency are resolved by it; Phase K.3
> is now pure deletion (wire-then-delete split). The C2/C3/C4 review notes in the plan each
> carry a ✅ RESOLVED marker pointing to E.0.

---

## 3. Critical — §2 Migration Ordering (Validation Queue vs. Classes 22/23) — C1

Plan line 3657 (§2 review note). The plan tells Phase B (V051 `reborn_python_code`,
line ~2048) and Phase C (V052 `reborn_extension_catalogues`, line ~2105) **not** to
include per-table `queue_code`/`review_attempts`/`review_feedback`/`rejected_at`/
`validation_errors` columns, because they "use `reborn_validation_queue` from day one
(§0.18, Phase N.4)". But `reborn_validation_queue` is created in **V058 / Phase N** —
roughly 12 phases *after* V051/V052. "From day one" is self-contradictory.

- Between V051 and V058, classes 22 and 23 have **neither** per-table Q1/Q2 columns **nor**
  the central queue. The §0.5 snippet→PythonCode promotion flow ("on WebUI save → creates
  a PythonCode component (class 22), enters Q1 queue") is **unreachable** — a WebUI-authored
  PythonCode can never pass Q1/Q2, so it can never be promoted from `type:"snippet"` to
  `type:"component"` (the IBS refuses un-promoted snippets → `IbsError::UnpromotedSnippet`).
  Phase B's own core WebUI-save path is dead until Phase N.
- V058's populate (line ~3357 "13 component tables"), drop (line ~3392), and the
  boot-integrity UNION ALL (line ~3593) all say "13" / "each table" without listing classes
  22/23. Since V051/V052 add no per-table columns there is nothing to DROP (consistent), but
  the POPULATE / boot-integrity INSERT *must* UNION ALL over the two new tables too, or
  their components never enter the queue even after V058.

**Recommended fix (preferred):** hoist a minimal `reborn_validation_queue`-table-only
migration ahead of V051 (split V058 into "create table + indexes" early, "populate + drop
legacy columns" at N). This keeps "from day one" literally true. Alternative: have V051/V052
carry the legacy per-table Q1/Q2 columns temporarily and extend V058's populate/drop to all
15 tables (amending the "13" counts and walking back the "Do NOT include" guidance at
lines 2048/2105). The sequence as written is not landable.

> **✅ ADOPTED (third path, not (a) or (b)).** Neither recommended option was taken; a cleaner
> third path was chosen and written into the plan. V051/V052 carry **only** `validation_status`
> (no per-table queue columns, no hoisted queue table); the plan now states the snippet→Q1→Q2
> promotion is a **Phase N capability** (queue + gate logic land together at V058). The
> pre-Phase-N window is an explicit, documented limitation (WebUI-authored class 22/23 rows sit
> at `pending` with no queue row and are not fetchable until V058 back-fills them), not dead
> code. Concretely in the plan: (i) §0.5 / Phase B / Phase C / Phase N.4 "from day one" /
> "rely on the queue from day one" wording was rewritten; (ii) V058 populate = 15 tables (with a
> literal-default IMPLEMENTATION NOTE for the column-less 22/23 arms), drop = 13 existing
> tables (the two Phase B/C tables never carried the five columns); (iii) the N.5 boot-integrity
> UNION ALL now enumerates all 15 tables explicitly. The C1 review note in the plan carries a
> ✅ RESOLVED marker. The sequence is now landable.

---

## 4. Stale / Factual Errors in the Plan (must be corrected, not implemented)

### 4.1 Phase N — "FINDING D" is factually wrong (M4, line 3448) — ✅ RESOLVED

**Status: RESOLVED.** The plan's N.1 trigger (Step 4) now uses the correct
`INSERT … ON CONFLICT (…) DO UPDATE` form with an explicit note that it matches the existing
`PgMontyVmSettingsStore::upsert` pattern (`pg_monty_vm_settings.rs:162-179`), and the stale
"bare UPDATE" premise / trailing "must be the INSERT+ON CONFLICT form" line was reworded so a
future implementer does not "correct" a correct trigger. No code change required — the
current trigger/upsert code was already correct; the plan text was the only thing wrong.

FINDING D claims `PgMontyVmSettingsStore::upsert` "will NOT create a row if one does not
yet exist" and that the N.1 trigger contains a "bare `UPDATE`". Verified against current
code:

- `./crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs:108` (signature;
  write SQL at `:162-179`): `upsert` **is** a true
  `INSERT INTO reborn_monty_vm_settings (…) VALUES (…) ON CONFLICT ON CONSTRAINT
  reborn_monty_vm_settings_scope_unique DO UPDATE SET …`. It reads the current row first
  (`self.get(…)` at `:125`) only to fill unchanged fields for the `DO UPDATE SET` clause;
  the write itself creates the row if absent. The line cite "line 103" is also off
  (actual `:108`).
- The N.1 `reborn_validation_queue_graduation()` trigger in the plan (lines ~3371-3376)
  **already** uses `INSERT INTO reborn_monty_vm_settings (…) VALUES (…) ON CONFLICT
  (tenant_id, user_id, agent_id, project_id) DO UPDATE SET last_graduation_at = now()` —
  option (b) is already implemented. The conflict target is backed by the
  `reborn_monty_vm_settings_scope_unique` constraint (`V034:67-69`).
- `reborn_monty_vm_settings` (`V034:15-69`) has `id DEFAULT gen_random_uuid()` and every
  resource column `NOT NULL DEFAULT …`; only `active_orchestrator_id` is nullable. The
  trigger's 5-column INSERT (plus `last_graduation_at`, added nullable in Step 3 of the
  same migration) succeeds — remaining columns take defaults — so the first graduation
  atomically creates the cursor row.

**Action:** treat FINDING D as already-resolved/superseded. Do **not** add a second
`INSERT…ON CONFLICT` "fix" on top of the existing trigger (no-op duplicate), and do not
rewrite `PgMontyVmSettingsStore::upsert`. The trailing line 3483 ("the trigger SQL must be
the INSERT+ON CONFLICT form, not a bare UPDATE") restates the stale premise and should be
deleted or reworded so a future implementer does not "correct" a correct trigger and risk
introducing a real bug.

### 4.2 `doc_type_weight_by_class` arms are obsolete (L1-L4, lines 1078/2062/2118/3013) — ✅ RESOLVED

**Status: RESOLVED.** All four LOW findings are marked obsolete in the plan itself:
- **L1** — §0.8 review note (plan line ~1078) states the function is gone and the weight
  table is retained for historical/authoring-intent only (must NOT be read as an
  instruction to add arms).
- **L2** — Phase B review note (plan line ~2095) drops the `22 => 0.42` arm; the test
  `doc_type_weight_by_class(22) == 0.42` is struck through (plan line ~2118) as removed.
- **L3** — Phase C review note (plan line ~2157) drops the `23 => 0.38` arm; the test
  `doc_type_weight_by_class(23) == 0.38` is struck through (plan line ~2183) as removed.
- **L4** — Phase K notes that only `extract_keywords`/`keyword_match_score` +
  `RamSource` + `retrieval_dbless.rs` remain for deletion; the weight function is already
  gone.

`Goals_pre_v3_review.md` Step 12 (commit `e0e7d164`) removed `doc_type_weight_by_class`
from `./crates/brassclaw_engine/src/memory/retrieval_dbless.rs` entirely, along with the
filesystem fallback-content path it served. Verified by grep: the function does not exist
in `crates/`. Both `RamSource::fetch_for_consumer` and `PostgresSource::fetch_for_consumer`
now order by `(class_code ASC, prompt_uid ASC)` (confirmed: doc comments at
`retrieval_source.rs` lines 21/100/111/239 and `ORDER BY class_code ASC, prompt_uid ASC`
SQL at line 441). There is no keyword/weight scoring step left to extend for classes 22/23.
The plan's `22 => 0.42` (Phase B) and `23 => 0.38` (Phase C) match arms reference a dead
function and must not be implemented. The weight tables are retained in the plan for
historical/authoring-intent only.

---

## 5. Per-Phase Findings (detail)

### §0 Architecture (M1, M2, M3, L1)

- **§0.3 (line 139):** the runtime flow assumes `PostgresSource::fetch_for_turn` is the
  live retrieval backend. It is not — `manager.rs:383` wires `RamSource`. Ordering
  consequence for Phases H/K (see §2 above).
- **§0.8 (line 738):** `PostgresSource` is correct but not the live backend;
  `ActionShortCircuit` and `SplitResult` cannot be reached until it is wired.
  `IntentResolution::Match` today has exactly `{ component_id, component_class_code }`
  (verified in `intent_system.rs` — no `step_link`), confirming Phase D's addition is
  greenfield.
- **§0.9 FINDING F (line 963):** `formatted_content` is shape-polymorphic today. The
  normal-assembly branch builds `{"prior_knowledge":[…], "matched_components":[…]}` as a
  JSON string (`orchestrator.rs` ~2706-2710); the single-override/Action branch sets
  `formatted_content` to the **prose** `item.effective_content` (~2677). The plan's cite
  "2674" is within the function but the JSON `.to_string()` is at ~2710. Phase F should
  reference both branches when documenting the shape change.
- **§0.8 (line 1078):** `doc_type_weight_by_class` removed (see §4.2).

### Phase A — Recipe types + store round-trip (H1, line 1995)

The live `Recipe` struct in `./crates/brassclaw_engine/src/types/recipe.rs` is the **v2**
design (`RecipeStep { skill, tool, params, description }`); no `RecipeVariant`,
`BuildInstruction`, `StepDescription`, or `step_link`. Phase A establishes v3 types and
preserves `trigger`/`steps` as fallback — correct. **Gap:** the runtime `RecipeStage`
consumes `RecipeMatchDto` (`./crates/brassclaw_reborn_composition/src/pg_recipe_store.rs`
~799) built from `PgRecipe`. `PgRecipe`, `RECIPE_SELECT`, `decode_recipe_row`, and
`NewPgRecipe` have **no `step_descriptions` field**. Phase A's file list omits the store
round-trip entirely. **Resolution (Phase H, line 2564):** the dispatch path reads
`step_descriptions` straight from the `reborn_recipes` row via `PostgresSource`, so
`RecipeMatchDto`/engine `Recipe` do not need it — *but* the WebUI authoring/save path
(`PgRecipeStoreFacade`, `pg_recipe_store.rs:861`) still needs to SELECT/decode/insert
`step_descriptions`, so `RECIPE_SELECT`/`decode_recipe_row`/`NewPgRecipe` must be extended.
This was missing from Phase A's file list and must be added.

### Phase E — `PostgresSource::fetch_for_turn` extensions (C2, line 2263)

See §2. Code correct, dormant until wiring. Wiring is an E/H prerequisite.

### Phase F — handler upgrade (H2, C3, lines 2360 & 2374)

- **Scope bug (H2, line 2360):** `handle_assemble_prior_knowledge` constructs
  `ComponentScope` with `tenant_id:"default"` and `agent_id:String::new()`
  (`orchestrator.rs:2575-2580`; the plan cites "line 2581"). The `Thread` struct
  (`./crates/brassclaw_engine/src/types/thread.rs:212`) carries `user_id` and `project_id`
  but **has no `tenant_id` field and no `agent_id` field** — that is *why* the literals are
  hardcoded. In a multi-tenant deployment, User A could match intents seeded by User B's
  tenant. Phase F must either (a) add `tenant_id`/`agent_id` to `Thread` (touching
  `Thread::new`, every creator, and checkpoint serde via `#[serde(default)]`), or (b)
  source them from the turn/loop context available at the call site. Option (b) is likely
  cheaper but must be confirmed against the actual call site. This is larger than
  "construct the scope from the thread" implies and must be scoped in Phase F.
- **Fallback (C3, line 2374):** see §2. Preserve the `retrieve_context` fallback in Phase
  F; remove it in Phase K.

### Phase H — `RecipeStage` dispatch + inputs (H3, H4, lines 2514 & 2564)

- **COMP-01 resolved (H3, line 2514):** `LoopInput` is defined in
  `./crates/brassclaw_turns/src/run_profile/host.rs:843`. The variant is
  `UserMessage { message_ref: LoopMessageRef }` — **no `content` field**; the payload is a
  reference, not text. `LoopMessageRef` is an opaque newtype from the `loop_ref!` macro
  (`brassclaw_turns/src/ids.rs:242`, string form `"msg:…"`). `FollowUp` and `Steering` are
  identical. Consequence: `consume_drainable_inputs` (`input.rs:154`) cannot "extract the
  text" from the `LoopInput` alone. To populate `state.last_user_text`, Phase H must
  resolve the ref via a host/turn API (the accepted-message body keyed by `message_ref`),
  or capture the text earlier in the pipeline where it is still available (the
  turn-submission path that mints the ref). The plan's "capture the text from whichever
  UserMessage/Steering input was consumed" understates this. Specify exactly which call
  resolves `message_ref → text`.
- **Host port (H4, line 2564):** `RecipeStage` is in `brassclaw_agent_loop`, which does not
  depend on `brassclaw_engine`; `RetrievalSource`/`PostgresSource` live in
  `brassclaw_engine`. The retrieval source is exposed to stages via the host port
  (`ctx.host.…`), not a direct import. Phase H must specify the host-port method that
  exposes `fetch_for_turn` to `RecipeStage` (the §0.3 flow assumes it exists; verify or
  add it).

### Phase J.1 — skill intent examples (H5, line 2858)

J.1 conflates three distinct things:

1. **Migration V054 `ADD COLUMN IF NOT EXISTS intent_examples` is a NO-OP.**
   `reborn_skills` already has `intent_examples JSONB NOT NULL DEFAULT '[]'` — added in
   `V027__reborn_skills.sql:67`, GIN index at `V027:139-141`. It stores an array of
   `{input, class}` objects (class 1|2|3), **not** an array of strings. The §2 table
   already flags V054 as a no-op; J.1 should state it explicitly.
2. **`SkillManifest` (`./crates/brassclaw_skills/src/types.rs:127`) has NO
   `intent_examples` field today.** Intent examples are authored via the **skill store**
   input structs (`CreateSkillInput`/`UpdateSkillInput` in `db_store.rs:167`/`:207`) as
   `intent_examples: JsonValue`, validated to the `{input, class}` shape at
   `db_store.rs:348-371` and persisted at `db_store.rs:463/505`. They are DB-only metadata
   set through the store API, not a SKILL.md frontmatter field. Adding
   `intent_examples: Vec<String>` to `SkillManifest` would (a) introduce a new SKILL.md
   authoring surface that doesn't exist, and (b) be **shape-incompatible** with the DB
   column — `Vec<String>` drops the `class` 1|2|3 field the schema/validator require.
   Reconcile before implementing: keep intent examples DB-only (drop the `SkillManifest`
   change) or, if SKILL.md authoring is desired, use `{input, class}` objects.
3. **The genuinely missing wiring is `auto_passed → seed_intent_input`.**
   `seed_intent_input` (`intent_system.rs:462`, writes to `reborn_intent_inputs`, the V028
   table `resolve_intent` actually queries) is **not called from `brassclaw_skills`
   today** (only references are `intent_system.rs` and `pg_intent_inputs_store.rs`). Skill
   intent examples sit on the skill row but never reach the intent-inputs table —
   `resolve_intent` cannot match them until J.1 wires `auto_passed → seed_intent_input`.
   This is the correct core of J.1; points 1 and 2 are separable.

### Phase K — deletion ordering (C4, L4, lines 3024 & 3013)

- **L4 (line 3013):** `doc_type_weight_by_class` already deleted; only
  `extract_keywords`/`keyword_match_score` (move to `retrieval_source.rs`), the
  `retrieval_dbless.rs` file, and the `RamSource` struct/tests in `retrieval_source.rs`
  remain for Phase K to delete.
- **C4 (line 3024):** see §2. Split K.3 into wire-then-delete.

### Phase N — validation queue (M4, M5, lines 3448 & 3584)

- **M4 FINDING D (line 3448):** see §4.1 — stale and factually wrong.
- **M5 N.4 struct audit (line 3584):** verified the engine `Recipe` struct
  (`types/recipe.rs:144`) carries `validation_errors: Vec<String>` (:167),
  `review_feedback: Option<String>` (:168), `review_attempts: u32` (:169),
  `rejected_at: Option<DateTime<Utc>>` (:170) and has **no** `queue_code` (matches N.4).
  `PgRecipe` (`pg_recipe_store.rs:117`) + `RECIPE_SELECT` (:208-217) select/decode all
  five incl. `queue_code` (:120, :236 area). **`decode_recipe_row` (:219) is positional**,
  so dropping a column requires renumbering every `row.get(N)` index after it — the
  "remove all five from both `PgRecipe` and `RECIPE_SELECT`" must also re-index
  `decode_recipe_row`, not just delete lines. `RecipeValidationStatusUpdate` (:170) does
  carry `validation_errors`/`review_feedback`/`queue_code`. `recipe_matcher.rs` reads
  `wilson_lower` + `tier` (not dropped) and references the dropped fields in conversion
  paths — the N.4 "audit required" is genuine. The two-phase deploy is the correct
  mitigation.

### §2 Migration Sequence (C1, line 3657)

See §3.

### §3 Open Questions — verified consistent, no new note required

Q7 ("`__assemble_prior_knowledge__` removal timing") is accurate: the handler already
calls `fetch_for_turn` and returns `{content, formatted_content, override_prompt_creation,
matched_component_ids}`; Phase F extends it; Phase G removes the dead
`__retrieve_docs__` *shim call*; Phase K removes the `__retrieve_docs__` handler
registration. The imprecise "via `PostgresSource`" lives in the Phase F clarification block
(line ~2298), corrected by note C3. Q13 (stash/unstash) and Q14 (Tier 0 Python runs) are
internally consistent with the Phase F/H notes. Q8/Q12 (`source:"system"` builtins bypass
Q2 with `validation_status:"validated"`) are correctly excluded by the §0.18 boot-integrity
check's `WHERE validation_status != 'validated'` (lines ~3590-3593), so system builtins are
not auto-submitted to the queue — consistent. No §3 edits were needed.

---

## 6. Required Changes Before Implementation

1. **Insert a "wire `PostgresSource`" prerequisite** (Phase E.0 or hoist into E/H) and
   verify a live turn takes `fetch_for_turn` before Phases F/G/H consume its variants.
   Resolves C2, and the transitive dependency in C3/C4/H4. (§2)
2. **Split Phase K.3** into (1) wire `PostgresSource`, ship, verify; (2) delete `RamSource`
   + `retrieval_dbless.rs` + `handle_retrieve_docs` + the `retrieve_context` fallback in
   `orchestrator.rs:2620-2637` +, if no remaining callers, `retrieve_context` itself.
   Resolves C3/C4. (§2, §5/Phase K)
3. **Resolve the V058 ordering contradiction** (§3): hoist a queue-table-only migration
   ahead of V051, or extend V051/V052 to carry per-table Q1/Q2 columns and amend V058's
   "13 tables" counts to 15. Resolves C1.
4. **Phase A: add the store round-trip** — extend `PgRecipe`/`RECIPE_SELECT`/
   `decode_recipe_row`/`NewPgRecipe` for `step_descriptions` so the WebUI authoring path
   works. Resolves H1. (§5/Phase A + Phase H)
5. **Phase F: fix the scope bug** with a real `tenant_id`/`agent_id` source (add to `Thread`
   or source from turn/loop context), and explicitly preserve the `retrieve_context`
   fallback. Resolves H2/C3. (§5/Phase F)
6. **Phase H: specify the `message_ref → text` resolution call** and the host-port method
   exposing `fetch_for_turn` to `RecipeStage`. Resolves H3/H4. (§5/Phase H)
7. **Phase J.1: drop the no-op migration note and the `SkillManifest` `Vec<String>`
   change** (or use `{input,class}`); implement only the `auto_passed → seed_intent_input`
   wiring. Resolves H5. (§5/Phase J.1)
8. **Phase N: delete or reword FINDING D** (and the trailing line 3483) — it is factually
   wrong; do not "fix" the correct `upsert`/trigger. Resolves M4. (§4.1)
9. **Phase N.4: re-index `decode_recipe_row`** (positional) when dropping the five
   columns, not just delete lines. Resolves M5. (§5/Phase N.4)
10. **Phases B/C and §0.8: drop the dead `doc_type_weight_by_class` match arms** — the
    function no longer exists. Resolves L1-L4. (§4.2)

---

## 7. Appendix — In-Plan Note Anchors (18)

| # | Plan line | Section | Sev |
|---|----------|---------|-----|
| 1 | 139 | §0.3 | MEDIUM |
| 2 | 738 | §0.8 | MEDIUM |
| 3 | 963 | §0.9 (FINDING F) | MEDIUM |
| 4 | 1078 | §0.8 (doc_type_weight) | LOW |
| 5 | 1995 | Phase A | HIGH |
| 6 | 2062 | Phase B | LOW |
| 7 | 2118 | Phase C | LOW |
| 8 | 2263 | Phase E | CRITICAL |
| 9 | 2360 | Phase F (scope bug) | HIGH |
| 10 | 2374 | Phase F (fallback) | CRITICAL |
| 11 | 2514 | Phase H (COMP-01) | HIGH |
| 12 | 2564 | Phase H (store/host port) | HIGH |
| 13 | 2858 | Phase J.1 | HIGH |
| 14 | 3013 | Phase K (doc_type_weight) | LOW |
| 15 | 3024 | Phase K.3 (ordering) | CRITICAL |
| 16 | 3448 | Phase N (FINDING D) | MEDIUM |
| 17 | 3584 | Phase N.4 | MEDIUM |
| 18 | 3657 | §2 (migration ordering) | CRITICAL |

> All 18 notes are present in `./saved_plan_to_v3.md` as `> **Review note (pre-v3 audit):** …`
> blocks. This document is the index; the plan is the authoritative location of each
> finding's full text.

---

## 8. Second-Agent Review Verification + DRIVER-GAP Design Resolution

A second, independent review of `./saved_plan_to_v3.md` reported ~13 more issues plus a
flagged **🚨 TIER0-GAP** design gap. Per the user's instruction, each claim was re-verified
against live code, the design gap was traced to its **root cause**, and a resolution was
written into the plan. Summary below; authoritative text lives in `./saved_plan_to_v3.md`
(index rows `TIER0-GAP`, `DRIVER-GAP`, and Phase H.0 §H5).

### 8.1 Second-agent claims — verification verdicts

| Tag | Verdict | Note |
|-----|--------|------|
| `ARCH-01` | ✅ Correct | `ExecutionLoop::with_retrieval_source` exists (`loop_engine.rs:219`) and is called at `manager.rs:400`. Work is adding the override field to `ThreadManager`. Already fixed in plan. |
| `ARCH-02` | ✅ Correct | `ThreadManager` is in `brassclaw_engine/src/runtime/manager.rs`, NOT `crates/brassclaw_reborn_composition/src/runtime.rs`. Already warned in plan. |
| `SQLX-01` | ✅ Correct | Repo uses `tokio-postgres`/`deadpool-postgres` only — no sqlx. N.3 already corrected in plan. |
| `SCHEMA-01` | ⚠️ Correct but overstated | `review_attempts` is `INT` in only **3** tables (V027 skills, V029 actions, V030 tools) and `SMALLINT` in the other **10** (V032 extensions_unified, V033 recipes, V036 specs, V037 tool_skills, V038 plans, V039 summaries, V040 docus, V041 lessons, V042 issues, V043 notes). `validation_errors` (TEXT[]) and `rejected_at` (TIMESTAMPTZ) are uniform across all 13. So the V058 `COALESCE(review_attempts, 0)` cast is needed for the **10 SMALLINT tables only** (queue `counter` is `INT`). Plan note already says "for SMALLINT tables" — accurate; the 3 INT tables need no cast. |
| `RETRIEVAL-01` | ✅ Correct, refined | `retrieval_dbless.rs` still exists with `doc_type_weight(DocType)` (line ~76) AND `extract_keywords`/`keyword_match_score`. Only `doc_type_weight_by_class(i32)` was removed (Goal-pre-v3 Step 12). Plan §0.11/K.3 already corrected. |
| `PERF-03` | ✅ Correct | Verified 12 per-class sub-selects in `PostgresSource::fetch_for_consumer`; +classes 22/23 → 14. Already fixed in plan. |
| `RECIPE-SELECT` | ✅ Correct | `RECIPE_SELECT` ends at index 30 (`updated_at`); append new columns at 31/32/33 to avoid re-indexing 31 `row.get(N)` calls. Already fixed in plan. |
| `CANONICAL-01` | ✅ Correct | `canonical.rs:94-96` single-variant `RecipeStep::Continue` exhaustive match → compile error when `TierZero`/`ActionExecuted` added. Already noted (COMP-02). |

All second-agent corrections were already applied to the plan in a prior session; this pass
confirmed they are accurate against live code. No additional plan edits were needed for these
eight (SCHEMA-01 precision is recorded here for completeness).

### 8.2 The design gap — root cause (DRIVER-GAP) and resolution

The second agent flagged **TIER0-GAP**: "how is Python kicked in Tier 0 when
`CapabilityStage` reacts to model output and there is none?" Tracing this to its root cause
surfaced a deeper, plan-wide gap now tagged **DRIVER-GAP**.

**Root cause (verified against live code):**
- The **production turn driver is the engine `ExecutionLoop::run`** (`./crates/brassclaw_engine/src/executor/loop_engine.rs:413`), which calls `execute_orchestrator` (Python `default.py`) directly with **no stage pipeline**.
- The agent-loop **`DefaultExecutorPipeline::execute`** (`./crates/brassclaw_agent_loop/src/executor/canonical.rs`) — where `RecipeStage`/`PromptStage`/`ModelStage`/`CapabilityStage` live — is a **skeleton**: `DefaultExecutorPipeline`/`execute_family` appear **only** inside `brassclaw_agent_loop` (canonical.rs, pipeline.rs, tests). No product surface drives it.
- `brassclaw_agent_loop` does **not** depend on `brassclaw_engine` (Cargo.toml), and `__assemble_prior_knowledge__` exists **only** in `brassclaw_engine`. So the plan's RecipeStage↔Python-step-0 stash/unstash (Phase H item 5) and the §5 Tier 0 diagram ("Python runs `default.py` step 0" inside the agent loop) both assume a RecipeStage-then-Python unification that **does not exist**.

**Resolution (written into the plan, Phase H.0 §H5):**
1. A third host-port prerequisite — **`LoopOrchestratorPort`** (15th `AgentLoopDriverHost` port, verified current list has 13 ending at `LoopInterceptorPort` at `host.rs:2198`; H4's `LoopRetrievalPort` is 14th). It exposes the engine Python orchestrator to agent-loop stages as `run_step_zero` (Tier 1 prior-knowledge) and `run_tier_zero` (Tier 0 no-LLM execution). It is implemented by **`brassclaw_reborn_composition`** — the only crate depending on both `brassclaw_engine` and `brassclaw_agent_loop` (Cargo.toml). `brassclaw_turns`-native payload types (`PriorKnowledgeBundle`, `TierZeroReply`) preserve the crate boundary; a `NoOrchestrator` default returns `None` (Tier 0 degrades to Tier 2).
2. **TIER0-GAP kick mechanism — Option 1 chosen (agent-loop path):** a new **`TierZeroExecutionStage`** in `canonical.rs` (between `RecipeStage` and `AssistantReplyStage`), invoked only on `PostRecipeOutcome::TierZero`, calls `ctx.host.run_tier_zero(...)`. `CapabilityStage` is **not** bent (keeps its model-output assumption; simply skipped). Option 2 (synthetic signal into `LoopCapabilityPort`) is rejected — it would couple the capability port to Tier 0 routing.
3. **MODEL SELECTION refinement (after a third agent correctly flagged the plan was still mixing runtimes):** the plan now names the two execution models explicitly and covers **both** during migration —
   - **Model A — engine `ExecutionLoop` / Monty (CURRENT PRODUCTION).** Verified Python IS the outer loop and calls the LLM via `__llm_complete__` (`default.py:1103` → `handle_llm_complete`, `orchestrator.rs:563`), and the engine ALREADY has a deterministic no-LLM path: `execute_action_procedure` (`default.py:901`, "returns without calling `__llm_complete__`") gated by `override_prompt_creation: true` (`orchestrator.rs:2689`, from `assemble_from_component_items`). **Phase H item 3b wires production Tier 0 onto this existing signal** — `handle_assemble_prior_knowledge` emits `override_prompt_creation: true` when `SplitResult.llm_call_required == false`; Python step-0 skips `__llm_complete__` and runs the recipe's orchestrator channel deterministically. No new "kick" — the kick IS `default.py` step-0, already live. This gives production Tier 0 NOW, before any agent-loop switchover. (The earlier draft wrongly "rejected" the engine-side implementation, which would have left production with no Tier 0.)
   - **Model B/C — agent-loop `DefaultExecutorPipeline` (target state, skeleton today).** `LoopOrchestratorPort` + `TierZeroExecutionStage` bridge to the engine orchestrator. Active only after the switchover (`DRIVER-PREREQ`); test-only until then.
   - **LLM-call ownership (v3 target):** once the agent-loop is the driver, `ModelStage` owns the Tier 1+ LLM call and the Python `__llm_complete__` loop is retired. During migration both mechanisms coexist: Model A serves production, Model B/C stages are test-only.

**Plan edits applied this pass:** index rows `TIER0-GAP` (marked ✅ RESOLVED) + new `DRIVER-GAP`; Phase H.0 §H5 (port spec + `TierZeroExecutionStage` + DRIVER-PREREQ + MODEL SELECTION naming A/B/C + engine-path Option A mechanism); Phase H item 3b (engine-path `orchestrator.rs`/`default.py` Tier 0 wiring via `override_prompt_creation`); Phase H §4 Tier 0 dispatch arm (explicit kick pseudocode); §5 Tier 0 turn-flow diagram (reframed from the wrong "CapabilityStage / Python execution" to "TierZeroExecutionStage — kicks Python via LoopOrchestratorPort", with a Model A/B/C header note); §0.3 Tier 0 flow (split into Model A vs Model B/C); Q14 (resolution recorded); §0.9 + item-5 DRIVER-GAP cross-references. This review doc's §1/§2 narrative is unchanged (the C2 dormancy theme now extends to the driver itself).

**No Rust code was modified this pass** — DRIVER-GAP/TIER0-GAP are plan-design resolutions, not current-code bugs (the agent loop being a skeleton is by design / a separate migration; the engine `override_prompt_creation`/`execute_action_procedure` machinery already exists and is reused, not changed). The only code change across all sessions remains the H2 fix at `./crates/brassclaw_engine/src/executor/orchestrator.rs:2573-2591` (verified: clippy clean, 610 tests pass). No `git commit` was run (per the standing constraint).
