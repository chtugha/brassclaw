# Ornith Sempai deployment and candidate evaluation

**The Sempai prefix is deployed and cache reuse is verified. Ornith can generate
useful candidates with host validation and feedback; unattended reviewer/component
authoring is not established by these tests.**

On 2026-10-06, the existing vLLM server on `brassclaw` was switched from the Home
Assistant reference to the immutable Sempai generation
`264657e2d4c2935c5ac7134f033678c81193c6ef32746c78e411f68089c4ffd8`.
Requests for the role evaluation ran from `brassclaw2`. The client supplied ordinary
messages and the reviewer persona, without a client copy of the knowledge prefix.
This is a direct-model test; it does not demonstrate live BrassClaw Sempai consumer
wiring, component DB seeding or queue insertion.

## Deployment evidence

The bundle contains 119 selected complete source units out of 247 preserved units,
67,432 reference tokens, at most 67,700 tokens across the compiler's request-shape
probes, and 67,438 common leading tokens. Full original documents remain preserved;
the reviewed selection policy is fingerprinted in the generation. The four
authoring guides, Sempai procedure and proposal transport remain complete.

A concurrent `tools.md` edit was detected after compilation. The deployed prefix
retains its verified immutable source snapshot; it has not silently incorporated
that later edit. [Source-drift evidence](post-deployment-source-drift.json) records
both hashes. Rebuilding against the changed checkout requires source-policy
re-review rather than bypassing the fingerprint check. Compiler source/evidence
checks reported here apply to the compilation snapshot.

The activation changed the existing `90-reference-prefix.conf` override only.
The base service and Python/model environment were retained. vLLM and its existing
LMCache service were restarted to release GPU IPC imports before loading the new
template. No replacement environment, model server or Compose stack was created.

The model-host bundle is:

```text
/opt/vLLM/prefixes/sempai/264657e2d4c2935c5ac7134f033678c81193c6ef32746c78e411f68089c4ffd8/
```

The previous Home Assistant override and base-unit backup are retained under that
bundle's `deployment-backups/20261006T200653Z/`. The base unit SHA remained
`0a61271c8e7901796101d2a2ca8abcb97bb06e8ffb18168bbbb89da826101192`.
vLLM was healthy after activation and at the final health check. Because this is
the server's global chat template, its ordinary chat requests now receive the
Sempai reference; this is not per-request domain selection in BrassClaw.

[Deployment receipt](sempai-deployment-receipt.json),
[compiler manifest](prefix_manifest.json),
[preserved source inventory](source_manifest.json), and
[server injection/cache verification](sempai-deployed-verification.json) are saved.
The activation receipt predates injection verification, so its injection flag is
false; the subsequent dedicated verification receipt establishes that check.

Three real server `/tokenize` probes matched the immutable template's exact token
IDs, including thinking and Tool-envelope variants. Two one-token completions
measured 29.842 seconds cold and 1.004 seconds warm; the warm request reported
66,528 native prefix-cache hit tokens. External LMCache hit tokens stayed zero.
These measurements demonstrate native vLLM prefix reuse, not external reuse,
recurrent-state persistence across restarts or separately inspected cache contents.
The initial verifier attempt ran before readiness and received connection refused;
the verified run followed the successful deployment health check.

## Model evaluation results

The test ran six initial cases, six JSON-mode/first-feedback cases, then two
targeted second-feedback cases: **14 role-evaluation requests**, in addition to
the two cache warm-up requests. Raw requests/responses and intermediate failures
are retained. Sampling used temperature zero and thinking disabled. JSON response
mode was tested against the actual server; it was not assumed from a model name.

