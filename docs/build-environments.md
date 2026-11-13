# Independent local and Modal builds

The authoritative checkout remains the repository root. One native local Cargo
execution and one Modal compilation may run concurrently. Each environment
serializes its own jobs; they do not share mutable compiler inputs or caches.
This supersedes the old global serial-build rule and temporary parallel-host
exception. The user's 2026-11-12 authorization permits test builds, tests and
other necessary Modal work when it credibly reduces total completion time,
without a per-batch question. See the current
[cost and validation policy](development-policy.md#modal-usage-and-cost-policy-updated-2026-11-12).
Activation and actual controller/runtime support remain prerequisites.

## Settings and commands

[scripts/build-environments.toml](../scripts/build-environments.toml) owns host
defaults. `[local]` controls local Cargo jobs; `[modal]` owns remote defaults;
`[modal.apple]` and `[modal.linux]` override only their target's remote defaults.
`--config /absolute/path/settings.toml` or `BRASSCLAW_BUILD_CONFIG` selects an
alternative complete configuration. Do not put credentials in that file.
Existing Cargo profiles, target-specific linker settings and host environment
remain in their current owners; no second Cargo target is introduced locally.

The workspace `Cargo.toml` sets the normal test profile to `opt-level = 1` across
workspace members and runtime dependencies, retaining the existing `sha2`
optimization override. Test debug assertions, overflow checks and unwinding
remain enabled; `.cargo/config.toml` retains debug level 1, unpacked debug
information and incremental compilation. Existing CI environment overrides for
debug information and incremental compilation still take precedence. Dev,
release and dist profiles are unchanged. The first test build after this change
recompiles affected dependencies and needs additional cache space; earlier warm
build timings do not estimate that rebuild. Keep the canonical target and
retained evidence. This addresses the distributed unoptimized contributions to
large native harnesses. The measured C-00 baseline passed its actual native
link, unwind-section and selected caller gates; see the
[scoped native profile record](validation/2026-11-05-c00-test-unwind-profile.md).
A later changed graph still needs its own warning-free link, retained unwind
sections and `__eh_frame` below 16 MiB. Configuration inspection alone does not
establish those acceptance gates or cross-platform runtime support.

| Setting | Meaning |
| --- | --- |
| Local `jobs` | Positive Cargo job count, or `"inherit"` to preserve `CARGO_BUILD_JOBS` and existing Cargo defaults. This is not an OS CPU/RAM reservation. |
| Modal `cpu` | Reserved physical CPU cores; establishes the minimum CPU billing basis. |
| Modal `cpu_limit` | Maximum permitted cores, at least the reservation. It permits additional utilization; availability and workload parallelism still matter. |
| Modal `memory_mib` | Reserved RAM in MiB. |
| Modal `memory_limit_mib` | Hard RAM ceiling, at least the reservation; exceeding it may terminate the worker. |
| Modal `jobs` | Concurrent Cargo jobs, separate from the CPU reservation/limit. More jobs do not guarantee more usable parallel work. |
| Modal `timeout_secs` | Function deadline; must preserve the existing shutdown/receipt margin. |
| Modal `shutdown_grace_secs` | Cargo termination grace before forced termination. |
| Modal `nonpreemptible` | Existing explicit interruption/cost tradeoff; the qualified profile retains `true`. |

Inspect settings without Cargo, cloud access or spending:

```sh
python scripts/local_build.py --show-settings
python scripts/modal_build.py --platform apple settings
```

Local example with an independent job count:

```sh
python scripts/local_build.py --jobs 4 -- cargo build --locked -p brassclaw_reborn_cli --bin brassclaw-reborn
```

The launcher invokes direct Cargo without a shell and inherits the Codex-supplied
`CARGO_TARGET_DIR`; it never sets a replacement target, contacts Modal or uploads
source. Direct Cargo remains valid when the caller already holds the native
build lock. Do not nest the launcher inside another owner of that same lock.
The launcher enforces the existing local compilation disk thresholds and refuses
whole-target cleanup, which would destroy retained evidence. Package cleanup
remains available through `cargo clean -p package_name` under the local lock.

The local launcher recognizes an optional leading `+toolchain`, then the
Cargo global switches `--offline`, `--locked`, `--frozen`, `-v`/`-vv`,
`--verbose`, `-q`/`--quiet` and split/inline `--color auto|always|never`.
It rejects aliases, external/unknown subcommands, unparsed top-level actions,
`--config`, `-C`, `-Z` and target/build/artifact-directory overrides. Unsupported
forms fail before execution; direct Cargo remains available to the supervisor
under the native lock. Package cleanup parses every argument and requires a
nonempty `-p foo`, `-pfoo`, `-p=foo`, `--package foo` or `--package=foo` value;
`--` cannot hide a package selector. `clean --doc` is rejected even with `-p`: Cargo
documents removal of the target doc directory without a package-only guarantee.
Compilation checks both the canonical target volume and the source/validation volume against the existing 15 GiB/90% limits.
Standalone `--help`/`-h`, `--version`/`-V` and `--list` are supported after
these globals. Local subcommand options such as standalone test
`--manifest-path` remain direct arguments; this is separate from the restricted
remote grammar. Settings inspection starts no Cargo process.

CI derives immutable release/seed inputs from the existing literal Rust
`include_str!`/`include_bytes!` declarations and checks release-inventory coverage.
An inventory entry marked `development-only` may still be compiled into a binary.
Unembedded development documents remain docs-only; the installer job always runs
the Python build-helper suites and shell scope regressions, without a cloud job.

For an independently authorized, eligible remote build, this example reserves
eight cores but permits sixteen, with sixteen Cargo jobs:

```sh
python scripts/modal_build.py --platform apple run \
  --confirmed-modal --estimated-local-secs 600 \
  --cpu 8 --cpu-limit 16 --jobs 16 -- \
  cargo build --locked -p brassclaw_reborn_cli --bin brassclaw-reborn \
  --target aarch64-apple-darwin
```

CLI values override the selected Modal profile. For backward compatibility,
`--cpu N` alone sets both reservation and limit to N; an explicit `--cpu-limit`
separates them. `--memory-mib` and `--memory-limit-mib` follow the same rule.
`--no-nonpreemptible` explicitly overrides a profile's `true` value. Current
checked-in defaults preserve the qualified eight-core configuration; the example
is not an instruction to launch a sizing experiment. `--allow-parallel-hosts`
remains accepted by old commands but is no longer required. It grants no spending
or activation permission.

## Locks, inputs and interruption

- Native Cargo uses `/private/tmp/brassclaw-target-cargo.lock` on this Mac
  (`BRASSCLAW_CARGO_LOCK` overrides the path).
- Source sync and Modal submission use the separate
  `/private/tmp/brassclaw-modal-build.lock` (`BRASSCLAW_MODAL_LOCK` overrides it).
  Other platforms use their system temporary directory. All controllers on this
  machine must agree on these paths and the canonical target/receipt location.
- The Modal lock covers unsettled-job preflight, source sync, readback, submission
  and terminal receipt collection continuously. A second remote build or
  standalone sync fails while it is held. Local compilation is not cancelled
  and does not cancel remote work.
- A disconnected controller can release its OS lock while its function is still
  running. Persisted `running`/`unresolved` states therefore continue to block
  Modal source/cache changes until actual remote termination is reconciled with
  `status`. They do not block local Cargo. Do not delete receipts to clear them.

Source capture must remain stable through capture/readback/admission or the job
is rejected before compilation. The worker verifies the fixed source manifest
and uses the image-specific Linux-host/target cache on the Volume. Once admitted,
later local edits do not change that snapshot. Keep the native build's captured
inputs stable independently; remote isolation does not make edits to an actively
compiled local source file safe. No sync into running remote inputs is permitted.
No local/remote target-directory copying or two-way source merge is performed.

This is single-controller-machine coordination. The local OS lock is not a
distributed lock for independent submitting machines; do not introduce a second
controller host writing the same Volume under this contract.

Each result retains its own exact command, source manifest, image, resources and
artifact identities. Native tests use their existing database/runtime locks;
supported remote execution needs equivalent fixture custody and compatible
artifacts. Normal Apple offloading still supports CLI dev/release builds
only; the qualification-only test preparation described below does not activate
a test mode; the [combined-build plan](../combined-building_v3_at_bob3.md) describes
future extensions. User permission alone does not establish test transport
qualification.
Different snapshots establish
different claims; remote compilation cannot certify a newer native test result.
Screen can keep either launcher running detached; it does not replace these
locks, source custody or evidence capture.

See [Modal image qualification](modal-cross-build.md) and
[development cost/validation policy](development-policy.md). The existing
qualified image can execute this controller's resource settings without a new
image build; no embedded compiler-worker implementation changed here. Local
regression checks exercise settings, actual cross-process locks and SDK transport
handoff. The subsequent necessary
[integrated-main build](validation/2026-11-03-bob-main-integration.md) passed
through this controller on the qualified image, followed by native startup and
embedded-byte checks. That live run used the unchanged eight-core settings;
it does not establish sixteen-core scaling or burst utilization. No separate
verification-only paid build was run.

## Qualification-only Apple library test preparation

Source support now prepares one bounded command shape through the existing
controller/worker: `cargo test --locked --no-run --lib -p
brassclaw_reborn_composition --target aarch64-apple-darwin`. This is compile and
artifact transport preparation only. No new compiler image was built or
qualified for this mode by the source change. Ordinary test offloading remains
**DEACTIVATED**, including with an existing product-build activation. The
existing `--qualification` route still needs a separately authorized, necessary
qualification job and a newly prepared exact worker/image. CLI `accept-native`
and `activate` reject test receipts; a native test/fixture qualification and its
activation path remain open.

The controller accepts split and `=` forms for its declared value options.
It supports the default test profile and explicit `--profile test` /
`--profile=test`. Cargo itself also accepts other profiles and `--release`;
this bounded preparation rejects them before admission. It also rejects
`--doc`, bin/integration/bench/example/all-target selection, test-name filters,
`--` forwarding and custom library harnesses. Named composition features,
`--all-features` and `--no-default-features` use the captured manifest's explicit
local feature closure; unresolved/implicit feature forms are rejected. An
unexpected Cargo feature selection fails collection rather than exporting a
possibly incompatible program. This is not qualification of every feature set.

For this mode only, omitted message format and the supported JSON forms `json`,
`json,json-diagnostic-short`, `json,json-diagnostic-rendered-ansi` and
`json-render-diagnostics` normalize to exactly `--message-format=json` **before**
admission. Repeated, human or unknown formats are rejected. Requests and results
retain both the original command and actual normalized invocation. The worker
logs the entire merged stdout/stderr stream while reading Cargo artifact and
build-finished JSON messages. Only that invocation's exact package ID, source
manifest, library target, `profile.test`, features and executable path can
supply the single required test artifact. Cargo-selected `fresh=true` artifacts
are eligible; cache globs and mtimes never select a test binary. Missing,
ambiguous, wrong-profile/target or unexpected test artifacts fail closed.

The executable must be a regular, non-symlink Mach-O arm64 executable within the
canonical image/Apple-target `debug/deps` directory. The job retains a private
snapshot and hashes that snapshot, together with image/source/worker identities.
A synchronized repository never replaces `/opt/brassclaw` in an immutable image.
New image preparation records the embedded worker/source-sync byte identities
and supported preparation modes; test admission requires an exact match, and
the executing worker checks its own bytes. Legacy product image/activation
records remain valid only for their original product scope.

`run` and `status` use one artifact collector, original job UUID, Volume and
environment. Downloads stage privately, check size/SHA-256 and rename atomically.
After a returned remote result, a download failure leaves `collection_pending`
with `remote_state=completed`. The existing unresolved-job gate conservatively
blocks new remote synchronization until collection finishes. `status <UUID>`
reconciles the original durable completion even if the spawn response/FunctionCall
handle was lost. A missing or transport-failed Volume receipt leaves the job
unconfirmed. Canonical regular local artifact files are reused only after size
and SHA-256 verification; corrupt, missing or symlink copies are fetched again
from the original job. Repeated status calls therefore do not retransmit a valid
large binary. No Cargo or job replay occurs, including after SDK result-handle
expiry. Local state becomes
`completed` / `collection=verified` only after all required transfers succeed.
That state means verified byte collection; it does not certify native test
execution, fixtures, shared services, companion workers, compile-time paths or
product acceptance. Darwin test binaries are never executed on Linux.
