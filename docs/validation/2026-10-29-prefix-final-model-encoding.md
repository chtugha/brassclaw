# Contained final-model encoding prerequisite — 29 October 2026

**Scoped prerequisite, not ordinary full-request acceptance.** The concrete
`ContainedFinalRequestMeasurer` now uses the existing bounded model-encoding
worker and immutable original tokenizer/default-template artifacts. Its native
integration reaches the actual SDK body and canonical PostgreSQL hard-limit
guard. The test's acceptance verifier is explicitly a fixture: real checked
pair/profile/current-generation acceptance and ordinary role attachment remain
implementation work.

## Connected behavior

The versioned `openai-text-chat-context/1` mapping rejects duplicate JSON keys,
unknown or unsupported request modes, missing/ambiguous output reservations and
unqualified media. Text-array joins, assistant nulls, Tool argument interpretation
and forwarded template controls require explicit mapping rules. They are not
inferred from a model name. Mapping alternatives require actual provider parity
qualification before production use.

Protocol 3 retains the original request/mapping/target identities and renders the
complete context with the original template. It verifies exact leading prefix
bytes, encodes the full input once and partitions those actual token IDs. A token
crossing the required boundary or unmappable offsets blocks the measurement;
separately tokenizing substrings cannot establish the partition. Original protocol
1/2 report shapes remain unchanged.

The native counter checks target, generation selection, original assets, worker,
runtime preparation revision, request/context/mapping digests, full counts and
prefix identity. Existing resource accounting includes failed worker work and
releases reservations on cancellation. Canonical provider hard limits remain in
their existing owner and apply independently of optional task budgets.

The actual SDK integration exposed redundant equal-model overrides producing
duplicate root model keys. Both completion paths now omit that redundant override.
Guarded calls also require the immutable selected client's model to match. The
strict parser remains strict; differing-model legacy behavior is not qualified by
this fix.

The retained Ornith default template requires a user query. The independent test
uses a legal complete conversation, preserves the reference and verifies the
fixed boundary without changing the template. A tools-mode case with no asserted
fixed prefix proves encoding parity only; it does not qualify the no-tools fixed
prefix for templates that render dynamic tools before it. Transformers emits its
model-type compatibility warning; it remains in the original oracle logs.

## Observation and evidence

R7 completed successfully: **11 fresh Rust cases, zero ignored**, comprising
three independent offline Ornith/HF comparisons, one actual SDK/native-worker/PG
counter integration, four existing encoding cases, two canonical PG hard-limit
cases and one ordinary global Monty regression (85.35 seconds). Fifteen encoder
cases and twelve SDK/transport cases are reused from the exact unchanged images
and applicable source closure, explicitly recorded rather than rerun. This is
38 scoped positive results, not 38 fresh executions.

Affected connected and default-feature strict Clippy, format and whitespace pass.
After termination, current protected source bytes and all executable hashes were
independently verified, own processes were absent and the common lock was free.

The immutable batch archive is
`/private/tmp/brassclaw-final-counter-validation-20261029/run-7/`;
`accepted.json` SHA-256 is
`a2af2da3849c997a08b58c843b88845686e8a485663401862aa3021875dfca51`,
`source.json` SHA-256 is
`fc530ff31a29e72d46f5855c218ccb33f185c7ff3823140bd2dfbf9dfa6c5163`.
Commands, features, profiles, source manifests, per-case counts, original logs and
reused evidence are in that archive. Actual protocol-3 worker SHA-256 is
`91a2165fa396dfd9513d15e886eaba2ed692f1a5b9ac472ef3fb734391e47343`.
The complete thirteen-path own manifest is `core-manifest-r7.json`, SHA-256
`572519fedf55ffa09d00faa43ff8453830113e9c701b35b571d92108af952094`.
Root source inputs are compilation inputs here, not independent Auth acceptance.

All seven runs are retained, including the illegal system-only probe, Python
quoting failure, feature-gate/fixture corrections and artifact-collection failure.
Successful results do not erase those diagnostics. The earlier
[SDK transport receipt](2026-10-28-prefix-final-request-guard.md) remains historical
and is not rewritten as real-tokenizer evidence.

## Remaining gates

Implement the checked canonical acceptance writer/reader and task-captured
binding verifier, qualify actual client defaults and provider rendering, then
attach the same guard to both ordinary Kohai/Sempai targets and continuations.
Exercise their complete request sequence with accepted current generations,
disabled discretionary budgets, late changes, history/Tools/output, retained
selection isolation and confirmed local rejection before submission.

Private scope-migration/sidecar proposals are unexecuted preparation, not accepted
schema or serving artifacts. No new approval/settings registry, mutable cap copy,
remote service change, provider template change, cache warmup or deployment is
part of this prerequisite.
