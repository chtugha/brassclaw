# B3 current-source Modal CLI build

Date: 2026-11-03. Scope: default-feature standalone CLI compilation, native
startup probes and embedded-asset identity. This does not accept live Codex
login/refresh, PostgreSQL integration, Rust tests or the complete Bob plans.

The user explicitly selected the qualified Modal image while the other chat
finishes local validation, including isolated overlap between the two hosts.
Image `im-S1cpT9G1fsMPpwjISvBSlK` was verified against its successful qualification,
native executable and PostgreSQL-extension receipts, then explicitly activated.
No image setup, remote tests, second build or local Cargo execution was run.

## Actual build

Job `e1fb11c2-e5ce-4bb9-be5b-a3f9d8e98b0a`,
[Modal App](https://modal.com/apps/chtugha/brassclaw/ap-BJ6WG0inuMLRSGC9AJhrf4),
used environment `brassclaw`, Volume `brassclaw-build`, eight requested CPUs,
32 GiB RAM, eight Cargo jobs and the existing qualified nonpreemptible settings.
The fixed source snapshot has manifest SHA-256
`26b4e5b77ebbdf9faa3ca5f0d7c2b29931e04f0e5f5f91dd2dc31d747384b4e2`.
Its recorded HEAD is `35be9773d498256c6ab788d9ad613f52e776fd71`; the manifest
also includes the actual uncommitted inputs. Source synchronization verified
10,129 files and uploaded a 236,035-byte overlay containing 36 changed files.

```sh
cargo build --locked -p brassclaw_reborn_cli --bin brassclaw-reborn --target aarch64-apple-darwin
```

Result: **return code 0**, no compiler warnings/errors in the retained log.
Cargo reported **4m 54s**; worker duration was **340.56 seconds**, controller
duration **383.09 seconds**. The difference includes source/cache preparation,
artifact recording and controller synchronization/transfer. No per-crate or
linker timing was captured, so neither the dominant compiler bottleneck nor a
speedup over an equivalent native build is established. The dashboard's later
idle CPU reading cannot establish compilation CPU utilization.

The resulting Mach-O arm64 executable is 336,109,904 bytes, SHA-256
`3880b76c5569397745fcb15d63fec2ad77dffc8455fbd5522801aa979bbefb08`.
Native Mac `file`, `otool -L`, `--version` and `--help` all passed.
The three changed runtime JavaScript assets (`provider-mutations.js`,
`provider-login.js`, `provider-mutation-status.js`) match the captured source
manifest and occur byte-for-byte in the executable. Factory/native-auth source
identities also match that snapshot. This closes packaging evidence for these
assets without claiming real-browser or issuer acceptance.

The pgvector artifact hash is unchanged from the qualified native PostgreSQL
extension test; that exact prior evidence is reused rather than repeating it.
The App stopped at **06:08:48 Europe/Berlin**, with **zero tasks**. There is no
remaining build worker. The reusable image/cache and explicit activation remain.

## Evidence

[Machine-readable checks](2026-11-03-b3-modal-cli-build.json) record the source,
artifact and embedded-byte identities, native probe results and scope limits.
Full source/request/completion/provenance/Cargo/native logs and binary remain in
`$CARGO_TARGET_DIR/modal-build/jobs/e1fb11c2-e5ce-4bb9-be5b-a3f9d8e98b0a/`.
The controller transcript and terminal result remain under
`/private/tmp/brassclaw-bob3-modal-current/`.

Existing [73-case browser transport evidence](2026-11-03-b3-mutation-auth.md)
is retained; it was not rerun on Modal. The other chat's validator acceptance
remains separate.
