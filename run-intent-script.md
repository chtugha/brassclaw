# BrassClaw Design Transition — Execution Script

Each step below is self-contained. Run them **one at a time** by spawning a
fresh subagent per step. The subagent gets its own isolated context so the
parent never accumulates the full plan in memory.

## How to execute

For each step, tell Bob:

> "Read the plan at `.zenflow/tasks/i-want-you-to-make-a-plan-to-fun-80d7/plan.md`
> and `spec.md` (for glossary). Implement **only** the step listed below,
> commit all changes, and push. Do not proceed to the next step."

---

## Step 0 — Phase 0 sign-off (SKIP if spec already approved)

**Scope:** Verify spec v5.5 completeness. No code changes.

```
Implement plan step 0 (Phase 0 — Spec & plan sign-off).
Read plan.md + spec.md for context.
Verify all 31 open questions are resolved in spec §7.
Report any unresolved issues. No code changes. No commit.
```

---

## Step 1 — Phase 1: DB-stored Skills + Intent system + Actions

**Scope:** 6 sub-steps (1.1–1.6). Heavy DB + Rust work. Commit after each sub-step.

```
Implement plan step 1 (Phase 1 — DB-stored Skills + Intent system + Actions).
Source files: crates/brassclaw_pg/, crates/brassclaw_skills/,
crates/brassclaw_reborn_composition/, crates/brassclaw_engine/.
Sub-steps:
  1.1 Write V027__reborn_skills.sql migration.
  1.2 Write crates/brassclaw_skills/src/db_store.rs + validator wiring.
  1.3 Write crates/brassclaw_reborn_composition/src/skill_import.rs (SKILL.md importer).
  1.4 Update prompt assembler to load from reborn_skills (feature flag: skills-db).
  1.5 Write V028__reborn_intent_inputs.sql + crates/brassclaw_engine/src/memory/intent_system.rs.
  1.6 Write V029__reborn_actions.sql; add Action execution mode to default.py.
Commit after each sub-step. Push once all 6 are done.
Run: cargo test -p brassclaw_skills -p brassclaw_pg
```

---

## Step 2 — Phase 1.5: Prompt-path dedup + formatting + self-mod boundary

**Scope:** Resurrect `build_step_context`, delete 8 intent functions + 3 Python
formatters, reroute `memory_write`, implement LLM code-audit gate.

```
Implement plan step 2 (Phase 1.5 — Prompt-path dedup + Q19 formatting).
Source files: crates/brassclaw_engine/, crates/brassclaw_turns/,
crates/brassclaw_reborn/, default.py (orchestrator).
Work:
  - Resurrect build_step_context as Rust User-message-at-N-1 injection.
  - Delete signals_tool_intent, signals_execution_intent, score_skill,
    extract_explicit_skills, format_docs, format_skills, append_system_append
    from default.py.
  - Reroute memory_write for code/component changes through __validate_component__.
  - Implement LLM code-audit gate for Orchestrator/Scaffold at Q1→Q2.
Commit + push.
Run: cargo test -p brassclaw_engine -p brassclaw_turns
```

---

## Step 3 — Phase 2: DB-stored Tools (Rusty-only)

**Scope:** V030 migration, tool store, capability surface reads from DB.

```
Implement plan step 3 (Phase 2 — DB-stored Tools).
Source files: crates/brassclaw_pg/, crates/brassclaw_capabilities/.
Work:
  - Write V030__reborn_tools.sql.
  - DB-backed tool store; capability surface reads from reborn_tools.
  - Strip Monty/LLM prompt text from tool rows.
Commit + push.
Run: cargo test -p brassclaw_capabilities
```

---

## Step 4 — Phase 3: Remove trust layer + 4-queue validation lifecycle

**Scope:** Delete SkillTrust, source-driven trust, attenuation; formalize
4-queue lifecycle with validator-tag greyed-out mechanism; per-class validation
config (V031).

```
Implement plan step 4 (Phase 3 — Remove trust layer + 4-queue lifecycle).
Source files: crates/brassclaw_skills/, crates/brassclaw_webui_v2/,
crates/brassclaw_pg/, crates/brassclaw_reborn_composition/.
Work:
  - Delete SkillTrust enum, V2SkillMetadata.trust, default_trust(),
    trust-by-source-directory in registry.rs, skill-trust attenuation.
  - Write V031__reborn_validation_config.sql.
  - Generalize validation routes to all ~20 class codes.
  - Implement Q1/Q2/Q3/Q4 queue state machine + is_valid_transition extensions.
  - Validator-tag pop on Step-2 validate; Q4 wipe as single transaction.
  - Rename RecipeValidator → ComponentValidator with validate_by_class dispatch.
Commit + push.
Run: cargo test -p brassclaw_skills -p brassclaw_webui_v2
```

