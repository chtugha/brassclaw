# BrassClaw Reborn — Agents v3 System Documentation

> **Purpose.** This directory is the authoritative, human-readable documentation set for the
> **agents v3 system**: every system, subsystem, functionality, skill kind, recipe/tool
> construct, and the supporting kernel/composition/WebUI surface that together make up the
> v3 agent. It is written to be **grounded in the real codebase** (`crates/…`) and the v3
> implementation plan (`./saved_plan_to_v3.md`), and to be **machine-convertible** into
> LLM-optimized form by the documentation auto-conversion mechanism (see
> `DOC_CONVERSION_MECHANISM_DESIGN.md`).
>
> **Scope vs. existing docs.** The repo already has rich per-subsystem contracts under
> `docs/reborn/contracts/` and architecture notes under `docs/reborn/`. Those describe the
> *current* (pre-v3) runtime. This `docs/agents-v3/` set is **v3-oriented**: for each topic it
> states (a) what exists **today**, (b) what the **v3 plan adds/changes**, and (c) the
> **LLM-relevant summary** that the conversion mechanism will distill into the base prompt /
> prefix cache / on-demand context. Where a `docs/reborn/contracts/*` contract already covers a
> subsystem authoritatively, this set links to it rather than duplicating it, and focuses on the
> v3 delta.

---

## Documentation taxonomy

Each row is one documentation file in this directory. The status column tracks creation in this
documentation effort.

| # | File | Subsystem / topic | Status |
|---|------|-------------------|--------|
| 01 | `01-architecture-overview.md` | Three-layer model (Products / Loops / Kernel); v3 target message flow; how the new subsystems fit | done |
| 02 | `02-intent-system.md` | Intent matching: `resolve_intent`, 4-class classifier, `reborn_intent_inputs` (V028), `step_link` (Phase D/V053), disambiguation | done |
| 03 | `03-recipe-system.md` | Recipes, `step_descriptions` JSONB (Phase A/V050), variants/steps, tiers 0/1/2, store round-trip | done |
| 04 | `04-ibs.md` | Instruction Builder System: `build_instruction`, `VariablePattern`/`ToolBinding`/`ErrorPolicy`, `SplitResult`, two-channel delivery | done |
| 05 | `05-skills-system.md` | Four skill kinds: Classic (Claude-style, DB-stored, `SKILL.md`-exportable), ToolSkills (Rust executor), Orchestrator Skills, ExtensionCatalogues (class 23) | done |
| 06 | `06-tools-system.md` | `reborn_tools`, `capability_id` (Phase L/V057), builtin bootstrap, tool approval/auth | done |
| 07 | `07-pythoncode-system.md` | `reborn_python_code` (Phase B/V052), snippet→component promotion, Q1/Q2 gate (Phase N/V059), `UnpromotedSnippet`, shell-injection scan | done |
| 08 | `08-actions-system.md` | Class-16 Actions, 13 step types, `execute_action_procedure` (no-LLM), `override_prompt_creation` (Solution Override, LLM path), `action_short_circuit` (Phase E/G), dead `__retrieve_docs__` shim (§0.9), `call_action` by-name→UUID migration (Phase G) | done |
| 09 | `09-sempai-kohai.md` | Sempai-Kohai interceptor: routing (Kohai always-on) vs rerouting (Sempai pre-send review); `SempaiReviewOutcome` → adjusted prompt + Q1 proposals (`SempaiProposalSink`); base-prompt config keys; `base-prompt` substitution is Phase K.1 | done |
| 10 | `10-prefix-base-prompt.md` | vLLM prefix caching, `base-prompt` placeholder substitution, `reborn_basic_prompt_store` (Phase K.1/V056), Sempai vs Kohai base prompt, `do_reassemble` assembly, Prefix Tab | done |
| 11 | `11-retrieval-system.md` | `RetrievalSource` trait, `RamSource` (current prod, keyword-over-postgres), `PostgresSource` (intent-driven UNION ALL, dormant), `fetch_for_turn` vs `fetch_for_consumer`, `FetchForTurnResult`, `fetch_component_by_id`, SEC-01 gate, E.0 wire-then-K.3-delete split | done |
| 12 | `12-agent-loop.md` | Canonical executor pipeline (`DefaultExecutorPipeline` stages), `ExecutorStage` trait, `RecipeStage` stub (Tier 0/1/2), 13-port `AgentLoopDriverHost`, DRIVER-GAP (engine vs agent-loop drivers), `TierZeroExecutionStage` + `LoopRetrievalPort`/`LoopOrchestratorPort` (Phase H.0) | done |
| 13 | `13-orchestrator-default-py.md` | Python orchestrator `default.py` (Model A outer loop): `run_loop` iteration, step-0 three calls (`__assemble_prior_knowledge__` PKC, dead `__retrieve_docs__`+class-16 shim, `__list_skills__`/`select_skills`), `__llm_complete__`, `execute_action_procedure` no-LLM, v3 `tier_zero`/`action_short_circuit`/`recipe_hint`/`execute_recipe_orchestrator_channel` + new `pub` Rust fns | done |
| 14 | `14-validation-queue.md` | `reborn_validation_queue` (Phase A.5/V051 + Phase N/V059), two-state-machine model (queue pre-validation vs `validation_status` post-validation), Q1 Gate-1 (`ComponentValidator` + `q1_orchestrator.rs`) / Q2 manual review, state-2 security invariant, Wilson scoring + `classify_tier` (live), `is_tier0_eligible` missing-guard fix (`FIND-P7-11`), graduation trigger + `last_graduation_at` upsert (FINDING D resolved), V059 populate/column-drop + `decode_recipe_row` re-index | done |
| 15 | `15-component-catalog.md` | Full class-code taxonomy (0–23), `COMPONENT_TABLES` + `class_label`, common content-table shape (V036 canonical), content-column dispatch (`fetch_component_by_id`), `prompt_uid` stable ordering key, SCH-02 `prior_knowledge_content`, lineage, legacy `MemoryDoc`→class-table import (`component_import.rs`), hierarchy (§0.1), ExtensionCatalogue (§0.2), StepContextSpec (§0.5), cognitive-weight/`FINDING B` (frozen `DocType`), V050–V059 additive migrations | done |
| 16 | `16-kernel-composition.md` | Kernel authority crates (`brassclaw_trust`/`secrets`/`safety`/`capabilities`/`runtime_policy`/`process_sandbox`/`reborn_identity`/`outbound`/`approvals`) with non-negotiable boundaries; composition wiring (`brassclaw_reborn_composition`: `factory.rs` `RebornServices`/`build_reborn_services`, `runtime.rs` `RebornRuntime`, `webui_v2_app` security middleware stack); runtime profile (Goal 1: `RebornCompositionProfile` removed, `BRASSCLAW_RUNTIME_PROFILE` = capability policy only; Goal 2: Postgres mandatory, partial); kernel is not a v3 migration target | done |
| 17 | `17-webui-prefix-tab.md` | WebUI v2 SPA (React + Rust route layer + host ingress), descriptor-driven routes, 17 settings tabs, existing base-prompt compile path (Interceptor tab reassemble+prewarm → `do_reassemble`/`prewarm` Sempai-gateway shipment, `brassclaw_config` key-value storage), v3 Prefix Tab (Phase K.1 + item 8: `reborn_basic_prompt_store` V056, `PgBasicPromptStore`, `mark_stale`-on-graduation, generate/regenerate per prefix, `base-prompt` placeholder substitution §0.13), SKILL.md export (item 5.1, not yet implemented) | done |
| — | `DOC_CONVERSION_MECHANISM_DESIGN.md` | Design/approach (presented, not yet implemented, stopped for approval) for the auto-conversion mechanism (repeat item 4): converts each `docs/agents-v3/*.md` to LLM-optimized form, stores as `reborn_docus` (class 17) components, auto-updates via `content_hash` + idle-time Sempai-Kohai loop, injects into base prompt (Prefix Tab) + per-turn retrieval; built AS v3 artifacts (Action `doc-sync` + Recipe `doc-convert` + Skill + PythonCode `doc_upsert`/`doc_hash`/`doc_diff` + Tool `mark_prefix_stale` + ExtensionCatalogue) | presented |

