# Modal build lifetime and remote observation — 2026-11-16

The operator-selected provider Function limit is 1800 seconds (30 minutes).
`timeout_secs` remains the compatible configuration/CLI/request name, but is no
longer used to construct a Cargo deadline. The worker's threading.Timer and
120-second deduction are removed. Source preparation, the main compilation and
the normal companion compilation share the actual Function lifetime. Independent
communication timeouts, compiler-error cleanup and native test deadlines remain.

The controller samples the original App/container approximately every 60 seconds
while polling the same FunctionCall. A manual `inspect-running <job>` uses the
same observer without taking the build's exclusive source/cache lock. The worker
records the actual Popen PID and Linux start ticks so exec transitions such as
Clippy do not defeat matching, and PID reuse is not attributed to the job.
Inspection reads process states, CPU ticks and at most 16 KiB of Cargo log bytes;
it reads no environment or process argv. JSON metadata must be regular,
non-symlink files with unique keys and bounded size. Named pipes cannot block
inspection. Observations are explicitly not progress or completion receipts.

The Modal 1.6.3 CLI's actual `container list --app-id ... --env ... --json` keys
and `container exec --no-pty ... -- ...` grammar were inspected in the installed
SDK. Both CLI operations have independent 30-second communication timeouts.
Observation failures are retained and printed without stopping or replaying
work. Manual and automatic observers have unique atomic staging and individual
historical files. Existing Screen notifications retain their 180-second default
and five-minute missing-confirmed-heartbeat investigation rule.

A terminal Function error without an original worker completion receipt remains
unresolved; the existing unsettled-job guard blocks subsequent remote work.
Durable original completion can still be reconciled and collected. Historical
pre-forwarding and pre-observation worker identities retain collection-only
compatibility; they cannot activate or execute the new worker protocol. Current
worker identity is checked for both Apple and Linux before synchronization and
again under the remote lock, and by the actual worker before compilation.

## Validation and limits

Final scoped evidence is in
`target/validation/modal-monitoring-20261116-r2/`:

- 64 controller/settings/collection tests passed.
- 27 worker/inspection tests passed.
- Six source-sync and 55 Screen/notification tests from `r1` remain applicable:
  their complete relevant inputs are unchanged and verified by SHA-256.
- Total: 152 passing cases, none failed or skipped. Modified inputs remained
  unchanged during the final checks. Python syntax and `git diff --check` passed.
- Both actual supervising Screen sessions exited zero, their final notifications
  were accepted and received, and their recorded socket directory was empty.

The tests cover elapsed preparation without a derived Cargo cutoff, companion
execution, provider configuration bounds, observation failures without replay,
the observation cadence, wrong App/job/container refusal, exec/PID identity,
FIFO/symlink/duplicate/oversized metadata refusal, unchanged original collection,
and unresolved terminal-result custody. Process parsing uses Linux `/proc`
fixtures on the local macOS test host; this is not a claimed live remote build.
No project Cargo compilation or PostgreSQL instance was needed for these
Python build-tool changes. Screen tests use their private simulated IPC peer,
while the supervising sessions use the real Desktop IPC connection.

At the end of this initial source-only checkpoint, the immutable Apple image still
contained the historical worker. It was
not rebuilt, activated, relabelled or silently upgraded by source sync during
this change. Actual `activation-status` now reports it DEACTIVATED for this worker
protocol. Image preparation and the next useful authorized build must use the
current worker before remote execution resumes. No unsuccessful/uncertain
previous build was automatically resubmitted. This record does not qualify
native Linux tests, a product release or the complete Bob plans.

The subsequent [live image activation](2026-11-16-modal-worker-activation.md)
supersedes that deployment status. It also fixes the SDK CLI's short-command
output-drain race with an authenticated, version-checked PIPE adapter. The actual
post-activation build and observations passed; this earlier evidence and its
source-only scope are retained, rather than rewritten as a remote acceptance.
