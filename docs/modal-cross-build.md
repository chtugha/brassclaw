# Modal Apple Silicon cross-build

This extends `scripts/modal_build.py`; it is not a second build pipeline.
The compiler host is Linux/amd64; the artifact target is
`aarch64-apple-darwin`, minimum macOS 14.0 (also required by the bundled PG).
Use only the `brassclaw` Modal environment and `brassclaw-build` Volume.

The image is a prerequisite, not proof of a working BrassClaw build or activation.
All tests, including test compilation, remain local. Remote workers compile only.

## Image inputs

`modal_apple_toolchain.py` pins Rust 1.96.0, the amd64 Rust base by OCI digest,
OSXCross commit and tarball SHA-256, Debian's 2026-07-01 package snapshot, LLVM 19,
including its security repository, and SDK 26.5 by the SHA-256 of the supplied archive. The image receipt records the
actual Modal image ID. `/opt/brassclaw/` records the complete dpkg package versions,
Rust/Clang/linker versions, OSXCross configuration and SDK provenance.

Package an actual local SDK with OSXCross's `tools/gen_sdk_package.sh` on macOS
(`XCODE_TOOLS=1` selects Command Line Tools). For this qualification the upstream
packager was copied with one selection-only guard in its SDK loop:
`[ -z "$SDK_FILTER" ] || [ "$SDK" = "$SDK_FILTER" ] || continue`.
`SDK_FILTER=MacOSX26.5.sdk SDK_COMPRESSOR=gz XCODE_TOOLS=1` packages only the real
26.5 SDK; no SDK contents or version labels are invented. The source packager,
selection-only copy, logs and archive are retained with setup evidence.

```
python scripts/modal_build.py --environment brassclaw --platform apple setup \
  --confirmed-paid-setup --sdk /absolute/path/MacOSX26.5.sdk.tar.gz
```

This is paid setup and needs a concrete authorized build. It publishes a reusable
image, with no warm worker. It does not activate general Modal build use.
The OSXCross setup performs compiler/linker probes only; no target executable
or project test is run on Linux. The OSXCross make job count is two and its
command has a 1200-second timeout.

## Project build

The existing source synchronizer runs first, uploads the deterministic overlay,
and verifies source and readback hashes. The controller checks actual local Cargo/compiler processes before synchronization,
after acquiring the common local Cargo lock, and immediately before submission.
It holds that lock until remote Cargo is terminal and checks for uncooperative
local builds every five seconds during the call. A detected collision cancels
the remote input, leaves the local build untouched and requires terminal-state
reconciliation before another Cargo run. Do not run other Cargo commands
on any host during this interval. The current Apple controller supports the standalone CLI product build shown
below (dev or `--release`), not arbitrary check/profile commands.
The worker verifies base/overlay coverage and
all resulting source bytes before updating the persistent image-specific source
and target caches. Host build scripts/proc macros use Linux compilers; only the
Apple target has target-prefixed CC/CXX/AR/RANLIB/linker settings.

```
python scripts/modal_build.py --environment brassclaw --platform apple run \
  --qualification --confirmed-modal --estimated-local-secs 1800 \
  --cpu 8 --memory-mib 32768 --jobs 8 --timeout-secs 900 \
  --shutdown-grace-secs 30 --nonpreemptible -- \
  cargo build --locked -p brassclaw_reborn_cli --bin brassclaw-reborn \
  --target aarch64-apple-darwin
```

`--qualification` is only for a specifically authorized image qualification. It
never activates general usage. No automatic retries or parallel containers are
configured. The worker terminates Cargo before the platform timeout to allow log
and receipt commit. Interrupted/unresolved jobs block subsequent submissions;
reconcile actual Modal function/app state before releasing the build slot.

`$CARGO_TARGET_DIR/modal-build/jobs/<UUID>/` retains the exact request,
`source.json` (HEAD plus file identities), `applied.json` (baseline-to-selected
changes), `original-runner.py`, Cargo log, completion receipt and downloaded
binary. The matching Volume job directory contains the same receipt, artifacts,
and Mach-O headers/dependencies. A successful command alone is insufficient:
verify the arm64 executable and a nonempty, complete Mach-O arm64 pgvector archive.
Run `python scripts/modal_build.py --platform apple accept-native <job-UUID>`
on the Mac to inspect and execute the downloaded binary and retain the native
acceptance record with the exact artifact SHA-256. Do not replace local Cargo caches with
remote artifacts.

## Target PostgreSQL metadata

Linux must not execute Apple's `pg_config`. The build script reads captured
`pg_config-aarch64-apple-darwin.txt`, checks its archive identity against the
existing PostgreSQL pin and checks the extracted target pg_config binary hash.
It generates a host shell reader with paths relocated to the extracted PG root.
Only values captured from the actual checksum-verified target installation are
served. Verify the captured metadata on a native Mac with
`python scripts/verify_apple_pg_metadata.py /path/to/postgresql.tar.gz`. It checks
the full report and every individual option against the exact target binary.
The captured report intentionally preserves native trailing whitespace, including
the separators of empty option values; it must not be trimmed.
Native builds keep the native pg_config path. PGXS receives all four
actual target tool commands and portable `OPTFLAGS=`. Cross-build failure is
fatal, so empty placeholder pgvector archives cannot qualify the image.

## Explicit activation

```
python scripts/modal_build.py --platform apple activation-status
python scripts/modal_build.py --platform apple activate <qualified-job-UUID>
python scripts/modal_build.py --platform apple deactivate
```

