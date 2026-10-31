# K3 conditional filesystem writes and internal result evidence

Status: **scoped acceptance passed**. R5 is terminal, including Prefix Direct5,
default-feature Clippy, formatting and whitespace checks. This report does not close
K3, another complete plan, production serving or platform acceptance.

## Implemented boundary

The existing run/approval, turn, thread, conversation, outbound and process
filesystem consumers preserve the exact requested CAS condition. Unsupported
conditional writes remain errors. Conversation/outbound/process records may
remove unsupported index projections, keeping the same body/content type and
condition; an unsupported version check never becomes `Any`. Process creation
uses atomic `Absent`, so independently constructed stores cannot replace an
already published original. A local mutex is not cross-process atomicity.

Actual LocalFilesystem initial creation remains supported. Its unsupported
version transitions reject without changing the original. Positive transition
fixtures use the existing record-capable owner; actual local negative cases
remain. One reusable contract per consumer runs against the actual memory owner
and the same six consumers against actual PostgreSQL. A barrier at final
publication proves one winner for two independent process stores.

The connected Prefix failure exposed a separate missing evidence handoff:
ordinary internal execution disabled the bounded step observations required by
its promoted receipt verifier. The existing trusted `InternalRecipeSource` now
declares whether it requires those observations, defaulting false. Its ordinary
constructor combines that requirement with explicit bootstrap observation; only
the PairCases receipt owner opts in. This reuses the existing runner and limits.
It changes no approval, Tool permission, workflow sequencing or settlement guard.

Test-only read observations expose actual receipt classifications, acknowledgement
state and retained reports without consuming receipts, polling their futures,
reconciling effects or exposing raw internal results/lease tokens. Custody-check
diagnostics print only fixed source-owned labels. No timeout/assertion is weakened.

## Evidence and applicability

- R1/R2 retain their actual lint failures. R3 passed affected test lint/build but
  failed the first new contract on an invalid fixture virtual root. Both affected
  fixture paths now use the supported `/engine` root.
- R3 plus its independent remaining sweep passed284 existing consumer cases,
  zero ignored. R4 ran all six new contracts and the six-consumer native PG case
  successfully. Thus291 distinct consumer tests have applicable passing evidence.
- R4's Prefix Direct4 failed the ordinary Pair case after two answered native
  calls. The exact static rejection was `pair case completion/order differs`;
  the service returned `InvalidPortResult` before a receipt or settlement existed.
  The failed receipt/log/source remain unchanged, not relabelled as success.
- R5 passed seven-package strict test Clippy (`-D warnings`,80.18s), built the
  actual Monty worker and selected test images (211.91s). Its consumer results are
  reused only where the actual emitted executable hash matches the corresponding
  passing case. The receipt records each original evidence path/hash separately.
- R5's actual Pair2 passed, zero ignored. The matching and changed-suffix cases
  complete and settle; the invalid leading-prefix case fails and settles with
  native `retained_tool_invalid_input`, acknowledgement `Ok`, failed step `0:5`
  and no internal success receipt. Reconnect does not replay native effects;
  changed/foreign originals fail and no assistant reply/Base fallback is produced.

Joint receipt: `target/validation/conditional-writes-20261026/run-5/accepted.json`,
SHA256 `b2e1d8d99deee81d32e58450b8aba6645843cc269c95e4b04c8d54f7a58b16bb`.
All291 consumer cases reuse actual applicable passing executions through exact
emitted image equality; they are not291 fresh R5 executions. The three Prefix
cases are fresh R5 executions, zero ignored. Default-feature strict Clippy
passed25.57s and whole-workspace format/whitespace checks passed. Root independently
verified all22 retained executables and the complete current non-document compiler
input set, including additions/removals. No validation process remained before
the common implementation phase reopened.

Prefix-owned records:
`target/accepted-evidence/prefix-pair-private-admission-20261026/run-5/`.
Its scoped accepted receipt SHA256 is
`feb8110bddb17a7c09526b2d0b7ee42debad64eabce813417bdf049217a24d24`:
Pair2 passed82.97s and the full existing native Prefix regression passed369.02s.
Composition image SHA256 is
`2a1fcc545547aab8ef0deec49e4a9fbe3c9006fc3cba039d1620f5b96b8bc1fe`;
actual worker SHA256 is
`586693a8d5361acfaa98690b6ba23b1302832997c579937cae6e63683e07353f`.
The regression captured319 complete declared-release sources,9,147,370 bytes.
Its actual Sempai diagnostic376,593 tokens exceeds the model artifact's262,144;
the warning remains visible, context fit is false and no design/prefix is activated.
Exact current common inputs, emitted binaries, worker and manifests are retained
there. Source-only measurements do not qualify production serving or final model designs.

## Remaining work

The Prefix owner retains its product/design/serving work. Persistent requirement/account
binding, original setup/task-family credential selection, remaining K3 technical
settings/protocol consumers, K4–K7, general validator/composition cutovers and
durable recovery remain open in their existing owning plans. This correction
introduces no new queue, registry, VM, Rust workflow or fallback execution path.
