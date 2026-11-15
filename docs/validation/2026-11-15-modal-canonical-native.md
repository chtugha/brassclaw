# Modal canonical mount and native product-build qualification

The existing controller resolves Modal's `/build` SDK mount once, before
evidence, disk checks and the worker call. The immutable worker and physical
image cache remain unchanged. This fixes the mismatch between Cargo's canonical
manifest paths and the earlier lexical mount alias without relaxing artifact,
source, package, dependency or executable checks.

Companion commands retain admitted Offline/Frozen flags. Exact known historical
worker collection is separate from current activation/qualification. The actual
serialized entry point is checked locally with a canonical directory and an
SDK-like symlink; all 72 offline controller/worker cases pass. These local cases
use no cloud or Cargo execution.

## Observed normal build and native checks

- Environment `brassclaw`, existing Volume `brassclaw-build`.
- Image `im-J1hA8cNV5PRbk7mGWK8pH0`, Rust 1.98.1, SDK 26.5.
- Job `e2bea7ca-cac8-4a59-9b50-8384cc135c02`.
- App `ap-CnLE4paL433Yj7XERsoKUg`, call
  `fc-01M7F7TF9NDNNV18PEYCQDJH6C`.
- Source manifest SHA256
  `31e8b31864103d4303eeda22f631884dca0d2940fd6b01fc0e72931554014588`.
- Worker SHA256
  `b0e4b66c2dc0a63a4cee4943cfc86945324cbac49e57a6f4549a2230e4b5d277`.

One normal CLI development build and its normal companion build both finish
successfully, with no compiler diagnostics. The ordinary collector verifies and
downloads the complete runtime package v1. Root independently verifies:

| Artifact | Bytes | SHA256 |
| --- | ---: | --- |
| brassclaw-reborn | 432341696 | `2618afb7eedf7ffea4c29f7a81ce1ad69287459511e5b0508241a888b1077c36` |
| monty_worker | 70056912 | `479f25ce2d19dcad7ab008325669fdf2dc3c6a182bfc08c58f28b2929615efa7` |
| pgvector.tar.gz | 73887 | `8893c8e5a0bd2000939b8462f660992411f8c43817f294f21b0039581257334e` |

The current controller's `accept-native` runs `file`, `otool -L`, actual native
`--version` and `--help`: all return 0. Its exact `native-accepted.json` and four
logs are retained beside `completed.json` under
`/Users/ollama/brassclaw-target/modal-build/jobs/e2bea7ca-cac8-4a59-9b50-8384cc135c02/`.
The app is stopped with zero tasks; its Screen ends with exit 0 and final IPC
accepted and actually received in its owning chat.

The existing activation owner now activates only `apple-cli-build/1` for this
image/job, and `require_activation` confirms it. Local and Modal hosts retain
their separate locks, settings and caches. Explicit Modal choice and cost policy
still apply. This does not activate Apple test mode or Rust 1.100, certify full
CLI startup/PostgreSQL/Monty behavior, or qualify Linux/Windows/deployment.

## Performance and original failures

The CLI Cargo phase takes 17m13s, with 674 non-fresh and three fresh artifacts;
the real companion takes 36.97s, with 78 non-fresh and 340 fresh artifacts.
Remote work takes 1192.251s; full controller/transfer takes 1239.662s. The cache
is physically the same, but this canonical-path transition rebuilds much of the
graph. No general speedup or warm-cache performance claim follows.

Original job `f0ed42c9-c537-4e0f-9729-2cfacfc851f2` successfully compiles its CLI
but fails collection at the old path comparison. Exact original SDK exception,
remote traceback, failed call/input graph and immutable request/source hashes
resolve its local projection to failed, with a separate terminal-error receipt;
no successful completion is manufactured. The known bad-source attempt
`65c141ea-a49d-42dd-a045-84427879a945` is explicitly cancelled and reconciled
through normal status. Its originals are retained; missing Cargo logs do not
prove compilation started or succeeded. Neither attempt is silently replayed.

A separate Rust 1.100 image `im-T18jhSVqHi1K0LFzz8mGeW` is prepared from the
same base with a real Mach-O link probe. Its receipt deliberately has empty
supported modes. It has no project/native qualification or activation. Apple
Composition test-mode qualification remains a separate gate for a genuinely
needed future test build; current product evidence cannot authorize it.
