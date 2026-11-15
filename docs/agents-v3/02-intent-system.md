# 02 — Intent system

## 1. Purpose

Intent matching selects an eligible Recipe workflow or reports disambiguation
or actual No-Match. It does not execute components, approve revisions or grant
Tool permission. Ordinary retained routing uses the catalogue owner's coherent
active approved selection, not every mutable intent row. This source map was
checked on 2026-11-15 against [recipe.md](../../recipe.md),
[Simplified v3](../../simplified_v3.md) and [the roadmap](../../path_to_v3.5.md).

## 2. Location — verified sources

| Source | Symbols / responsibility |
| --- | --- |
| [intent_system.rs](../../crates/brassclaw_engine/src/memory/intent_system.rs) | `classify_query`, `match_order`, `IntentResolution`, `IntentCandidate`, `RetainedIntentEligibility`, `resolve_intent`, `resolve_intent_in_transaction`, `resolve_catalogue_intent_in_transaction`, `record_disambiguation_choice`, `seed_intent_input`, `class_label`. |
| [installed_monty_catalogue.rs](../../crates/brassclaw_reborn_composition/src/installed_monty_catalogue.rs) | Captures and verifies a catalogue view, derives eligible retained instructions and resolves matching in a repeatable-read transaction; `TaskCatalogue` serves that captured result. |
| [retained_instruction.rs](../../crates/brassclaw_engine/src/memory/retained_instruction.rs) | `compile_matched_retained_recipe` checks exact Recipe/variant intent membership rather than choosing the first compatible-looking variant. |
| [global_recipe_ports.rs](../../crates/brassclaw_reborn_composition/src/global_recipe_ports.rs) | `MontyIntentSelection` and `MontyTaskCatalogue` carry routing through the global task boundary. |
| [global_mode.py](../../crates/brassclaw_engine/orchestrator/global_mode.py) | `_execute_task` distinguishes routing outcomes; `_execute_recipe` and `_non_match` own their execution paths. |

Storage originated in [V028](../../crates/brassclaw_pg/migrations/V028__reborn_intent_inputs.sql),
with variant links in [V054](../../crates/brassclaw_pg/migrations/V054__reborn_intent_inputs_step_link.sql)
and template metadata in [V076](../../crates/brassclaw_pg/migrations/V076__reborn_intent_inputs_template.sql).
Those migrations are history, not the whole retained selection contract.

## 3. Data model

`InputClass` has `Word=1`, `Partial=2`, `Sentence=3`, `KeywordFallback=4`.
`classify_query` emits sentence for at least five whitespace tokens or terminal
`.` / `!` / `?`, partial for two to four tokens otherwise, and word otherwise.
Keyword fallback is not a classifier output. `match_order` prefers `3,2,1` for
sentence, `2,3,1` for partial and `1,2,3` for word/fallback.

The actual `IntentResolution` variants are:

- `Match { component_id, component_class_code, step_link, input_text, is_template }`.
  A link is optional in this compatibility type; ordinary retained Recipe selection
  requires its exact supported Recipe workflow identity.
- `Disambiguation { candidates }`, with candidates carrying `row_id`, component
  UUID/class, input class, score, class label and optional step link.
- `NoMatch`.

`component_name` is not a current field of `IntentResolution::Match`. Do not
copy historical JSON/type examples that include it. A component UUID or shared
step link alone does not identify a retained variant/input layout.

`reborn_intent_inputs` retains the existing scope tuple, text/class, component
identity, score/provenance/review state and template/link metadata. Existing
scope keys describe present storage; they do not add final-v3 operator roles or
Tool permission checks. The instance-wide policy contract is separate.

## 4. Ordinary behavior and failure boundary

1. The installed catalogue owner captures its active view and verifies that
   source selection in the same transaction used for matching. It builds
   `RetainedIntentEligibility` from selected instructions. Eligibility restricts
   matching before ranking; drafts/stale rows cannot displace eligible workflows.
2. `resolve_catalogue_intent_in_transaction` requires repeatable-read or
   serializable isolation. SQL/snapshot failures propagate as errors. The matcher
   ranks exact/template candidates and distinct workflows using the existing
   classifier and score spread; no fixed top-three claim is part of this guide.
3. `TaskCatalogue::resolve_intent` serves the captured resolution only for the
   original query. Its source explicitly rejects rematching or latest lookup
   during child execution. A match binds the original selected variant example
   into typed task inputs.
4. Retained compilation checks the Recipe UUID/class, exact link and intent
   expression against its immutable embedded variants. Missing or ambiguous
   membership is an error, not an invitation to choose the first variant.
5. Only a successful `NoMatch` result permits the non-matching model path.
   Database, catalogue-integrity, matching, composition and begun execution
   errors never become No-Match or authorize Tier-2 replay.

For a standalone legacy lookup, `resolve_intent` also increments the selected
row's score. `resolve_intent_in_transaction` does not commit or mutate score
telemetry; its caller supplies the coherent view. Neither helper alone approves
or activates a catalogue. Score increments are capped at 100 and rate-limited to
50 per existing scope/hour; these are telemetry safeguards, not task/token caps.

`record_disambiguation_choice` verifies row UUID, component UUID/class and all
existing scope fields under a row lock, scores and commits, and returns the
stored template/link. That standalone helper does not establish the complete
retained catalogue selection or permit an already selected task to rematch.

## 5. Compatibility readers

[PostgresSource::fetch_for_turn](../../crates/brassclaw_engine/src/memory/retrieval_source.rs)
uses the standalone matcher for its retrieval path. Its current source propagates
matching errors and enters keyword `fetch_for_consumer` only on No-Match; no
second score increment is needed. This retrieval helper is distinct from the
ordinary task's captured catalogue routing.

The legacy [orchestrator bridge](../../crates/brassclaw_engine/src/executor/orchestrator.rs)
returns an error for a missing composition port or input. It must not manufacture
No-Match from unavailability. These host-handler details do not replace the
ordinary global task adapter described in [12-agent-loop.md](12-agent-loop.md).

## 6. Support and remaining work

Coherent retained routing already has source support; it is not an entirely
missing V35-02 feature. V35-01/02 reconcile legacy descriptions and exact support;
new publication/compatibility and remaining runtime acceptance stay with their
existing catalogue, validator and Simplified owners. Use
[the implementation index](../implementation-status.md) for accepted scope.
Public workflows need genuine per-variant positive/negative intent coverage and
at least ten Recipe examples; private workflows qualify trusted typed admission.
MCP exposure separately requires a qualified command Recipe/variant.

## 7. LLM-relevant summary

Match only eligible workflows from the task's coherent active approved catalogue.
Keep exact Recipe/variant/template/link and typed captures. Disambiguation is
explicit. Only actual No-Match enters Tier 2; technical errors and begun failures
remain errors. Scoring is telemetry, binding is not authority, and resumption or
child execution never selects latest again.