---

## Step 5 — Phase 4: Unified Extensions + DocPlans dissection + Recipes class 21

**Scope:** V032 (reborn_extensions_unified) + V033 (reborn_recipes class 21),
DocPlan dissection, RecipeLookup from reborn_recipes.

```
Implement plan step 5 (Phase 4 — Unified Extensions + Recipes class 21).
Source files: crates/brassclaw_pg/, crates/brassclaw_extensions/,
crates/brassclaw_engine/src/memory/, crates/brassclaw_reborn_composition/.
Work:
  - Write V032__reborn_extensions_unified.sql (includes prior_knowledge_content
    + override_prompt_creation columns — SCH-02).
  - Write V033__reborn_recipes.sql (class 21, solution-class schema).
  - Unified extension store + class adapters.
  - Dissect DocPlans into constituent parts; migrate DocType::Recipe → reborn_recipes.
  - RecipeLookup trait reads from reborn_recipes (class 21).
Commit + push.
Run: cargo test -p brassclaw_extensions -p brassclaw_engine
```

---

## Step 6 — Phase 5: PlanA-memory connector + intent-driven retrieval + de-chunk

**Scope:** RetrievalSource trait + PostgresSource + RamSource; V034–V043 migrations
(Monty VM settings, user preferences, former-doctype tables); __assemble_prior_knowledge__;
remove brassclaw_embeddings; DB-less fallback file.

```
Implement plan step 6 (Phase 5 — PlanA-memory universal connector).
Source files: crates/brassclaw_memory/, crates/brassclaw_engine/,
crates/brassclaw_pg/, crates/brassclaw_embeddings/.
Work:
  - Write V034__reborn_monty_vm_settings.sql (prior_knowledge_token_budget=2000,
    q4_retention_days=30, single row per scope via upsert).
  - Write V035__reborn_user_preferences.sql (ai_before_user, user-level scope).
  - Write V036–V043 migrations for former-doctype tables (classes 12-20).
  - RetrievalSource trait + PostgresSource + RamSource (DB-less).
  - reborn_component_catalog read model (PERF-05 single-query fetch).
  - __assemble_prior_knowledge__ Rust host function (content-is-king + Solution Override).
  - Monty VM lifecycle manager (kernel-owned, drain + admission control).
  - DB-less fallback-content file (created at install time, keyword retrieval).
  - Remove brassclaw_embeddings crate + all references.
  - Relocate extract_keywords/keyword_match_score/doc_type_weight to retrieval_dbless.rs.
  - Delete all DocType:: variants.
Commit + push.
Run: cargo test -p brassclaw_memory -p brassclaw_engine
```

---

## Step 7 — Phase 5.5: Interceptor activation (Sempai–Kohai wiring)

**Scope:** Close all 5 wiring gaps. V044 ALTER migration. Sempai gateway,
3-part prompt, KV-cache pre-warm, interceptor config service.

```
Implement plan step 7 (Phase 5.5 — Interceptor activation).
Source files: crates/brassclaw_interceptor/, crates/brassclaw_reborn/,
crates/brassclaw_reborn_composition/, crates/brassclaw_agent_loop/,
crates/brassclaw_turns/, crates/brassclaw_pg/.
Work:
  - Step 5.5.0: Write V044__brassclaw_forensic_packets_alter.sql (ALTER existing
    V026 table — add component_refs JSONB + volatile_tail TEXT; keep prompt JSONB).
  - Step 5.5.1: Change on_prompt_assembled return type to Option<InterceptorResult>
    with adjusted_messages; update InterceptorPromptOutput + ModelInput +
    SempaiReviewOutcome (add adjusted_volatile_messages, bridge_messages,
    composition_summary, proposed_recipe_updates, proposed_intent_examples,
    settings_adjustments); update 6 test stub files.
  - Step 5.5.2: Wire PgInterceptorStore + allocate sempai_swappable +
    create SharedInterceptorMode in composition runtime.rs (replace interceptor_store:None).
  - Step 5.5.3: Add sempai_swappable + interceptor_mode to llm_config_service;
    set_active(Sempai) live-swap + mode flip.
  - Step 5.5.4: Sempai gateway + rerouting branch + 3-part prompt (Part A via
    direct SQL to individual component tables, Part B persona, Part C from
    matched_component_ids) + KV-cache pre-warm endpoint.
  - Step 5.5.5: InterceptorConfigService + reassemble_base_prompt() via direct SQL
    + ForensicPacket cleanup task; add prompts/sempai_audit.md.
  - Feature flag: interceptor (default off, gates wiring not V044 migration).
Commit + push.
Run: cargo test -p brassclaw_interceptor -p brassclaw_reborn -p brassclaw_reborn_composition
     -p brassclaw_agent_loop -p brassclaw_webui_v2
```

