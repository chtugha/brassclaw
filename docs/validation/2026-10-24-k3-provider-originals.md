# K3 — Original manual provider publications, 2026-10-24

The first connected K3 package is accepted for the existing manual provider
upsert, deactivate and role-selection paths. **K3 remains open:** actual effective
client revisions, desired/effective status, external-writer freshness, retained/
cache/retry dispatch fencing and the remaining Prefix consumer integration follow
in the same provider/configuration owners. K4–K7 and regular plan work remain open.
This is neither release/platform acceptance nor a testmachine deployment.

## Connected change

V178 records the original nonempty mutation UUID, operation, typed nonsecret
request, audit actor, exact provider-definition revision and committed role
projection inside the existing definition/key/selection transaction. A request
proof including credential identity is authenticated and encrypted in the existing
SecretStore. No plaintext key or publicly guessable key hash is placed in the
provider receipt. Canonical recursive JSON ordering keeps proof bytes stable
across JSONB and serde feature orderings. Equal identities serialize before lookup;
changed content conflicts, and exact repeats do not create another revision,
rotate keys, reactivate a provider or restore old selections over later edits.
Key-only edits can retain the same definition revision; the distinct original
receipt still identifies the publication.

Lost or failed commits are reconciled through the original identity. The existing
facade exposes authenticated original GET and reference-only POST reconcile.
Reconcile reads current desired configuration; it never republishes the original
values over a later writer. A role selection refreshes only its affected role,
preserving unrelated admitted clients. Existing embedding factories validate the
selected embedding contract before committing it. Backend errors from the primary
reload adapter become fixed safe messages.

The existing browser transport keeps original payloads only in memory and reuses
their UUID while outcome is unknown. Confirmed persistence is separate from failed
runtime reload: identical requests reconcile, while explicit corrective content
can obtain a new identity after commitment is known. Provider views await canonical
Query invalidation rather than installing mutation snapshots into shared data.
The model-limit editor retains confirmed receipts across reload/refetch failures.
Prefix model-design reads fence late stale responses/errors. No provider registry,
event queue, browser secret persistence or new Prefix serving pipeline is added.

## Observed checks

Final batch: `target/accepted-evidence/k3-provider-originals-20261024/run-7/`.

- **38 freshly executed Rust tests:** actual Memory-Embedding and model-discovery
  callers, provider HTTP contract, two descriptors, 32 architecture checks against
  the current tree, and the full existing native HTTP/PG/global-Monty/Prefix caller.
- **19 exactly applicable run-6 Rust results:** three original-request tests
  (canonical proof and actual PostgreSQL acknowledgement/concurrency/rollback/
  reload cases), ten existing service tests, model-limit and request-policy
  callers, startup/Sempai role isolation, retained in-flight global model switch,
  cache policy and embedding model contracts. Original logs and actual executable
  are retained; run-6 is still a failed batch.
- **26 current UI cases and seven syntax checks**, also qualified from run-6's
  unchanged sources. The status tests use the actual shipped QueryClient and
  QueryObserver; controlled transport/hook scheduling proves UI behavior, not PG
  commits or client uptake.
- Strict root/composition/product/WebUI/PG/secrets test Clippy under the common
  feature configuration, default-composition library Clippy, format, whitespace
  and final source/artifact identity checks pass without suppressed warnings.

The real original-request caller cuts five actual PostgreSQL COMMIT replies:
create, key-only update, deactivate, explicit re-add and role select. It covers
same-ID changed key, original reads/retries/reconcile, later independent edits,
service reconstruction, wrong master/namespace and unchanged ciphertext on replay.
The other real PG caller verifies concurrent conflicting originals, transaction
rollback, persisted original after actual reload failure, correction and immutable
records. No stub establishes these claims.

The full actual HTTP/Prefix caller passed in **343.284 seconds**. Complete package
sources, six retained original model artifacts and the existing offline Hugging
Face reference environment are used. Largest Sempai diagnostic: **371,684 tokens**,
above its declared 262,144 context. The warning and `fit=false` remain visible;
there is no source truncation, limit increase, prefix approval or serving claim.

## Preserved failed attempts

Run-1 failed at the generic transaction reader; the caller now passes the
underlying Tokio transaction. Run-2 compiled but exposed five visibility warnings
and invalid short test master keys; these were corrected rather than suppressed.
Run-3 passed original ACK/service/limit checks but found a request ID accidentally
added to a ProviderDefinition fixture; only that invalid field was removed.
Run-4 exposed the real unrelated-Sempai refresh regression, repaired by selecting
only the original affected role. Run-5 failed before Cargo because its runner
incorrectly changed a historical worker-evidence path. Run-6 passed the 19 reused
Rust results and current UI checks but found a remaining Memory-Embedding test
request without mutation_id. The caller audit found one equivalent model-discovery
edit. Run-7 proves that exactly those two fixture payloads changed, with production
and previously passed caller/UI sources unchanged, and executes both repaired
callers plus all remaining checks. Original exes, sources and raw logs remain.

## Exact identities

- `accepted.json`: `bdf40f057010bc4be4861521bfe5ed6b88b6cf896f258e80d3418b7dd35e673f`.
- `source.json`: `e2f8d69873dc6fddeaadbec9fdfba91530c43ad5625cb22d2464e098c0d240ec`.
- `reused-evidence.json`: `b05761984093b929e33f95d323a4525125e94e976c98668f8c241cdfd5a62936`.
- Fresh composition test executable: `5cb024fb57bd8354110265019b0a7d6a4be3fbbc1bcafc97e46da94c55277d99`.
- Actual run-6 executable for reused results: `be9ce49567b302a7f8f07c872c397669ef569b3c19493511d87bd5648ddd1900`.
- Adjacent retained worker: `d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.

Worker reuse verifies its 530 original compiler inputs. The architecture exes are
unchanged qualified originals executed against current sources. All runtime inputs
and 270 declared release documents were checked before subsequent edits. Cargo ran
serially in background screen under the shared lock with the configured canonical
target and actual exes preserved before required targeted cleanup. This report and
later status edits are a subsequent documentation snapshot, not retroactively
embedded in the accepted binaries. No regular application build is certified.
