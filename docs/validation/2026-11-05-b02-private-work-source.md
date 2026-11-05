# B-02 private Work — source checkpoint, not acceptance

2026-11-05. Scope: genuine message-free private Work for the existing structural
validator and quarantined source-only prefix compiler, in the existing PostgreSQL
turn owner, global Monty driver, native custody and worker. V207 is exclusively
reserved for this package. Historical migrations and approval bytes are unchanged.

The shared acknowledged PreparationCapture API was isolated in commit
8bc93bd29276bcb6bea31e3deca34b10055d8d31, on dd8307eb5. Main documentation successors
52df32658/6a440d715 were mechanically incorporated. This later checkpoint preserves
the in-progress connected source before integrating A-03's complete dependency
chain through 06a53dbba0bb54b5b466ab19dd42cc0f92814c89.

Source changes cover neutral private origin, original/FK/Ready-CAS and real PG
claims/reservations; explicit Chat/Private host and input; the existing shared
worker/fallback/heartbeat; original input/definition retention; pre-I/O submission
intent; actual native or sealed-unsubmitted terminal settlement; versioned prefix
origins/status projections. Workflow execution stays in the existing Recipes and
Python engine. There is no second work queue, VM, domain scheduler or LISTEN service.

This is **not** a complete connected package or a compiler/DB/runtime receipt.
A-03 domain input Prepare/Restore calls now target its concrete published API and
require that actual dependency integration. Structural production submission and
all native acceptance sources, caller/feature/type/SQL/lock/warning review remain
unfinished. No stub result or cfg(test) claim/host replaces the required real path.

Performed: source inspection and reasoning, standalone rustfmt and diff whitespace
checks. Not performed: Cargo, DB/schema execution, tests, Modal, paid setup/build.
Compilation and DB/test work remain held until the supervisor reviews the complete
connected diff. The package owns only its branch/worktree, never main integration.

Artifact scope remains explicit: the existing catalogue/global owner retains the
actual captured PackagedExecutableImage Arc and native implementation images for
the live service; A-03 restore receives the real verified worker copy, not a path
or checksum pretending to load an old binary. A differing old worker/root is
rejected before effects. Durable old-native loading/upgrade and general proof
restore are not established by this checkpoint and remain B3 gates. No second
artifact registry or loader has been invented to conceal that boundary.

External handoffs: /private/tmp/brassclaw-b02-contract-20261105, especially
PREPARATION-CAPTURE-API-1.md and LIVE-PRIVATE-INTEGRATION-3.md. Approval/activation,
GeneralPureApproval, prefix serving and complete B3 recovery remain separate gates.

## Connected source update after A03 integration

The complete A03 histories through 06a53dbba0bb54b5b466ab19dd42cc0f92814c89
and its source-close correction 71ad805478f4c7e4143b5fa709757d223d31e932
are integrated by merges 5edd9122a and 2831a1bbf. Both catalogue preparation/context
and A03 startup executor/resource-store inputs are preserved. The latter startup
values are bundled at their existing constructor, without another settings owner.
A03 contained-source close semantics are unchanged; chat-only readers explicitly
require their actual optional chat context. Native invocation lookup uses attempt ID.

Structural production submission now registers native delivery before publication,
uses the actual A03 PreparedValidatorReview serializer, captures acknowledged task
preparation once, and commits Work/original definition/Ready together through the
existing PgTurnStateStore transaction API. Original reads avoid recapturing current
limits. The existing Worker owns claim, heartbeat, capacity and fallback polling.
The existing runtime wake sender is bound to the catalogue; no new notifier loop
or domain queue is introduced. ValidatorTaskDriver awaits the original native
completion and authenticates its exact private original in one read transaction.
The obsolete synthetic validator-chat constructor/variant and string-prefix
validator dispatch branch are removed. Other contained/source/model workflows
retain their actual existing chat semantics and A03 close implementation.

Private native delivery now pins the same slot for the producer's complete lifetime,
including the interval after factory retention release and before publication.
Waiter disposal cannot remove an active producer's slot. Dropped producer futures
publish only an explicit interrupted/unresolved error; they cannot construct native
proof, start/cancel work or write settlement. The actual Worker's addressed stop /
fence path ends producer retention and resolves a never-polled producer's waiter.
Claimed/terminal late waiters check delivery/producer lifetime under the same slot
lock as producer registration; SQL terminal data never restores opaque proof.

WorkHeartbeat::OriginalSettled is a distinct read-only result for the exact original
claim after Work/Admission/immutable terminal coherently settled. The PG reader
checks tenant/work/run/runner/token hash/generation/request/definition and linked
settled Admission/terminal checksum. It renews no lease and grants no dispatch
permission. The existing heartbeat future waits for its existing cancellation
while the original driver delivers completion; outer Worker cancellation remains
active. Missing/conflicting/unresolved terminals fail closed. This avoids treating
the healthy post-settlement delivery interval as lease loss.

