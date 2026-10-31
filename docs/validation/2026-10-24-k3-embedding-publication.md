# K3 — Actual embedding publication and independent role uptake, 2026-10-24

The bounded successor passes **56 actually executed Rust cases**, strict affected/
root test lints, default-composition lint, formatting and source/artifact integrity.
This includes the ordinary registered Memory Tool/PostgreSQL/WebUI caller and
the ordinary global Monty model caller. Thirty-two architecture cases execute
against the current tree with their unchanged retained executables. No ignored
or zero-case run is counted. Evidence is retained in
`target/accepted-evidence/k3-embedding-publication-20261024/run-1/`.
K3 and the priority gate remain open.

## Implemented and observed

The existing memory embedding source publishes its actual client and captured
provider-definition/model/authenticated configuration reference under the same
mutex. Compatible cache-control changes are applied before their capture is
acknowledged. The runtime privately retains the very same source installed in
the Memory slot; the ordinary settings facade reads its publication into the
existing `runtime_publications` map. No utilities/core dependency on concrete
providers, second registry, settings store or reload coordinator is introduced.

Attachment alone remains untracked. A successful read of an empty desired
selection publishes unconfigured. Failed resolution retires the old source
cache and reports untracked, never a newly effective client or successful
disable. These states describe publication, not remote availability or full
desired/effective/pending convergence. A settings read does not resolve a later
desired credential and relabel it as already published.

The initial suspicion that definition edits omitted embedding refresh was
incorrect: `refresh_running_provider` already performed it indirectly. The
actual defect was unwanted role coupling and short-circuiting. A pure Kohai
selection now refreshes that role independently of a broken embedding selection.
Definition/authentication edits attempt all existing role owners even when
another owner fails. The original committed mutation remains independently
readable and the writer still sees incomplete takeover as an error.

The expanded actual registered-Memory/PG/WebUI case observes initial publication,
cache-only revisions without replacing the client, unchanged state after real
write rejection, key-only changes before/after actual publication, failed
resolution and confirmed disabling. With independently invalid embedding
metadata, a pure primary selection succeeds. A later definition commit reports
the embedding uptake failure while still publishing the actual new primary
revision. Original readback proves that exact commit. Repairing the embedding
selection then reconciling the original mutation returns its original receipt,
without another definition revision, inference request or effect replay.
This real caller passes in 71.26 seconds.

Existing original-mutation, role/service, model-limit, primary/Sempai publication
and live-cache cases also pass. The ordinary global caller passes in 35.34
seconds and retains the global service and controlled HTTP protocol exchanges.
That routing fixture is not actual Ornith inference evidence. These checks
cover the shared runtime/facade changes; this package introduces no JavaScript
change and counts no new UI test execution.

## Identities and remaining work

- accepted.json: `75c90e69434251ed4bfe1fb74464eca099bea759cd1d88094b250a9885a3cb1e`.
- source.json: `1c739ba1d10fda2333e831880ae3d9778eecade0cfb08b9eaade0a525a04e350`.
- validated-publication-source.json: `0b5d22e5aac747af67e11308f0cc4697173b44cc95ea2dde00e017b2e1c43f3f`.
- Actual composition executable: `a1fdf1f66082d37bf00ec2d609938edf9b699a83de722b749283a601489486f3`.

Six changed compiler inputs are preserved as immutable evidence deltas. The 530
worker inputs remain unchanged and the actual executable is hash checked. Cargo
runs serially in screen with the inherited canonical target. Executables are
preserved before required targeted disk cleanup. The operator-selected Ornith
endpoint/model and its reported 131072-token capacity are checked explicitly;
nothing is compiled on the test machine.

Full desired/effective/pending coordination, role-selection concurrency/CAS,
task-wide model/prefix bindings over subsequent steps/waits, other dedicated
protocol/session boundaries and K4–K7 remain open. The last-publication map is
not a task checkpoint, Tool grant or full-v3/deployment acceptance. Historical
[native submission-fence acceptance](2026-10-24-k3-submission-fence.md) retains its
exact prior source and three real model-custody receipts; this successor does
not relabel them as fresh execution against the changed runtime.
