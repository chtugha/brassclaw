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
| 10 | `10-prefix-base-prompt.md` | vLLM prefix caching, `base-prompt` placeholder substitution, `reborn_basic_prompt_store` (Phase K.1/V055), Prefix Tab | planned |
| 11 | `11-retrieval-system.md` | `RamSource` (current prod, keyword-over-postgres), `PostgresSource` (intent-driven, not wired), `fetch_for_turn` vs `fetch_for_consumer`, E.0/K.3 ordering | planned |
| 12 | `12-agent-loop.md` | Turn pipeline stages, `RecipeStep::Continue`, `TierZeroExecutionStage` (Phase H.0), host ports, DRIVER-GAP | planned |
| 13 | `13-orchestrator-default-py.md` | Python orchestrator `default.py`: step-0 three calls, `__llm_complete__`, `execute_action_procedure`, v3 step-0 handler changes | planned |
| 14 | `14-validation-queue.md` | `reborn_validation_queue` (Phase N/V058), Q1/Q2 graduation, Wilson scoring, graduation trigger + upsert | planned |
| 15 | `15-component-catalog.md` | Unified component tables (class 12–20), new classes 22/23, `PgMemoryDocStore`, `component_import` | planned |
| 16 | `16-kernel-composition.md` | Kernel authority (trust/secrets/safety/sandbox/capabilities/identity) + composition/wiring (factory.rs, runtime.rs, config) | planned |
| 17 | `17-webui-prefix-tab.md` | WebUI v2 SPA, settings tabs, Prefix Tab, recipe/skill/tool/PythonCode authoring UI, `SKILL.md` export | planned |
| — | `DOC_CONVERSION_MECHANISM_DESIGN.md` | Design/approach (presented, not yet implemented) for the auto-conversion mechanism (repeat item 4) | planned |

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