V207 additionally checks A03's actual capture fields against the immutable domain
selection, subject, original preparation, raw UTF-8 input SHA and inner workflow
SHA. This is relational integrity, not a second domain input serializer or approval.
Historical migrations remain untouched. SQL execution/NULL/constraint behavior has
not yet been accepted. Prefix origin's large preparation value is boxed without
changing its immutable wire bytes. Remaining source-visible old Reservation and
TaskEnvelope chat test consumers were adapted to their real tagged variants.

Native acceptance sources added/updated (all **unexecuted**):

- run_q1_validation_retained_passes_and_persists_evidence and
  run_q1_validation_retained_fails_for_empty_content now use actual private Work.
- private_validator_delivers_after_factory_release_to_original_and_late_waiters:
  pauses actual publication after native factory release, keeps healthy settlement
  pending across five configured Worker heartbeat periods, and reaches a second
  actual ValidatorTaskDriver::submit before releasing the same opaque result.
- private_validator_worker_shutdown_delivers_unresolved_without_replay: actual
  Worker cancellation drops its producer while the original native delivery waits.
- private_validator_heartbeat_loss_delivers_unresolved_without_replay: pauses the
  actual producer after real claim/reservation, before drive_host; an actual PG
  trigger rejects renewal of still-claimed Work, and the real Worker stops/fences
  the original to reconciliation. No fake heartbeat/claim/receipt is constructed.
- private_source_compiler_uses_shared_worker_without_messages_and_keeps_original:
  actual source collector/compiler/Worker/global root; rejects malformed source
  before Work, drops every waiter while the producer is held, registers a late
  waiter and authenticates actual native completion. It checks versioned private
  projection, no new threads/receipts, one claim/admission/terminal and same-ID readback.

These sources are not execution evidence. Additional real concurrency/CAS/claim,
unknown-COMMIT, before-I/O unsubmitted, cold Ready restore, fallback/missed wake,
second-process/capacity/reconnect and live cancel-during-open-work cases remain
required for their respective claims. A03's existing input restore case proves
same-process byte/settings restore, not process restart or durable proof restore.
No Cargo/DB/tests/Modal were run; source review still precedes compilation.

### Exact B3-06/07 submission gap remains open

The original V206 submission/queue transaction in pg_review_submission /
queued_review_submission / ValidationQueueStore is earlier than Work preparation.
ValidationQueueStore::fire_q1_kick_if_wired uses an existing detached tokio::spawn;
run_q1_validation_retained retains the original validator selection before calling
the new private producer. No startup sweep in runtime currently bridges a lost
original kick into absent Work. The existing TurnRunnerWorker startup/fallback
reads Ready Work and ordinary queued runs; it cannot discover a review original
that has no Work, nor prepare arbitrary AwaitingPreparation rows. Consequently,
original queue durability plus this live entry does not prove durable restart
scheduling from the original submission commit. Work/definition/Ready are atomic
only once the new structural preparation producer is reached.

Likewise, source-only prefix definition retention precedes private Work publication.
An explicit retry retains the exact existing definition/preparation and can prepare
it without recapturing sources; absent-Work startup discovery is not implemented.
The remaining connected cutover belongs to the existing original submission/queue
writers, Q1 retained preparation/startup attachment, source-only prefix submission
and shared Worker owner, with a coherent original/Work transaction or retained
preparation handoff. It must not add another poller/domain scheduler, replay native
work or infer a completed proof from stored JSON. This checkpoint does not close
B3-06/07 or full durable scheduling/recovery acceptance.

## Acceptance correction 5 — additional native matrix sources

Root requested four connected acceptance boundaries before its B02 build. The
additional sources are in runtime/tests/private_work_admission_tests.rs. They have
**not been compiled or run**. Exact selectors:

1. `private_prepare_claim_cas_and_chat_execution_collision_preserve_one_original`
   uses two actual concurrent producers, rejects a conflicting request and a stale
   Ready CAS, then races two real PgTurnStateStore claims with the real native
   reservation owner. It requires one claim/admission/native terminal, validates
   the actual opaque completion against its original, and also checks cancellation
   before claim. A genuine ordinary inbound/queued chat supplies the execution-ID
   collision; the private original must fail before any native admission/effect.
2. `private_ready_and_claim_lost_commit_ack_keep_original_native_reservation`
   uses the existing AckCutTransport to withhold actual Ready and claim COMMIT
   replies. Original readback must retain identical definition bytes and the real
   reservation/host/token address captured before writes. Another claim returns
   no Ready original; only an explicit test-supervisor continuation of that same
   authenticated still-unsubmitted handle performs its first native dispatch.
   A third cut loses the pre-service submission-intent COMMIT reply: the real
   service issues its own sealed unsubmitted proof before any enqueue. The test
   requires one settled original and rejects a second drive/claim, even with the
   committed submission-intent marker. This is not general native transport ACK
   recovery or a SQL-to-proof restore implementation.
