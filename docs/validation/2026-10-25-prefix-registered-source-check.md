# Registered-source diagnostic Recipe — 2026-10-25

> **26 October evidence correction:** the historical 530-file worker-reuse
> inventory did not cover all actual build dependencies. Observed outcomes and
> immutable receipts remain, but full unchanged-worker source applicability is
> unproven. See [the exact limitation and scoped current requalification](2026-10-26-prefix-worker-evidence-review.md).

**Scoped candidate acceptance passed.** The existing HTTP source diagnostic
Recipe now has a `registered` variant with 16 explicit component steps. It reads
a pinned registration, resolves its stored web URL, reuses the existing bounded
HTTP/digest/classification sequence, retains exact unqualified diagnostic bytes
and returns the native storage receipt. The original `selected` variant retains
its step definitions, input layout and invocations unchanged.

The four new Python exports handle parsing, registration/result agreement,
diagnostic serialization and receipt projection. Rust constructs declarative
component/Recipe records. Existing native HTTP, JSON and source-storage Tools
provide the four effects, each with its own binding/executable pair. Both source
usages share one Tool identity, and their binding steps come from their actual
canonical Recipes. No storage table, HTTP endpoint, provider service or approval
path was added.

## Identity and evidence boundaries

Task input is canonical JSON with only the exact source identity, explicit response
byte limit and timeout. URL and expected-hash overrides are rejected. The actual
native read result supplies the reference; mismatched pins or unsupported source
kinds fail before HTTP. A registration checksum identifies registration metadata,
not fetched content. This fresh-registration variant deliberately has no retained
content baseline: a nonempty HTTP 200 response remains neutral/unverified rather
than Available or approved. The final projection rejects upgraded qualification.

The existing observation primitive has a bounded metadata row. The new serializer
stores classification, pinned identity/reference, status, representation, byte
counts, redaction flag and complete text digest without placing the entire response
in that row. The complete raw response remains available in the actual execution
result. The native test independently verifies its full bytes and UTF-8 hash, even
when larger than the observation capacity. This is checking, not collection or
archival of new knowledge; it does not make fetched content eligible for generation.
Ordinary durable native-answer custody still requires the existing admitted adapter.

## Actual validation

Receipt: `target/accepted-evidence/prefix-registered-source-20261025/run-5/accepted.json`.
SHA-256: `427295d5f294c88f32f8fe7141b9c49b5cc59021bff36fa0120263350cac8720`.
The four exact changed source files, actual compiled executable, original serial
runner, companion worker and raw logs are retained beside it. All 530 recorded
worker source inputs matched before exact worker reuse. The canonical target and
shared Cargo lock were used; capacity stayed within the required disk gate.

**Seven selected Rust tests freshly passed.** Every run verified its expected
positive test count and zero ignored cases:

- Two new candidate tests. The full retained Recipe executes in contained Monty
  tasks through the actual kernel/native HTTP, digest and source handlers with a
  real loopback server and PostgreSQL. Five execution cases cover complete large
  text, 404, wrong registration pin, response overflow and a live HTTP block. The
  last three stop at the expected read/HTTP step without a follow-up observation
  or automatic Recipe retry. Successful cases read back exact immutable diagnostic
  bytes/checksum; Sources availability stays disabled/not-checked. Ten additional
  pure handoff cases execute in real Monty and check valid data, URL/hash override
  rejection, mismatched/non-web registration rejection and forged green/trusted
  qualification rejection. Both Recipe variants pass actual preparation/preload.
- Three existing HTTP/binding/classification tests freshly pass, including exact
  full content, live policy, transport bounds, redirects and the identity matrix.
- One existing canonical source-usage preload test and one actual PostgreSQL
  storage immutability/idempotency test freshly pass.

Current strict Skills library, Composition tests with the affected features and
default Composition library checks pass with `-D warnings`, together with format,
whitespace and final source integrity. No extra Root/provider/UI/model suite is
claimed. The contained test driver follows the selected Recipe's retained order
and typed references through `RetainedRecipeExecution`; its native adapter forwards
real kernel results. It supplies no fabricated HTTP success or storage receipt.
It is a private behavioral observer, not the ordinary job/admission adapter or a
new production workflow loop.

Earlier attempts remain preserved. Run 1 rejected a test snapshot that omitted the
four reused canonical Recipe roots; the fixture now retains all five roots and
then selects the exact diagnostic root. Run 2 rejected a newly reconstructed
binding that used display name `prefix-source` instead of host alias
`prefix_source`; the builder now copies both actual canonical bindings. Run 3
exposed an invalid test-worker geometry: two maximum utility IPC frames plus the
adapter reserve left no VM heap. The fixture now derives a sufficient frame from
complete data and captured source/compiled/stdout bounds, explicitly reserving
both frames and adapter before assigning finite heap. Run 4 rejected an extra
parent-supplied `task_token`; the exact existing six-field admission envelope now
lets the worker create its own routing token. These failed runs are not counted
as complete behavioral acceptance. No production protocol or settings changed.

## Remaining integration

Exact graph/system-seed qualification, independent pure/association approval,
ordinary durable Source job admission and effect settlement, coalescing/cancel/
reconnect/restart and trusted availability projection remain open. Generation-used
originals still need integration through the already implemented generation reader
and their qualified source adapter; logical paths must not become guessed URLs or
registration foreign keys. Collection/archive/review and next-build inclusion are
separate. The closed source-compiler exception was not widened.

The no-repeat observations cover the tested wrong-pin, overflow and live-block
cases at Recipe level. They do not qualify general HTTP/2 NACK/transport replay,
unknown outcomes or crash recovery; those stay with the shared network/runtime
owners. No public Sources health, model design, Generate, serving, cache correctness,
held-out model quality or full-plan/platform/deployment acceptance is claimed.
See the [remaining Prefix sequence](../plans/prefix_v3_upgrade.md#remaining-implementation-sequence--full-completion).
