# Parallel validation for the CI and CLI repair packages

User instruction, 2026-11-16: run independent Modal package builds concurrently,
with a Screen session per package. Implement repairs serially; parallel validation
does not authorize concurrent source writers or a shared mutable Cargo target.

1. Finish/review the source deltas before taking the snapshot. Reuse the accepted
   32-case Monty admission result; do not rebuild it for a second success message.
2. The leading controller holds the existing global Modal lock for the entire
   batch. Reconcile all original outstanding jobs first. Synchronize/verify one
   source snapshot once; never publish another snapshot into that batch.
3. Run two fixed, explicitly commissioned qualification functions concurrently:
   the existing activated Apple CLI worker and native Linux `utility_host`
   acceptance. Use their existing different immutable images and different
   repository/target cache directories. The unchanged shared Cargo registry keeps
   Cargo's own download lock; targets and source directories are not shared.
4. Use the current worker/materializer and original-request/result collectors.
   Preserve a distinct UUID/request/source/result/artifact record for each job.
   This explicit scoped batch does not activate a new generic test mode, change
   the ordinary controller grammar, or bypass another job's lock.
5. Associate each actual job with its own Screen. The second Screen observes its
   original FunctionCall; it never submits a replacement. Keep the owning App and
   global lock alive until both actual jobs are terminal. Observe actual remote
   processes/logs independently of Screen heartbeats. Unknown results keep their
   original unresolved state and prevent another batch.
6. Collect immutable artifacts directly to the existing SSDE validation evidence
   directory, retaining canonical requests/state in the existing registry. Preserve
   disk gates, provider limits, independent communication/test deadlines and
   180-second Screen notifications. Never replay successful work after a collection
   failure; recollect only its retained original result.
7. Repair observed causes and finish each bounded acceptance. Further CI database
   cases use genuine dedicated temporary instances and the existing fixture
   owner. Qualify Linux non-root execution and required fixtures before submitting
   them; do not launch a root-owned `initdb`, fabricate approval, restore startup
   requalification or claim compilation as native test acceptance.

Status, 2026-11-16: the scoped jobs and actual collectors are connected. Native
utility acceptance passes all fourteen cases; the Apple product build completes.
The first two jobs were initially serialized by the environment's actual
`max_concurrent_tasks = 1`, despite separate Function definitions. The operator's
parallel-build instruction is applied by raising only that CPU-task cap to four
in environment `brassclaw`; its GPU cap remains one. Actual container identities
and original process logs subsequently prove concurrent execution on independent
volumes. Native CLI acceptance passes all six selected cases. Reusing that exact
CLI program, all 68 remaining smoke cases pass without compiling or replaying the
four successful smoke cases. Updated strict CLI/Composition all-targets Clippy
with `postgres,skills-db` passes without warnings. The remaining Composition cases
retain their separate requests and results.

Composition's two Cargo phases completed before their old 30-minute Function
limit expired. Their exact source and actual Cargo artifact records are verified,
and the existing test/normal-worker executables are retained once with checksums.
New execution-only calls use the operator's 90-minute provider limit and separate
temporary non-root PostgreSQL fixtures. No recompilation is needed for unchanged
cases. An initial case exposed a genuine missing development-qualification scope;
its failed result is retained. A helper tuple/JSON-list comparison failure ended
the other calls; the stopped App, absent containers and terminal originals are
reconciled before explicitly restarting only those interrupted fixture cases.
Case collection now preserves other running calls even if one collection fails.
These are scoped development functions, not a generic Linux/test-mode activation.
V3.51 implementation is not part of this subplan and remains last in priority.
