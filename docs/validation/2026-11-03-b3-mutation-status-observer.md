# B3-09 provider mutation status observer

Date: 2026-11-03. Scope: existing WebUI observer lifecycle and its shared
transport handoff, not native Auth, issuer, PostgreSQL or complete B3 acceptance.

`ProviderMutationStatus` previously had no unmount/auth-context response fence
and passed no cancellation/current-observer contract to the shared mutation
reader/reconciler. A response after navigation could still set result/error
state, emit shared bootstrap notifications or start a provider-query refresh.

The component now captures the existing original auth context, owns an
AbortController for the observation, and forwards its signal/current predicate
to the existing shared helpers. Unmount unsubscribes and aborts observation;
response/error handling checks the same context before publication. Duplicate
clicks retain one actual request, and a stale response cannot leave the mounted
component busy. This does not cancel/undo accepted server work, repeat the
original provider mutation or add another authentication/approval owner.

## Verification

The prepared patch was tested outside the checkout while another chat compiled:

```sh
PREFIX_STATIC_ROOT=/Volumes/SSDE/brassclaw/crates/brassclaw_webui_v2_static/static PREFIX_STATUS_SOURCE=/private/tmp/brassclaw-b3-09-mutation-observer/provider-mutation-status.js node --test /private/tmp/brassclaw-b3-09-mutation-observer/provider-mutation-status.test.mjs
```

Final result: 10 tests selected, 10 passed, zero failed/skipped/cancelled.
The suite uses the shipped QueryClient/QueryObserver. It covers canonical-query
ordering and visible refresh failure, original request identity, forwarded
fences, unmount, changed auth context, missing context and duplicate clicks.
An additional integration case crosses the actual status component, actual
shared mutation helper and actual apiFetch with controlled HTTP responses,
checking both absence of late bootstrap notification and one normal notification.
Controlled browser scheduling/HTTP inputs do not certify the native backend.

The final fixture extension was followed by the full ten-case run; earlier
nine-case results are superseded. Applying the patch compared exact before
hashes and verified that repository files equal the tested after hashes. No test
was repeated merely to move identical bytes from the scratch files to the repo.
The [source identity record](2026-11-03-b3-mutation-status-observer.json)
retains the exact before/after hashes and final scoped result.

## Isolation and remaining acceptance

No Cargo command was started. Both component/test files stayed outside the
checkout until the other local Cargo/test process was terminal. The Modal chat
confirmed that its previous remote job was terminal and its new run used fully
captured, isolated remote sources. Integration changed local files only: no
source sync, remote file mutation, remote restart or extra cloud build occurred.
That remote build's evidence remains limited to its original source manifest.

The Rust static bundle still needs a subsequent coordinated build to embed the
new UI bytes. No installed app, real browser, native PG/issuer path or full Bob
plan is declared accepted here. Existing validator/PostgreSQL/Modal files were
not modified by this work package.
