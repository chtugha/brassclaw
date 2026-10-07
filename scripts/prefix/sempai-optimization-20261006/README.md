# Sempai prefix optimization evidence — 2026-10-06

This folder preserves the optimization runs, including failures. Generated
components are unapproved exports: no database submission, production Q1, human
Q2, activation or Monty execution was performed. Passing finite probes cannot
establish universally flawless model behavior.

## Generations and controls

| Generation | Text tokens | Shared tokens | Cacheable tokens | Change |
| --- | ---: | ---: | ---: | --- |
| v2 `acb23be2…` | 69,948 | 69,954 | 69,696 | Decision procedure at both boundaries; blocked proposals and submission claims clarified. |
| v3 `9220198f…` | 71,992 | 71,998 | 71,808 | Raw validation and complete-program examples also at both boundaries. |
| v4 `a19662e5…` | 73,120 | 73,126 | 72,864 | Canonical once-decoded Python-in-JSON; bounded drafting, helper definition, module scope and cardinality rules. |
| v5 `bd9bfe8c…` | 73,513 | 73,519 | 72,864 | Reviewed boundary procedure presented directly as instructions; complete payload before factual summary. |
| v6 `7152e501…` | 75,666 | 75,672 | 74,976 | Quoted boundary layout restored; three complete, independently checked program patterns. |
| v7 `d9cc59d5…` | 75,962 | 75,968 | 74,976 | Exact result shapes; conversation count constrained by the test protocol. |
| v8a `1b7c3c11…` | 76,376 | 76,382 | 76,032 | Draft versus activation prerequisites; full offline Recipe export; nonempty example retained outside repeated generic checkpoint. |

All generations use the same pinned tokenizer, model, 131,072-token context and
existing service configuration. `--max-num-batched-tokens` stays 8192. Changes to
the reference override were backed up by the existing deployment helper. The
base service file, environment and Compose deployment were not replaced.

All original source units were preserved: 250 in v2–v5, 251 in v6/v7 and 252 in v8/v8a. Reviewed
model-visible policies select 122 units in v2–v5 and 123 in v6/v7 and 124 in v8/v8a. Selection retains the complete Recipe, Skill, Tool and ToolSkill
guides, reviewer procedure and proposal transport. The other units remain in the
original documents; they are not secretly summarized or deleted. Every source
inventory resolves to byte-identical originals under `source-documents/`.
Policy refreshes are explicit reviewed revisions, not approval of components.

## First-pass comparisons

The first-pass runs below supply neither a previous failed response nor behavioral feedback. Separate feedback runs, when present, are identified explicitly.
The same six regression prompts are retained from the preceding evaluation.
Six additional cases test unknown provider facts, different scalar/list/boolean
contracts, helper invocation, completed effects and an authorized prompt repair.
Three later confirmation cases test enum matching, missing versus null and nested
objects. Their field names/bounds are not worked examples in the prefix.

| Run | Completed cases | Exact envelope/conversation/count gates | Candidate checks |
| --- | ---: | ---: | --- |
| v2 JSON object, thinking disabled | 12 | 11/12 | 2/6 accepted offline; missing access, Raise, misleading intents and malformed envelope rejected. |
| v3 JSON object, thinking disabled | 12 | 11/12 | 2/6 accepted offline; retry envelope, misleading intents, empty-list rule and undefined helper rejected. |
| v3 JSON schema, thinking enabled | 8 | 6/8 | Port and timeout drafts pass; retry/Recipe exhaust 8192 output tokens. |
| v4 JSON schema, thinking disabled | 15 | 15/15 | 4/9 accepted offline; incomplete helpers, misleading Recipe intents, empty-list and missing/null errors rejected. |
| v5 JSON schema, thinking disabled | 15 | 11/15 | 3/9 requested draft cases accepted offline; empty conversations, absent proposal, incomplete helpers and intent/data errors rejected. |
| v6 JSON schema, thinking disabled | 15 | 14/15 | 6/9 accepted offline; extra retry result fields, classifier type errors and truncated/fabricated Recipe response rejected. |
| v7 JSON schema + fixed message count, thinking disabled | 15 | 14/15 | 5/9 accepted offline; incomplete helper, empty-list error, lossy Recipe fields and false prerequisite refusal rejected. |
| v8a JSON schema + fixed message count, thinking disabled | 15 | 14/15 | 6/9 accepted offline; 129/144 Python behavior probes pass. Recipe intent/defaults, helper return and list cardinality failures retained. |

The thinking comparison was stopped during case 09 after two output-budget
exhaustions. Cases 10–12 were not run in that comparison. See its explicit
`interruption.json`; interrupted cases are not successes or model failures.
Schema-constrained decoding guarantees neither code semantics nor completion.
The retry reasoning repeatedly reconsidered Python/JSON serialization; that
observation motivated the v4 presentation change.

## Cache measurements

Exact automatic injection was verified against server token IDs for three request
shapes. The clients sent no copy of the reference or chat-template override.

| Generation | Cold one-token probe | Warm one-token probe | Native cache hit delta |
| --- | ---: | ---: | ---: |
| v2 | 31.319 s | 0.651 s | 69,696 tokens |
| v3 | 32.671 s | 0.632 s | 71,808 tokens |
| v4 | 33.323 s | 0.664 s | 72,864 tokens |
| v5 | 33.498 s | 0.907 s | 72,864 tokens |
| v6 | 34.854 s | 0.945 s | 74,976 tokens |
| v7 | 34.941 s | 1.099 s | 74,976 tokens |
| v8a | 35.160 s | 0.744 s | 76,032 tokens |