3. `private_cancel_before_submit_and_running_native_task_keep_exact_terminals`
   cancels the real claimed Work after real private preparation but before the
   first service submission, requiring the actual sealed-unsubmitted terminal.
   It then starts a native task and blocks its actual selection journal INSERT
   at PostgreSQL, observes the real started Admission/backend lock, cancels the
   current Work and polls the actual addressed stop before releasing the lock.
   The expected native cancellation terminal belongs to that original claim;
   no Tool dispatch or validator success may appear, and no original is replayed.
   The running case is before the first Tool effect, not external-effect recovery.
4. `private_same_id_capture_race_ready_reconstruction_and_worker_fallback_keep_original_bytes`
   holds the existing reserve-owner PG boundary so two public same-ID submissions
   both observe initially absent metadata. Between captures it publishes and
   verifies a real ACK settings change whose smaller fresh limit rejects the same
   source if incorrectly reapplied. Both calls must retain the first complete
   program/input/preparation original. Changed payload/owner/target or a real Chat
   origin still conflicts; full Origin capture equality remains strict.
   After another ACK change, a fresh actual catalogue/factory/host/driver is built
   using the same retained worker Arc and existing global service. It executes the
   persisted Ready original with byte-identical definition/program/input capture.
   This is catalogue/host reconstruction in the same process and same worker/root,
   not process restart or loading a different historical artifact. Finally the
   actual Worker passes its existing startup gate through the existing real
   loopback listener, completes an empty scan and consumes its initial wake. A
   second original is published through a catalogue with no wake binding; the
   configured existing Worker fallback must claim and execute it on the same root.

The reservation observer forwards to OwnedGlobalTaskFactory and retains the exact
returned PgPrivateClaimReservation Arc. It creates no claim, admission, native
answer or permission. The only added native barrier extends the existing cfg(test)
delivery pause to the boundary before first service submission; no product or SDK
API, replacement host, fake backend, second queue/VM/scheduler or recovery loop was
added. Test-only timeouts bound observations; production cancellation uses its
existing acknowledged policy.

The matrix exposed/addresses two connected source defects. Request identity now
compares exact origin kind and real scope/context independently from new offered
internal preparation bytes, while Origin::PartialEq keeps complete capture equality.
The private immutable-prefix reserve writer serializes same-ID first captures with
a transaction advisory lock, reads the winner before fresh validation, and validates
fresh canonical source only when no original exists. The public submission boundary
therefore cannot apply smaller newly captured limits to a concurrently won original.
Legacy Chat reserve locking/limits semantics remain unchanged. Prefix Work insertion,
complete immutable preparation and Ready seal now share the existing owner's single
ReadCommitted transaction and Work row lock; an identical producer reads its committed
winner rather than competing with a stale RR Ready CAS. The selected root/program /
approval references are exact immutable originals; this introduces no latest lookup.

Source-visible test caller repairs use the tagged legacy thread scope and remove
an invalid private-runtime-field access from the separate preparation-codec test.
Same-global-root assertions remain in actual private native execution cases.

Performed only: source/type/ownership/lock review, standalone rustfmt and whitespace
diff checks. No Cargo, DB/schema, native tests, remote tests or Modal execution.
Second-process/restart/different-artifact loading, unknown native transport recovery,
external-effect cancellation/reconciliation and original-commit-to-absent-Work
startup dispatch remain unaccepted/open. The original submission/queue and prefix
metadata commit still precede the new atomic Work+Ready transaction: B3-06/07 are
not closed by these tests or source fixes. Root's separate C00 baseline PASS does
not supply A03/B02 execution evidence.

The same fourth selector also covers the authoritative collector entry, not just
the explicit-snapshot boundary. It pauses the existing real source-capture seam
after the API's absent-original read, lets a genuine first captured original win,
then releases fresh collection under the acknowledged smaller limits. That public
request must authenticate/read its exact original rather than reject it using the
new capture. Collected internal bytes may differ while explicit profile/revision /
owner/capture-kind remains the same; the explicit-snapshot API still requires exact
supplied payload. The existing collector API now performs a bounded original
readback after a concurrent collection/reservation failure, using its same original
request validation and Work owner. It never retries a native task or restores proof.
The test uses the existing cfg(test) catalogue capture gate and executes that other
Ready original before checking the empty-queue/fallback boundary.

The first three new selectors hold the actual Worker's existing startup gate and
exercise real PgTurnStateStore/native-driver owners directly, retaining opaque
claims from actual locked SQL transitions. The last selector releases the real
Worker and observes its actual fallback. These are distinct claims: they do not
pretend that every direct-owner failure case has already run through the complete
Worker integration or that a second operating-system process was exercised.
