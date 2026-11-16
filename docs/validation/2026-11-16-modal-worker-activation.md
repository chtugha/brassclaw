# Actual Modal worker update and activation — 2026-11-16

Image `im-qm70mVr4NTtFZgxk748qWZ` is ACTIVE for `apple-cli-build/1` in the
`brassclaw` environment. The SDK, OSXCross, macOS SDK and Rust 1.98.1 toolchain
configuration are unchanged. Existing layers were reused; image preparation took
10.69 seconds. The previous image and activation records are preserved under
`target/validation/modal-worker-activation-20261116-r1/`.

The new worker's actual installed bytes were read from the running container:

| File | SHA-256 |
| --- | --- |
| `modal_build_worker.py` | `8677cf52dfc52a9153cf03a37e719c528bbf16fc890ecbdfccfad0fc4e577d7e` |
| `modal_source_sync.py` | `c53d9cc3738689a878c43410219704c3c48b7f48d0c275126ba0edf4fe3a55ea` |

## Build, native checks and ordinary activated execution

- Qualification job `3ed0bafe-5b6e-4279-8927-8cd06fdc85ea` built the actual
  default CLI plus normal Monty worker and retained pgvector in 705.14 seconds.
  Current Cargo output and original immutable job completion were collected and
  verified. No compiler warnings or errors were emitted.
- Local native `file`, `otool -L`, `--version` and `--help` checks passed for the
  exact returned package. The existing activation command then enabled that
  image/job; no record was relabelled and no activation gate was bypassed.
- Post-activation job `bfab48c1-f02e-4f69-9bfc-72035aba7169` used ordinary
  `run --confirmed-modal`, without `--qualification`, and `cargo build --locked
  --frozen`. All 1,095 Cargo artifacts were fresh: no Rust compilation was
  repeated. The Function completed in 205.93 seconds, including source/cache
  preparation, artifact inspection and persistence. No compiler diagnostics
  were emitted. All three exported files are byte-identical to qualification.
- Both real requests contain `timeout_secs: 1800`. No derived Cargo timer or
  120-second deduction exists in the actual installed worker. No unclear job
  was replayed; the first job's complete, verified success precedes the distinct
  normal-path acceptance job.

| Artifact | SHA-256 |
| --- | --- |
| CLI | `fa673002e162f92bb84b091956075171cfb82de9e30c7615848504d5fac6a0f4` |
| Monty worker | `cc84a75a02e0a546e7a8a55f3bcc5ff99c35f2c219fc52f23353b38cf1850ca1` |
| pgvector archive | `39683477a4463783ed9f2f0b6fdf7e25d3c912e68b2097547ad3c0085447df4d` |

## Live-discovered SDK exec race and repair

The initial qualifier exposed eight failed inspection attempts; they remain in
its original observation history and `accepted.json`. A captured transport
response proves `modal container exec --no-pty` could return zero with empty
stdout and stderr. This was not a Cargo failure or permission to replay it.

Installed SDK 1.6.3's `modal.sandbox._sandbox._container_exec` creates
STDOUT-printing readers and awaits `_ContainerProcess.wait()`. That wait
acknowledges process exit; it does not await the background printing tasks in
`_StdoutPrintingStreamReaderThroughCommandRouter`. Short output can be lost.

`scripts/modal_container_probe.py` confines the required compatibility bridge
to tested SDK 1.6.3. It uses the same credential client, router access fetcher
and container-scoped authorization, with PIPE stdout/stderr. It explicitly
awaits both readers and process exit on the SDK's own event loop. It propagates
the real exit status and stderr, accepts only the fixed read-only inspection
program with original request data, and starts no Function/container/Cargo.
No arbitrary sleep, PTY, installed-SDK patch, access bypass or suppressed error
is used. Unsupported SDK versions fail explicitly until qualified.

The existing controller/settings suite now has 68 passing cases, including exit
before stdout, delayed stderr, nonzero exit propagation and version/identity
refusal. The 27 worker, six source-sync and 55 Screen cases remain applicable
to unchanged inputs: 156 passing local cases in total.

With the repaired adapter, the actual normal-path job produced four successful
automatic remote samples and zero observation errors. Three additional explicit
live observations all returned matching job/image identities, Cargo and process
data. Initial preparation without a Cargo process is labelled as an observation,
never failed compilation or successful completion. Original failed observations
are preserved; later success does not rewrite their history.

Image preparation, qualification, native acceptance/activation and post-check
used the existing Screen starter/helper and the current chat. All four sessions
ended zero, final notifications were accepted and received, and the private
Screen socket directory was empty afterward. No temporary PostgreSQL cluster
or local Cargo build was needed.

This activates the current Apple CLI build path. Native test-mode activation,
native Linux execution, the prepared Rust 1.100 candidate and complete product/
Bob-plan acceptance remain separate scopes. The image update does not claim
those capabilities or transfer component approval.
