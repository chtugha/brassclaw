# Subplan — Step B: Dual-nature recipe syntax (human-readable explanation alongside the machine `step_link`)

> Sequenced **before C** (Model A retirement) and **A** (reshaped H.12.6), per
> user steer ("do B first and then C and then A").
>
> **Scope narrowed per user:** the `step_link` formula is good and **stays
> as-is**. The dual-nature requirement is satisfied by a concise
> **human-readable explanation of what happens** carried alongside the
> machine-readable `step_link` + IBS `step_descriptions` → `BuildInstruction`
> form. **Not too much detail.**

## Goal

Codify and close the dual-nature recipe convention: every recipe carries

1. **Machine-readable exact logic** = `RecipeVariant.step_link` +
   `Recipe.step_descriptions` → IBS `build_instruction` → `BuildInstruction`
   (`rust_steps` + `orchestrator_steps`). Deterministic, **untouched**.
2. **Human-readable explanation** = concise prose that says **what happens**
   (recipe-level, variant-level, step-level).

## Grounding (existing, stays)

- `Recipe.description` — recipe-level prose.
- `RecipeVariant.variant_key` — human-readable variant id (e.g. `"ls-la"`).
- `StepDescriptionEntry.label` + `StepEntry.goal` — step-level prose
  (`instruction_builder.rs`).
- `RecipeStep.description` — legacy step prose (Tier-1 injection).
- Machine form: `RecipeVariant.step_link` + `Recipe.step_descriptions` →
  `build_instruction` → `BuildInstruction`. **UNTOUCHED.**

## The one structural gap

`RecipeVariant` has **no prose description** — only `variant_key` (a short id).
A variant's "what happens" is not explicitly explained at the variant level.
(`crates/brassclaw_engine/src/types/recipe.rs`.)

## Proposed work

1. Add `RecipeVariant.description: Option<String>` (`#[serde(default)]` so
   legacy rows deserialise unchanged) — concise human-readable explanation of
   what this variant does.
2. **Q1 validation gate**: a new/graduating recipe must have non-empty
   `Recipe.description` AND non-empty `RecipeVariant.description` for every
   variant. Legacy rows exempt (serde default).
3. **WebUI authoring**: surface `RecipeVariant.description` alongside
   `variant_key` + `step_link` (read + edit).
4. **Docs**: record the dual-nature convention in `CLAUDE.md` and
   `docs/agents-v3/03-recipe-system.md`.

## DECISION (your call — do not implement until confirmed)

- **D-B1:** Add `RecipeVariant.description` as above? *(recommended)*
- **D-B2:** Required at Q1 for new/graduating recipes (legacy exempt), or
  optional? *(recommended: required, legacy exempt)*

## Out of scope (explicit)

- Changing the `step_link` formula or IBS `build_instruction` compilation.
- A new DSL / Markdown transpiler.
- Recipe-driven second VM for intent (future work — see mindmap §8).

## Steps (run after design confirmation)

- **B.1** — `RecipeVariant.description` + `#[serde(default)]` + unit
  round-trip test (`types/recipe.rs`).
- **B.2** — Q1 validation gate (`recipe_validator` / `component_validator`) +
  test through the caller (integration tier).
- **B.3** — WebUI authoring surface (`descriptors.rs` / `handlers.rs`) + test.
- **B.4** — docs: `CLAUDE.md` dual-nature note + `03-recipe-system.md`.
- **B.5** — verify both configs green (default + `--features skills-db`); mark
  B done; commit + push; then proceed to **C**.

## Completion log

- **B.1 — DONE.** Added `RecipeVariant.description: Option<String>`
  (`#[serde(default)]`) in `crates/brassclaw_engine/src/types/recipe.rs` + 2 unit
  round-trip tests (`recipe_variant_description_round_trips`,
  `recipe_variant_description_legacy_defaults_to_none`). `step_link`/IBS
  untouched.
- **B.2 — DONE.** Q1 gate `check_variant_descriptions` in `recipe_validator.rs`
  (v3 variants `[step_link present]` require non-empty `description` ≤ 512 chars;
  legacy `[step_link == None]` exempt) + 3 unit tests through `validate_recipe`
  + 1 integration test through `component_validator::validate_by_class(21, …)`
  (`class21_recipe_v3_variant_without_description_fails_q1_gate`).
- **B.3 — DONE (verification-only).** Read surface `RecipeDetail.recipe` is
  opaque full-engine JSON → `variants[].description` rides along automatically
  (covered by B.1 serialization). No WebUI recipe-authoring route exists today →
  edit surface is future work. No code change.
- **B.4 — DONE.** Dual-nature convention documented in `CLAUDE.md` (authoring
  rules) + `docs/agents-v3/03-recipe-system.md` §1.1.
- **B.5 — DONE.** Both configs green: default clippy clean + 599 lib tests pass;
  `--features skills-db` clippy clean + 610 lib tests pass.
  `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target`; scoped clean
  (Capacity 91% > 90%) before compile per the disk rule.

Step B complete. Proceeding to **C** (Model A retirement subplan).
