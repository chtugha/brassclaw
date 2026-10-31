# K3 — Exact failed-publication targets, 2026-10-24

**Scoped acceptance passed:** 62 freshly executed Rust cases, 47 UI contract
cases, six JavaScript syntax checks, strict affected/root/default lints,
formatting and final integrity under
`target/accepted-evidence/k3-failed-publication-20261024/run-1/`.
This completes the bounded failed-target publication extension, not K3, durable
recovery, task-wide selection, prefix serving or installed/platform acceptance.

## Implemented owner and consumer boundary

`ProviderPublicationStatus` captures effective client metadata and the last
known failed target in the same Swappable snapshot. Existing primary and Sempai
publication gates accept a preparation error carrying only the configuration
actually captured before the failed resolution/build. They retain the original
error and old client. Unknown capture/source errors do not invent a failed
revision from a later read. Successful concrete publication clears its failure
atomically. No new provider registry, event queue, model workflow or authority
is introduced. The failure context boxes only its target to keep Result errors
small; status contains no raw backend error or credential material.

The existing root resolver is split into capture and pure resolution of that
same captured selection. Startup and other callers retain the original resolver
entry point; reload can now associate a real guard/configuration/build failure
with its original target without capturing or consuming another secret snapshot.
Primary metadata-validation errors release their read guard before updating
failure metadata; they cannot lock themselves out while retaining the old client.

The existing embedding resolution gate tracks its captured attempted target.
Failed resolution retains the established cache-retirement/untracked behavior,
now with exact known failure metadata; successful compatible cache application,
new client publication or explicit disable clears it under the same owner lock.
The old separate private publication getter is replaced at all found callers
by the atomic effective/failure status getter.

The canonical settings snapshot adds `runtime_publication_failures` and derives
both actual maps from one status read per owner. Desired revisions remain the
read-only snapshot qualified in the preceding acceptance. The WebUI compares
a failure only with that exact desired target, shows older failed targets
separately, and gives English/German help. A later key/model/definition edit does
not inherit an old failure label. Client publication remains distinct from
reachability, prefix readiness and task-wide binding.

## Actual acceptance

The existing actual PostgreSQL/provider caller now changes a real definition to
require a credential and deletes its encrypted key. Root and Sempai factories
fail after capture, retain their previous concrete clients and report their own
exact failed targets. A later external model edit does not replace those targets.
Restoring a valid encrypted credential and publishing succeeds and clears both
failures. Original receipts, ACK-loss, role concurrency and deactivation remain
covered by the five real provider cases (15.51 seconds).

The actual registered Memory/PG/WebUI caller covers failed embedding selection,
a different later desired model, real repair and clearing, then preserves the
existing invalid-write and cache-retirement checks (67.64 seconds). Live role
cache controls pass (56.40 seconds). The ordinary same-global-service model
caller passes with its controlled HTTP protocol responses (35.13 seconds).
Separate actual primary and Sempai Ornith inference passes in 18.76 seconds;
this is not a full Sempai Recipe or prefix-serving acceptance.

The actual library snapshot tests additionally cover the rejected wrong-model
publication retaining its client, exact failure metadata, successful clearing
and unknown-source errors without fabricated targets. That fixture checks
atomic metadata mechanics, not an upstream model service or a hard runtime
startup deadline. Existing service, HTTP routing/descriptors and 32 architecture
cases pass. Required selectors execute positive case counts with zero ignored;
the actual inference case is explicitly selected with `--ignored` and executes.

The full HTTP/Prefix consumer passes in 341.63 seconds with all 319 declared
sources, original model artifacts, exact measurements and reconnect without
replay. The actual `371716 > 262144` reference-tokenizer warning is retained and
the oversized candidate remains `fit=false`. No approval or serving activation
is inferred. UI tests qualify their contract/rendered-tree harness; no rendered
browser, test-machine build or deployment is claimed.

## Exact evidence and remaining work

- accepted.json: `c7fac2391bda294d7f6b33953cfb5cad2346d79ce976f5c3a12332714d519f50`.
- source.json: `8aff54f6d8b717801dae08875bc5dab62d6c9a59f518915cbbc1b81060e5ed7e`.
- validated-failed-source.json: `2241eecc8e47f18ee1894ebf74cf58e4787d69bb2ea88520cda90644740ae983`.
- Actual composition executable: `40e7340e241b46635d467cab94954a0fb11b6fd740dc7ef72342846bc7ed0d8f`.
- Actual LLM executable: `b6d8b98b4068a0aeb2b99b2bddcf66c4bb3f50d577ba1587be90f7d117269011`.
- Actual WebUI handler executable: `6b08730f59ff5ff3a0c1ec35e414e178262b49e142bd1ea5edf226a76d13cd72`.

Sixteen changed compiler/JavaScript inputs are retained immutably. The 530 worker
inputs remain unchanged and its actual retained executable is hash checked.
Cargo runs serially in screen with the inherited canonical target; actual
executables are preserved before required cleanup. Unchanged architecture
executables are freshly run against this tree; no prior test result is counted
as a fresh execution. The preceding authenticated read-only secret inspection
acceptance remains applicable to its unchanged owner source.

Failure metadata describes the owning runtime's last captured publication
attempt; it is not a durable attempt/effect ledger or an admission/recovery
proof. Original provider mutation receipts retain durable write custody. Full
task-wide provider/model/prefix pinning before effects and over children/waits,
dedicated auth/login and generic settings/import writer convergence, other
protocol/session boundaries, invalid optional-role startup/admin coordination
and K4–K7 remain open. The final request guard stays with the Prefix owner.
