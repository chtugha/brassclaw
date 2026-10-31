# K3 — Displayed role selection and canonical target publication, 2026-10-24

The bounded package passes **63 executed Rust cases**: 57 fresh executions and
six precisely qualified reused executions. The 43 UI contract cases and twelve
JavaScript syntax checks from run-1 remain applicable through exact source
checks. Strict affected/root test lints, default-composition lint, formatting and
source/artifact integrity pass. Final evidence is retained in
`target/accepted-evidence/k3-role-selection-20261024/run-3/`.
The two preceding failed packages remain failed; their passing scopes are
identified individually. K3 and the conformity gate remain open.

## Implemented boundary

The existing settings snapshot now returns exact role-selection content
references from its same PostgreSQL repeatable-read view. They include complete
role EAV values and distinguish absent and empty fields. They are optimistic
edit preconditions, not monotonic activation epochs, task pins or Tool grants.
There is no new revision table, provider registry, event queue or periodic monitor.

Every new explicit role edit requires the displayed selection reference and,
for a nonempty provider choice, its displayed definition revision. Atomic
definition-plus-primary-selection writes require the same selection precondition.
The existing `db_config` owner compares current values under a short table write
lock in READ COMMITTED, covering absent keys and external SQL writers. Ordinary
configuration reads remain available. Provider rows precede the config lock;
no provider/network work occurs while that lock is held.

Provider deactivation now delegates its config cleanup to this same owner before
taking config row locks. This prevents a supported deactivation/other-provider
selection lock cycle. Alias cleanup and unrelated settings remain intact.
Explicit canonical provider/model selection removes legacy text-role `base_url`
and `api_key_env` overrides, so preparation and the published client use the same
declared target. Retained existing calls are not redirected.

Original committed mutations are looked up before fresh preconditions. Omitted
new fields preserve old canonical request-proof bytes; a real persisted legacy
select without the new reference is replayed as its original receipt. Missing
preconditions for new requests fail rather than loading a fresh precondition
silently or replacing the request identity.

The UI keeps displayed references with provider views and captures dialog
references/defaults when opening. Background refresh no longer resets that
dialog through a freshly allocated empty override object or newer defaults.
Definition editing preserves an explicitly selected model. A labelled option
with English/German help requests atomic selection while saving; onboarding
shows that selection explicitly and uses one atomic operation. It reports Use
failures instead of navigating after a rejected selection. The API helper never
fetches a newer precondition on behalf of an old user intent. These GUI changes
have source/contract/syntax coverage here, not rendered-browser or installed QA.

## Actual acceptance and failure repair

The new PostgreSQL case uses two independent service instances, actual encrypted
receipts and real locks. It observes a conflict after a change to an empty role's
legacy fields, canonical target cleanup, legacy original replay, exactly one
winner for concurrent edits, independent role edits, a pending external writer,
missing preconditions, repeatable-read rejection and both real config barriers
for deactivation versus selecting another provider. It passes in 6.89 seconds.

A separately enabled real-model case publishes primary and Sempai through the
actual API/reload owners and obtains actual Ornith content and measured usage
from both clients. Selecting Sempai preserves the primary client; a stale role
reference cannot clear it. It passes in 15.95 seconds. This is provider-client
acceptance, not full Sempai workflow/prefix-serving activation.

The ordinary global routing case passes in 35.45 seconds using controlled HTTP
protocol responses; it is distinct from actual Ornith inference evidence.
Sempai startup before ingress and subsequent actual settings uptake pass in
99.73 seconds. Existing role/service, live-cache, model-limit, descriptor and
boundary cases also pass. The actual Memory/PG/WebUI case remains applicable
from run-1's 71.23-second execution.

The full existing HTTP/Prefix consumer passes in 346.14 seconds. It retains all
319 declared release sources, original model files and exact measurements,
including reconnect without replay. Oversized candidates remain `fit=false`;
the 371733 > 262144 reference-tokenizer warning is preserved. No design approval,
generation acceptance or serving activation is inferred from those negative
measurements.

Run-1 failed because two new test master-key literals were shorter than the
existing crypto constructor's minimum. Both were corrected without changing
production validation. Run-2 then passed the new PG case but its positive
model fixture imposed a 256-token output ceiling. Actual HTTP/tokenization
diagnostics show a whole rendered request of 113600 tokens, `finish_reason=length`
and 256 reasoning tokens without final content. The same actual request without
that artificial fixture ceiling ends normally after 770 output tokens, with
nonempty content. Run-3 removes only that positive fixture's ceiling. No
production error, output validation or retry behavior is suppressed.
Those whole-request counts establish neither managed-prefix length/generation
nor a cache hit; the Prefix owner retains actual deployed-template qualification.

## Evidence identities and limits

- accepted.json: `6ba5c31b3ccf4a9526a4bea666e9c221a28b4b2ffdbee11bf6142c96f83128f5`.
- source.json: `0ba9847734d05e08515cbd97cfe26aac253239ffa310fcccb47dbfc8730b9b0d`.
- validated-role-source.json: `050b94d99a0e6903ff9136bfe40e2b8fd015813ace26438bb6a382bf70735ecf`.
- reused-r1-evidence.json: `51cb65e004f82ec2722480c99e1c7b9756b7bd3fafaab1ed876b0217c92fbe27`.
- reused-r2-cas-evidence.json: `2f817f2ddc6c53d7da76b53126c50adef64ede6ac5fe0a4850ecde9125338bd8`.
- Actual composition executable: `be2ce7e18a41e9cc3b9b9d2cc886c967a13f99555194f57e37eb6265f6fcd6cf`.
- Actual WebUI handler-contract executable: `3153a61ede21b15972bf47ae8774fff9479f2b81dc0e2f30891013a15de5babf`.

Eighteen changed Rust/JavaScript inputs are preserved as immutable evidence
deltas. Exact reverse reconstruction verifies the prior fixture source hashes;
all production and reused-case sources remain unchanged across the repairs.
Four individually passing cases from the failed run-1 harness, its actual Memory
case and the passed run-2 PG case account for the six reused Rust executions.
Thirty-two freshly executed architecture cases use their unchanged retained
executables against the current tree. The 530 worker inputs remain unchanged and
the actual worker is hash checked. Cargo runs serially in screen with the
inherited canonical target; executables are preserved before required cleanup.
No test-machine build or deployment occurs here.

Task-wide model/prefix bindings over later steps/waits/children, complete
desired/effective/pending status, dedicated auth/login and generic settings/import
writer convergence, other protocol/session boundaries and K4–K7 remain open.
Ordinary role API CAS does not certify those remaining writers, complete restart/
upgrade safety or full-v3/platform/installed acceptance.