The default is **DEACTIVATED**. `activate` is an explicit operator action after
qualification; it requires the successful exact-image receipt, artifacts, and
`native-accepted.json` whose `artifact_sha256` matches the cross-built executable.
The activation record pins the image ID and qualification job. Replacing that
image invalidates activation. Authentication, source Volume, an image draft or a
published image alone never suffice. Without activation normal `run` refuses
before source sync or worker launch; local Cargo requires no Local/Modal question.
General build-location choice only applies after this activation and under the
current development policy. Inspect status for the actual target/platform.

## Qualification record, 2026-11-03

Published image: `im-S1cpT9G1fsMPpwjISvBSlK`, named
`brassclaw-rust:1.96.0-apple-arm64`, setup App
`ap-6uZrRxDSSX1BZbqrMO0RN8`. SDK archive SHA-256:
`0c93ec9a911f0ee9b3de343e3b980d3a449370c737f722ca780f1dd0b4f09926`.
OSXCross's C, C++11/libc++ and C++20 compile/link probes passed. Native checking
of the PG report and all 23 options passed. Focused native embedded-Postgres
Clippy with `-D warnings` passed (15.24 seconds); the 11 local source/worker tests
passed without duplicate fixture inheritance.

The first project worker, job `1acf3236-ff97-4ca4-86eb-711d6c6e58fa`, was stopped
with SIGTERM after 177.17 seconds when another local Cargo process was reported
bypassing the common lock. Its completion receipt has return code `-15`, no
finished artifact, and retained source-manifest SHA-256
`29951a763a6aba1a016dd62b1b3f3ead79749b530ae5a64b7688635839773723`.
The App `ap-Grch5hKIBbRVpj2wrIS9mJ` is confirmed stopped with zero tasks. Its
registry and target caches remain. This interrupted run is not a successful
project qualification. General Apple Modal use remains **DEACTIVATED**.

### Explicit parallel-host exception

On 2026-11-03 the user explicitly instructed this qualification to compile in
parallel with the existing local build. `run --allow-parallel-hosts` records
that per-call exception in `request.json` and disables only the local-process
collision rejection/cancellation for that call. The Modal source is an immutable
verified snapshot and its Linux-host/Apple-target cache is separate from native
macOS Cargo caches. The default remains the existing serial policy; this flag
requires explicit human authorization and does not activate general Modal use.

Job `58f2b80e-927c-4ba7-a103-a0a0ffc7e914` was cancelled during source-cache
preparation when a new local build was detected. Its terminal remote result is
recorded in `terminal.json`; the App is stopped with zero tasks. Remote Cargo
had not started in that job. The subsequent explicitly parallel-authorized job is
`7da58631-e7e7-4faa-a144-025eb4f1845f`, App `ap-WiVXCfFo0pLugfBuz1EFyZ`.

### Successful project qualification

Job `7da58631-e7e7-4faa-a144-025eb4f1845f` completed with return code **0**.
Worker time was **808.41 seconds (13m 28s)**; Cargo reported **10m 34s**.
The remaining time includes verified source materialization/cache preparation
and worker bookkeeping. The published CPU/RAM rate implies about **$0.43** for
this successful function at the requested non-preemptible resources; this is an
estimate, not account billing, and excludes setup, interrupted runs and storage/
transfer. No warm worker remains: App `ap-WiVXCfFo0pLugfBuz1EFyZ` is stopped,
with zero tasks. Registry, source and target caches are retained.

The exact source-manifest SHA-256 is
`d15add35d931bc824c30a4a0335ff4408dc443cec56ec12471c811ea8f64e914`.
This qualifies the recorded snapshot; later concurrent local WebUI/plan changes
are not included. The command was the default-feature standalone CLI build
shown above, with the explicitly authorized `--allow-parallel-hosts` flag on the
controller. The native dev profile and its debug settings were retained.

Actual image provenance confirms Rust **1.96.0**, commit
`ac68faa20c58cbccd01ee7208bf3b6e93a7d7f96`, rustc LLVM **22.1.2**, host
`x86_64-unknown-linux-gnu`, Debian Clang **19.1.7 (3+b1)**, LLD **19.1.7**,
OSXCross **3.0**, flavor `llvm`, target `darwin25.5`, SDK **26.5** and minimum
macOS **14.0**. Full package versions are in the image-provenance receipt.

Artifacts and receipts are under
`$CARGO_TARGET_DIR/modal-build/jobs/7da58631-e7e7-4faa-a144-025eb4f1845f/`:

- `brassclaw-reborn`: 336,076,880 bytes; Mach-O 64-bit arm64 executable;
  SHA-256 `4114e3707a0964f0b3db120e9dcaf9f4ac07c810a057cf65038aa8f55febe032`.
- `pgvector.tar.gz`: 73,888 bytes; complete Mach-O arm64 extension archive;
  SHA-256 `e4e03c5defc87c5540c2b294cd32b6b7010d54afa54dd37376ab89e8520f3b1f`.
- `completed.json`, `accepted.json`, `source.json`, `applied.json`, original
  worker/controller/native runner, `cargo.log`, `controller.log`, and
  `image-provenance.json` retain the exact run and toolchain identities.
- `native-accepted.json` records real Mac executions of `--version` and `--help`
  and Mach-O/dependency inspection. The binary reports `brassclaw-reborn 1.7.0-rc.4`.
- `native-pgvector-accepted.json` records a fresh native PostgreSQL 16.4 database,
  successful `CREATE EXTENSION vector` and the actual vector-distance result
  `5.196152422706632`. The temporary server was stopped and the database removed.
  The initial sandbox attempt was denied at `shmget`; the permitted native rerun
  passed and both logs are preserved.

The image and this concrete cross-build are qualified for the stated checks.
General Modal usage remains **DEACTIVATED** until explicit operator activation.
