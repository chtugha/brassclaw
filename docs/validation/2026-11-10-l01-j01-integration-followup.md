# L-01 — connected J/I/K source follow-up, 2026-11-10

Status: **coherent source integration; compile/native/runtime acceptance UNRUN**.
Owner branch: `codex/l01-revision-policy`. Initial checkpoint `3a0812bfe`.
J/I/K base integrated first: `8dc775a003c42de4d4c18b696f0b39314100c1ca`,
recorded in owner merge `01cab9888`. Final current-main base:
**`a8ace38b6f029c9238ff780c364814f5a70e4675`**, including K's adapter fix.
The completion message identifies the resulting owner merge commit.
Root/main was neither edited nor pushed.

## Actual integration, not attachment replacement

The two initial conflicts were resolved semantically against the supplied actual
main: J's factored Pure helper remains, and the single PG wire owner contains J's
named-record helpers alongside L's unnamed/DataRow helpers. The old J attachment
was not applied and no older whole file replaced current J/I/F/K source.
The historical Nov9 handoff/manifest/patches remain checkpoint evidence only;
they are not final source inputs and must not be applied to this integration.
The new source manifest below pins the actual current files relative to a8ace38b6.

All eleven previously identified connections are now source changes in the
actual checkout:

| Current file | Connected behavior |
| --- | --- |
| engine `retained_source.rs` | Both input/result schemas in the factored Pure helper and in new `InspectedPureSelection::invocation` use the exact root's `parsing_limits()`. Actual inputs still bind with genuine HostCall `limits`. |
| composition `private_work_definition.rs` | J's `OriginalPureCaseV1` domain/ID/checksum guards survive. `/2` carries original root policy; async restore verifies exact immutable row/policy in the caller's transaction. Borrowed counting, checked output allocation and streaming canonical comparison remain connected. `/1` stays historical. |
| `pg_private_delivery.rs` | Delivery restore takes its existing `tx`; K's `io::acquire` remains. |
| `pg_monty_private_admission.rs` | Both live-completion and new terminal-status readback restores take their existing `tx`. |
| `qualified_pure_source_review.rs` | Source original restore takes its existing `tx`; current I/OriginalReview/custody guards remain. |
| `private_task_catalogue.rs` | All three restores take their own existing transaction. Four actual capture paths converge on verified retention. Exact typed private INSERT and K's `io::acquire` remain together. |
| `private_pure_case_work.rs` | Original Pure-case restore takes its existing transaction; its real J workflow is preserved. |
| `runtime/tests/validator_retained_q1_tests.rs` | Existing restoration check uses a coherent test snapshot. |
| `runtime/tests/qualified_source_review_tests.rs` | Only the existing restore check gains a coherent snapshot; current OriginalReview/watchdog/pool-custody changes remain. |
| `runtime/tests/private_work_admission_tests.rs` | Actual `/2` ACK markers and original-policy/canonical checks remain. |
| `runtime/tests/original_pure_case_tests.rs` | Existing actual native return/exception/contract-defect case additionally proves applied V213 and restores its real `/2` Work/root/profile in one snapshot. No SQL-simulated Case or successful native result is substituted. |

The complete production restore inventory is eight: delivery; live completion;
terminal-status readback; Source; original Pure case; three catalogue paths.
`restore` cannot return a definition before root-row/profile verification.
Single immutable-row verification supports the existing custody transaction's
isolation; full graph selection retains its separate RR requirement. No nested
pool acquisition was introduced by production restore.

V213's complete OriginalPureCaseV1/prefix/validator domain-guard suffix is
byte-identical to the actual integrated V211 function. Only the new definition
shape/root-policy and original-size guards are L additions. All 209 existing
migrations in current main, including V211 and V212, remain byte-identical.
K's free `io::acquire` adapter and its callers were preserved, not reimplemented.

## Three explicit review corrections

1. Existing EN/DE depth/nodes/bytes help now says that acknowledged limits apply
   to new component revisions, existing revisions retain their original parser
   profile, and graph/HTTP/IPC transfer and heap limits are separate. The current
   64-level support boundary remains explicit. No settings tab was added.
2. Original-row metadata computes presence from **either original column before
   bounded CASE projection**. Only actual NULL/NULL can yield Legacy. A non-NULL
   short checksum with NULL bytes cannot disappear into `(false,None,None)`.
   The focused reader regression covers the projected-corruption shape; the
   existing native partial-provenance upgrade case additionally checks a real
   rejected checksum-only/short-hash write. No trigger is disabled.
3. The complete unnamed exchange buffer must fit native `isize::MAX`, in addition
   to checked usize arithmetic and each PG message's own representation. There
   is no new aggregate MaxAllocSize quota. The scalar boundary case models a
   32-bit buffer limit with two individually valid messages, without GiB
   allocation, and checks exact N/N-1 native capacity.

## Checks performed and acceptance still required

Performed source checks: changed-file Rustfmt parsing/format checks; Node
`--check` for both EN/DE help files; static enumeration of all eight transaction-
bound awaited restores and four original Pure schema parses; one shared PG
capacity constant; exact current V211 domain suffix; existing migration byte
comparison; current I Source/test diff review; K adapter/caller preservation;
and final staged whitespace check. These checks provide no native execution
or approval evidence.

Final source manifest:
[`2026-11-10-l01-j01-integration-followup.sources.sha256`](2026-11-10-l01-j01-integration-followup.sources.sha256).
It covers every changed Rust/SQL/JS source relative to a8ace38b6, including the
EN/DE files; historical attachment snapshots are intentionally not source inputs.

No local Cargo, test compilation, Rust/native tests, DB, SSH, provider, Modal or
remote-test action ran in this follow-up. Root owns the necessary serial local
build/acceptance batch. The previous L native size/node/pair/original-readback,
V212-to-V213 upgrade, ACK/cancellation and detached-work regression sources
remain UNRUN. Also UNRUN: the connected genuine J Pure case under V213 and the
new parser-presence/native-buffer boundary cases. Node syntax checks alone do
not establish new revision semantics or runtime uptake.

This package changes immutable revision resource provenance and its actual
consumers. Recipes/PythonCode still own workflows; existing catalogue, ACK,
immutable stores, global VM and kernel policy owners are reused. No component
approval, active publication, wider depth support, new wrapper/Tool, fallback,
full-v3 or deployed-support completion is claimed.
