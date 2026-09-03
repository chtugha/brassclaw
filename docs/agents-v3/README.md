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
> *current* runtime. This `docs/agents-v3/` set is **v3-grounded**: for each topic it states
> (a) what is **shipped** in the codebase today, (b) what **remains pending** in the v3 plan,
> and (c) the **LLM-relevant summary** that the conversion mechanism will distill into the base
> prompt / prefix cache / on-demand context. Where a `docs/reborn/contracts/*` contract already
> covers a subsystem authoritatively, this set links to it rather than duplicating it, and
> focuses on the v3 delta.

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
| 06 | `06-tools-system.md` | `reborn_tools` + `capability_id` (V071 — the Executioner's dispatch id; dropped `cdylib_artifact_path` + 5 legacy cols), builtin bootstrap (`host.*` tools), tool approval/auth, Rust-Executioner framing | done |
| 07 | `07-pythoncode-system.md` | `reborn_python_code` (V052 + `includes` JSONB V069), snippet→component promotion, `{{...}}` placeholder-grammar Q1 gate (`validate_python_code_placeholders` — shipped), `UnpromotedSnippet`, shell-injection scan, class 22 in retrieval UNION/by-id | done |
| 08 | `08-actions-system.md` | Class-16 Actions, 13 step types, `execute_action_procedure` (no-LLM), `override_prompt_creation` (Solution Override, LLM path), `action_short_circuit` (Phase E/G), dead `__retrieve_docs__` shim (§0.9), `call_action` by-name→UUID migration (Phase G) | done |
| 09 | `09-sempai-kohai.md` | Sempai-Kohai interceptor: routing (Kohai always-on) vs rerouting (Sempai pre-send review); `SempaiReviewOutcome` → adjusted prompt + Q1 proposals (`SempaiProposalSink`); base-prompt config keys; `base-prompt` substitution shipped (per-turn `get_system_bundle` + `minimal_base_prompt_fallback`); New-Component-Creation Process (f8) | done |
| 10 | `10-prefix-base-prompt.md` | vLLM prefix caching, `base-prompt` placeholder substitution (shipped: per-turn `get_system_bundle` + `minimal_base_prompt_fallback`), `reborn_basic_prompt_store` (V063) + `PgBasicPromptStore` (`get_for_scope`/`store`/`mark_stale`), Sempai vs Kohai base prompt, `do_assemble_bundle` assembly, Prefix Tab | done |
| 11 | `11-retrieval-system.md` | `RetrievalSource` trait, `PostgresSource` (active prod backend via `PgRetrievalLookup`, intent-driven UNION ALL across class tables 1–23), `RamSource` (keyword-over-postgres, dormant → deleted Phase K.3), `fetch_for_turn` vs `fetch_for_consumer`, all 4 `FetchForTurnResult` variants, `fetch_component_by_id`, SEC-01 validated gate, class 22/23 shipped | done |
| 12 | `12-agent-loop.md` | Canonical executor pipeline (`DefaultExecutorPipeline` stages incl. `tier_zero`) IS the production turn driver via `PlannedDriver`→`CanonicalAgentLoopExecutor::execute_family`; `ExecutorStage` trait; real `RecipeStage` (H.9 retrieval + H.10 `Continue`/`TierZero` dispatch, not a stub); 15-port `AgentLoopDriverHost` (`LoopRetrievalPort`+`LoopOrchestratorPort` added); `TierZeroExecutionStage`; engine Monty VM `execute_orchestrator` dormant → activated C.5/C.6 (`host.run_program`) | done |
| 13 | `13-orchestrator-default-py.md` | The Orchestrator/Executioner split: Monty (Python) = brain (recipe/intent-driven, calls tools via `host.<tool>(...)`, assembles every LLM prompt, runs composed code via `host.run_program`); Rust = executioner (precompiled Tools + ToolSkills, executes on call, no sequencing). The retired `default.py` Model-A outer loop + meta-primitives (`__execute_action__` etc.) are gone; the composed-program runner is the C.5/C.6 activation | done |
| 14 | `14-validation-queue.md` | `reborn_validation_queue` (Phase A.5/V051 + Phase N/V059), two-state-machine model (queue pre-validation vs `validation_status` post-validation), Q1 Gate-1 (`ComponentValidator` + `q1_orchestrator.rs`) / Q2 manual review, state-2 security invariant, Wilson scoring + `classify_tier` (live), `is_tier0_eligible` missing-guard fix (`FIND-P7-11`), graduation trigger + `last_graduation_at` upsert (FINDING D resolved), V059 populate/column-drop + `decode_recipe_row` re-index | done |
| 15 | `15-component-catalog.md` | Full class-code taxonomy (0–23), `COMPONENT_TABLES` + `class_label`, common content-table shape (V036 canonical), content-column dispatch (`fetch_component_by_id`), `prompt_uid` stable ordering key, SCH-02 `prior_knowledge_content`, lineage, legacy `MemoryDoc`→class-table import (`component_import.rs`), hierarchy (§0.1), ExtensionCatalogue (§0.2), StepContextSpec (§0.5), cognitive-weight/`FINDING B` (frozen `DocType`), V050–V059 additive migrations | done |
| 16 | `16-kernel-composition.md` | Kernel authority crates (`brassclaw_trust`/`secrets`/`safety`/`capabilities`/`runtime_policy`/`process_sandbox`/`reborn_identity`/`outbound`/`approvals`) with non-negotiable boundaries; composition wiring (`brassclaw_reborn_composition`: `factory.rs` `RebornServices`/`build_reborn_services`, `runtime.rs` `RebornRuntime`, `webui_v2_app` security middleware stack); runtime profile (Goal 1 done: `RebornCompositionProfile` removed, `BRASSCLAW_RUNTIME_PROFILE` = capability policy only; Goal 2 done: Postgres always, no DB-less mode); `PostgresSource`/`PgBasicPromptStore`/15 loop ports/`ValidationQueueStore` all shipped; kernel is not a v3 migration target | done |
| 17 | `17-webui-prefix-tab.md` | WebUI v2 SPA (React + Rust route layer + host ingress), descriptor-driven routes, 18 settings tabs incl. `prefix`; **Prefix Tab shipped** (item 8: `prefix` SPA route + `usePrefixes` hook + `fetchPrefixes`/`regeneratePrefix` API; `GET /api/webchat/v2/prefixes` + `POST …/prefixes/{name}/regenerate`; `do_assemble_bundle`/`regenerate_prefix`/`get_system_bundle`; `reborn_basic_prompt_store` V063 + `PgBasicPromptStore`; `mark_stale`-on-graduation; `base-prompt` placeholder substitution §0.13); SKILL.md export (item 5.1, pending) | done |
| — | `DOC_CONVERSION_MECHANISM_DESIGN.md` | Design/approach (presented, not yet implemented, stopped for approval) for the auto-conversion mechanism (repeat item 4): converts each `docs/agents-v3/*.md` to LLM-optimized form, stores as `reborn_docus` (class 17) components, auto-updates via `content_hash` + idle-time Sempai-Kohai loop, injects into base prompt (Prefix Tab) + per-turn retrieval; built AS v3 artifacts per the **recycling principle** (§4.0: many one-tool reusable leaf skills/tools the library recycles + ONE doc-specific domain skill `doc-convert-method`) — Recipe `doc-convert` (class 21) + Action `doc-sync` (class 16, no-LLM) **compose** the leaves; PythonCode `sha256`/`hash_changed`/`markdown_section`/`token_estimate`/`format_component_header` (class 22, pure logic); Tools `component_get_content_hash`/`docu_upsert`/`mark_prefix_stale` (class 0) + ToolSkills (class 13); ExtensionCatalogue `doc-sync` (class 23) | presented |

---

## How each document is structured (convention)

Every doc in this set follows the same skeleton so it is both readable by humans and reliably
parseable by the conversion mechanism:

1. **Purpose** — one paragraph: what this subsystem is and why it exists.
2. **Location** — the crates, files, tables, and migrations that implement it, with paths.
3. **Data model** — the key types, enums, DB tables/columns, and their relationships.
4. **Behavior / flow** — how the subsystem behaves at runtime, step by step.
5. **Relations** — what it depends on and what depends on it (links to sibling docs).
6. **Status — shipped vs. pending** — an explicit split of what is shipped in the current
   codebase versus what `saved_plan_to_v3.md` still adds. Each pending item cites the plan
   phase/migration that will deliver it.
7. **LLM-relevant summary** — the compact, concept-dense form the conversion mechanism will
   further optimize for inclusion in the base prompt / prefixes / on-demand retrieval. This is
   the seed the mechanism refines, not the final compiled form.

---

## v3 architecture principle — recycling (read this before authoring any recipe)

**The library is the asset.** Skills should be as small as practical — **at best, the
description of ONE tool usage** — so they can be reused in many recipes. Tools too: one concern
each. A Recipe (class 21) is a **composition** of already-existing, one-purpose library parts
(leaf Skills, ToolSkills, PythonCode) referenced by UUID in each step's `include`; the recipe is
the *ordering* + the *wiring*, not the capability itself. Prefer reusing a library part over
authoring a new one; when a genuinely new capability is needed, add it as a small leaf so the next
recipe can reuse it too. **Never bake a whole procedure into one fat skill.** Two skill grains
coexist: **leaf skills** (one tool — the reusable unit, user case (a)) and **domain skills** (span
tools, the bigger picture that *references* leaves by name, user case (b)); one domain skill per
task area. See `05-skills-system.md` "Recycling" and `03-recipe-system.md` §1; the worked example
is `DOC_CONVERSION_MECHANISM_DESIGN.md` §4.0/§4.3 (one recipe + one action composing ~11 reusable
leaves + one domain skill).

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
