# Own prepared patches integrated on local main

Date: 2026-11-03. Integration commit:
`c3de59205` (`fix: integrate Bob auth custody and independent build environments`).
The checkout was already on `main`; no separate branch or stale source tree was
merged over it. Prepared own patches were already applied. Final recorded
source identities were checked before committing 44 owned files. Superseded
initial auth/UI patches were not replayed. The Bob-2 inventory plan's later
`prepared` → `recorded` wording was preserved.

Other-agent validator changes, their scratch runners, orchestration notes and
`.DS_Store` remain outside this commit. No foreign work was reset, overwritten
or committed, and no GitHub push occurred.

## Actual remote build and native checks

The existing qualified image `im-S1cpT9G1fsMPpwjISvBSlK` built the integrated
working-tree snapshot through the updated independent-environment controller.
The snapshot includes separately uncommitted validator work; it is not a
clean-commit-only acceptance claim. All 44 owned committed paths match its
manifest. Source manifest SHA-256:
`fce56666bd1d11dd46c87440d1b11c77273e0c4731c49ce2b6a96d8b657ed9f9`.

Job `911aa4b9-571a-41be-a53a-c8eb00c23f46`,
[Modal App](https://modal.com/apps/chtugha/brassclaw/ap-zTdmBtm5bOQs5vfh8Ogrdl):

```sh
cargo build --locked -p brassclaw_reborn_cli --bin brassclaw-reborn --target aarch64-apple-darwin
```

Result: **0**, no compiler warnings/errors. Cargo reported **4m 47s**;
worker duration **365.40 seconds**, complete controller turnaround
**404.52 seconds**. Request and limits were both eight cores and 32 GiB RAM,
with eight Cargo jobs. No larger machine, new image or second build was used.
The remote critical section includes source sync and submission; the native
Cargo lock is independent. This accepts the live default-resource controller
path, without claiming a sixteen-core speedup or measured burst utilization.

The 336,129,008-byte Mach-O arm64 binary has SHA-256
`37a02e44576f9774d6e18d8c7bdc0b5158796e6acac579fb667291ed6922bd15`.
Native `file`, `otool -L`, `--version` and `--help` passed. The binary contains
the exact three changed runtime UI assets plus the current AGENTS.md and
CLAUDE.md bytes. The pgvector artifact is unchanged, allowing reuse of its
existing qualified native extension evidence.

Read-only inspections of this same worker showed Cargo's increasing read
counters before rustc started; there was progress during the quiet log period.
This points to cache/filesystem startup work. No extra worker, benchmark,
automatic retry or test was launched for diagnosis. Per-phase timing and a
comparable native baseline remain absent, so no speedup claim is made.

The App stopped at **06:49:48 Europe/Berlin**, with **zero tasks**.
Full request/source/completion/provenance/native logs and binary remain under
`$CARGO_TARGET_DIR/modal-build/jobs/911aa4b9-571a-41be-a53a-c8eb00c23f46/`.
The original controller/integration/check scripts and transcript remain in
`/private/tmp/brassclaw-bob-main-20261103/`.

See the [machine-readable receipt](2026-11-03-bob-main-integration.json).
The applicable 73-case UI and 27-case build-tooling evidence was reused rather
than rerun. No Rust tests, real issuer login/refresh, remaining validator cases
or full Bob-plan completion are certified by this compile/startup receipt.