---

## Step 8 — Phase 6: Settings UI (10-tab editor)

**Scope:** WebUI v2 REST routes + React SPA: Skills/Actions/Tools/Extensions/
Orchestrator/Scaffold/Monty VM/4-queue Validation/Reliability/Interceptor Config tabs.

```
Implement plan step 8 (Phase 6 — Settings UI 10-tab editor).
Source files: crates/brassclaw_webui_v2/, crates/brassclaw_webui_v2_static/,
crates/brassclaw_product_workflow/.
Work:
  - REST routes: /api/settings/{skills,tools,extensions,actions,orchestrators,
    scaffolds,monty-vm} + POST /api/settings/monty-vm/restart + GET .../status.
  - Generalized validation-queue routes for all ~20 class codes (Q9):
    GET /api/webchat/v2/validation-queue?q={auto|manual|revision|rejection}
    + PUT /api/webchat/v2/components/{class_code}/{id}/validate (pops 05:validator)
    + PUT .../reject + PUT .../send-to-revision + PUT .../re-review
    + DELETE .../{id} + GET .../audit-status + GET .../revision-history.
  - PUT /api/chat/preferences/{key} (ai_before_user persistence).
  - React SPA: 10 tabs — Skills/Actions step-list editor (all 13 step types)/
    Tools/Extensions/Orchestrator/Scaffold/Monty VM/4-queue Validation/
    Reliability/Interceptor Config.
  - Tag chip greyed-out rendering; disambiguation UX (30s timeout); "AI before
    User" flip switch; Validation Config sub-panel per class code.
  - Interceptor Config tab: mode toggle, Sempai provider selector,
    reassemble + prewarm buttons, components_since_rebuild badge.
Commit + push.
Run: cargo test -p brassclaw_webui_v2 -p brassclaw_product_workflow
```

---

## Step 9 — Phase 7: Final cleanup

**Scope:** Delete retired paths. Grep-verify all dead code is gone.

```
Implement plan step 9 (Phase 7 — Cleanup).
Work:
  - Delete remaining skills/*/SKILL.md on-disk discovery code.
  - Delete migrated_skills.rs + bundled_skills.rs v1→v2 blob migration.
  - Delete v1 skill shim + skill_migration.rs bridge.
  - Grep-confirm no remaining: signals_tool_intent, signals_execution_intent,
    llm_signals_tool_intent, user_signals_execution_intent, score_skill,
    extract_explicit_skills, format_docs, format_skills, append_system_append,
    DocType::, SkillTrust in production code.
  - Grep-confirm doc_type_weight/keyword_match_score/extract_keywords gone from
    DB-mode retrieval.rs (must exist only in retrieval_dbless.rs).
  - Demote BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS to DB-less fallback only.
  - Update AGENTS.md + CLAUDE.md + CHANGELOG.md.
  - Remove old recipe/tool_skill-specific validation route aliases.
Commit + push.
Run: cargo clippy --all --benches --tests --examples --all-features -- -D warnings
     cargo test
```

---

## Notes

- **OOM prevention:** each step above is scoped to one phase. Never load all
  10 step descriptions into a single agent context simultaneously.
- **Commit discipline:** commit after every sub-step. Push at the end of each
  numbered step. Use messages like `feat: Phase 1 Step 1.1 — V027 reborn_skills migration`.
- **Reference docs:** `plan.md` for detailed step requirements; `spec.md` for
  glossary, security invariants (§6.1), and open-question resolutions (§7).
- **Clippy rule:** zero warnings — `cargo clippy --all ... -- -D warnings` must
  pass before any push.
- **No .unwrap()/.expect()** in production code (tests and infallible
  compiled-in literals are exempt with a safety comment).