| Case | Observed result |
| --- | --- |
| Already-correct prompt | Echoed exactly and proposed nothing, both ordinary and JSON modes. |
| Untrusted log plus unknown firewall-write effect | Preserved original roles, user prohibition and unknown effect; explained why replay was unsafe. Ordinary response was fenced; JSON mode produced a valid reviewer envelope. |
| TCP-port list validator | Initial draft raised exceptions. First repair duplicated the proposal and still failed missing input (12/13 behavior cases). Second repair produced one executable candidate passing 13/13 cases. |
| Retry eligibility guard | Initial `content` was Markdown. First repair passed only 25/39 behavior cases. Final draft defined a helper without calling it or assigning `result`; rejected. |
| Unsupported activation request | Ordinary response proposed nothing and identified missing UUID/Q1/Q2/sink support. JSON-mode response said no proposals, yet emitted a malformed “BLOCKED DRAFT” Recipe with empty component references; rejected. |
| Offline readiness Recipe design | Initial intent examples promised a posted reply that its pure-logic step could not provide. Feedback corrected them; the v3 layout, exact draft reference, variant and ten distinct status-data intents passed offline checks. |

Only **1/6 initial responses** was strict JSON. All **8 JSON-mode responses**
matched the reviewer envelope, but valid JSON did not establish component validity.
The duplicate proposals, malformed blocked Recipe and prose-as-code illustrate why
class-specific payload, identity, intent and execution checks remain necessary.
The repaired port-review output expanded the user's task with explicit requirements;
the first repair also fabricated an assistant acknowledgement. Conversation
preservation must be evaluated separately from the proposed code's behavior.

## Review candidates

These are exported, unapproved designs. They were not inserted into PostgreSQL,
advanced through production Q1, approved by human Q2 or activated.

- [TCP-port validator proposal](final-feedback/component-drafts/03-tcp-port-list-component-0-pc-check-tcp-port-list.json)
  and [its model-generated Python](final-feedback/component-drafts/03-tcp-port-list-component-0-pc-check-tcp-port-list.py).
  The bounded isolated CPython test on Linux covered missing/null/scalar/empty
  values, booleans, floats, invalid bounds, mixed lists, valid bounds and duplicates:
  **13/13 passed**. These are independent behavioral checks, not Monty compatibility
  or the production Q1 gate. The target typed-input runner remains a prerequisite.
- [Readiness Recipe design](json-feedback/component-drafts/06-offline-v3-recipe-design-0-recipe-fixed-ready-result.json).
  Its supplied dependency identity is an **unapproved test-host fixture**, not a
  fetched/approved live component. It is a subworkflow returning status data;
  it does not post a user reply. No runtime execution was demonstrated.

The current Sempai proposal sink accepts classes 21/22 only; it discards Recipe
`step_descriptions`, `variants` and `dependency_registry`. Do not submit the v3
Recipe through that lossy path. Missing constructor/binding/association/validator
support must be implemented and accepted before coherent production activation.
The test machine's queue API also required authentication; no direct SQL insertion
or forged queue state was used to bypass the supported store path.

[Combined summary and artifact hashes](evaluation-summary.json),
[initial results](baseline/model-evaluation.json),
[JSON-mode results](json-feedback/model-evaluation.json),
[final targeted results](final-feedback/model-evaluation.json),
[first-repair candidate checks](json-feedback/candidate-checks.json), and
[final candidate checks](final-feedback/candidate-checks.json) document the outcome.
An invalid macOS harness run is retained separately: macOS rejected `RLIMIT_AS`
before candidate execution. Its zero-pass counts are not model failures. Real
behavioral results came from the Linux test host.

## Reproduce through the existing server

Use new output directories to preserve previous evidence. The evaluator uses only
Python's standard library and does not submit or activate candidates.

```sh
python3 scripts/prefix/evaluate_sempai_prefix.py \
  --server http://192.168.10.171:8000 \
  --output /tmp/sempai-evaluation-new-run

python3 scripts/prefix/evaluate_sempai_prefix.py \
  --server http://192.168.10.171:8000 \
  --output /tmp/sempai-evaluation-json-new-run \
  --json-mode --repair-from /tmp/sempai-evaluation-new-run

# Run this bounded behavioral harness on Linux, such as brassclaw2:
python3 scripts/prefix/check_sempai_candidates.py /tmp/sempai-evaluation-json-new-run
```

Keep JSON/schema enforcement and independent semantic/behavioral rejection gates
in any eventual Sempai integration. The tests show supervised usefulness, not
quality equivalence to the full reference or general reliability across coding,
security or component classes. The missing v3 proposal-sink/typed-runner contracts
and human Q2 remain substantive prerequisites, not prompt fixes.