These are single operational probes, not a controlled throughput benchmark. Native
hybrid cache reuse is observed; separate inspection of recurrent-state contents,
external LMCache persistence and reuse after eviction/restart is not established.
External cache hits were zero.

## Validation boundaries and reproduction

`evaluate_sempai_prefix.py` records raw requests/responses and validates the exact
review envelope, requested proposal count and unchanged/authorized conversation.
`check_sempai_candidates.py` executes AST-allowlisted pure logic in Linux isolated
CPython children with time/memory bounds. It checks missing/null/types/bounds,
duplicate names, complete programs and the requested helper invocation. The
Recipe probe checks persisted IBS shape, exact fixture UUID, variant/step link,
intent meaning and unapproved-state notices. The UUID is an offline fixture,
not an approved catalogue component. These checks are not Monty or production Q1.

```sh
python3 scripts/prefix/evaluate_sempai_prefix.py \
  --server http://192.168.10.171:8000 \
  --output ./sempai-new-run --schema-mode
# Linux only; run after the evaluator finishes.
python3 scripts/prefix/check_sempai_candidates.py ./sempai-new-run
```

For a separate thinking comparison add `--thinking` and use a new output folder.
Do not infer production readiness from a valid JSON object or cache hit. The
current proposal sink accepts classes 21/22, drops essential v3 Recipe fields and
cannot preserve a full v3 design. Actual queue transitions, exact-combination
approval, selected typed runner, Tool policy and human Q2 remain prerequisites.

The immutable manifests pin compiler, source selection, rendered template and
token hashes. Historical compiler/evaluator copies identify the tested revisions;
raw failures are retained alongside accepted exports. The local source-preservation
test and real compilation ran successfully. A test invocation without a tokenizer
initially failed after a temporary rendering assertion; that assertion was repaired
to keep the source-only test independent of tokenizer setup. No model result was
changed to repair the harness.

The checker was corrected to permit a helper called in a module-level condition,
not only a direct `result = helper(...)` assignment, and safe dictionary read
methods. The original narrower AST gate falsely rejected valid drafts. Their
unchanged programs now pass real behavior checks. A helper-only result assignment
is rejected at preflight; six real checker/CPython self-checks passed. These are
harness corrections, not changes to model replies or task semantics.

The v5 direct-instruction presentation regressed evidence preservation, so v6
restores quoted checkpoints and adds complete generic program patterns. All three
new source programs passed 57 independent Linux behavior checks before inclusion;
these are documentation-quality evidence, not model test results.

The v7/v8a test protocol constrains adjusted-message count using the provider
JSON schema. Roles/content are still independently checked; constrained length
is not evidence of unconstrained conversation preservation. Preliminary v8 was
compiled but never deployed or tested. Its manifests and originals are retained.

The v8a prerequisite correction was verified against `pg_python_code_store.rs`: a
new class-22 draft needs no existing UUID; insertion returns its assigned identity
before queue submission. Activation and approved dependency reuse still require
their own review evidence. Unknown Kohai provider facts do not erase known reviewer
constructors. Full offline Recipe designs retain fields the current live sink drops.

Current served generation is v8a `1b7c3c117e57e5c0efb3355afcc7b40ff73a07251677af8235494908771881b3`.
All fifteen first-pass conversations satisfy preservation/authorized-repair checks;
the Recipe case fails the separate empty-compatibility-array constraint. All eight
requested Python drafts were emitted. Six pass all behavior cases; timeout helper
lacks a return (0/14) and weighted entries misses maximum cardinality (23/24).
The Recipe retains full v3 fields but its steps/defaults and intent semantics are
wrong. Its factual summary incorrectly says steps is empty. Those failures are
preserved in `sempai-optimization-v8a-schema-count-20261006/`.

The checker now checks the explicit offline constructor defaults, equality of
constructor/variant intent examples and absence of fixture Tool calls/dependencies.
The historical checker used for v2–v7 is retained separately. These added checks
apply the same original case requirements; they do not redefine successful output.

## Explicit feedback repairs

`sempai-optimization-v8a-feedback-20261006/` supplies the retained failed replies
and case-specific independent-check feedback. Both Python repairs pass (timeout
14/14; weighted entries 24/24). Together with the six accepted first-pass programs,
all eight requested Python candidates have a passing export covering 144 probes;
this is mixed first-pass/repair evidence, not 144/144 first-pass success. The Recipe
constructor defaults and root compatibility arrays were corrected, but its rejected
speech/posting intent examples were copied unchanged, so it remains rejected.
The exact feedback is archived alongside raw requests and responses.

The second Recipe feedback round fixes speech/posting intent meaning, but emits
ten entries containing one duplicate (nine distinct intents), and again fills the
root compatibility intent array. Both failures remain rejected. No failed or
repaired response has been relabeled as a first-pass success.

The third Recipe feedback round passes all twelve offline design checks and the
review-envelope/conversation checks. It remains an unapproved offline export
because the live sink drops v3 fields. `accepted-candidate-index.json` identifies
the nine passing exports: six first-pass and three feedback-repaired, preserving
all intervening rejected replies. This demonstrates bounded candidate generation
with validation/repair; it does not establish universal flawlessness or production
Q1/Q2/Monty acceptance. vLLM and LMCache remain active; the base unit checksum is
unchanged. Batch tokens remain 8192.