---

## How each document is structured (convention)

Every doc in this set follows the same skeleton so it is both readable by humans and reliably
parseable by the conversion mechanism:

1. **Purpose** — one paragraph: what this subsystem is and why it exists.
2. **Location** — the crates, files, tables, and migrations that implement it, with paths.
3. **Data model** — the key types, enums, DB tables/columns, and their relationships.
4. **Behavior / flow** — how the subsystem behaves at runtime, step by step.
5. **Relations** — what it depends on and what depends on it (links to sibling docs).
6. **Status: today vs. v3** — an explicit split of what exists in the current codebase versus
   what `saved_plan_to_v3.md` adds. Each v3 item cites the plan phase/migration that delivers it.
7. **LLM-relevant summary** — the compact, concept-dense form the conversion mechanism will
   further optimize for inclusion in the base prompt / prefixes / on-demand retrieval. This is
   the seed the mechanism refines, not the final compiled form.

---

## Relationship to the documentation auto-conversion mechanism (repeat item 4)

The files here are the **source** documentation. The auto-conversion mechanism (designed in
`DOC_CONVERSION_MECHANISM_DESIGN.md`, to be implemented as v3 artifacts — recipe + skills + tools
+ PythonCode + action — **not** as Rust code) will:

- **Convert** each source doc into an LLM-optimized form (token-compact, concept-dense, stripped
  of human-only navigation).
- **Store** the converted form in the database (the ExtensionCatalogue namespace, class 23, and
  the base-prompt store — see `10-prefix-base-prompt.md`).
- **Keep it updated** automatically via an idle-time Sempai-Kohai-driven refresh loop that
  re-converts when a source doc changes.
- **Make it injectable** into an LLM prompt on demand (selected by intent) and into the
  precompiled prefix prompts (base prompt and future prefixes).

Per the task, the approach for that mechanism is **presented before implementation**; this
README is the index for the source docs that the mechanism consumes.

---

## Relationship to `saved_plan_to_v3.md` and the prior audits

- `./saved_plan_to_v3.md` — the v3 implementation plan (phases A–N). These docs describe the
  *target* system the plan builds; each doc's "Status: today vs. v3" section cites the plan.
- `./Goals_pre_v3_review.md` — the two-goals audit (no installation profiles; no postgres-less
  design). Goal 1 fully accomplished; Goal 2 partially (production path fail-hard; the residual
  postgres-less *test build* is Step 13, deferred to a task with e2e execution capability).
- `./saved_plan_to_v3_review.md` — the 18-finding plan review (all resolved across 14 passes).
- `./MESSAGE_FLOW_AND_PLAN_AUDIT.md` — the message-flow audit (current vs. plan vs. user
  description).

These docs are consistent with those audits and cite them where relevant.
