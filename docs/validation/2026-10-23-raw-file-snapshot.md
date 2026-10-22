# Complete UTF-8 file capture: scoped native validation

Date: **2026-10-23**. Common root `/Volumes/SSDE/brassclaw`, `main`, base
`19cf6382f697cc52d272629cadfaf525e1e8de41`. The changed primitive/registration and
real caller tests are identified by complete source inventories in the archives
below. This is not canonical component approval, public Recipe activation,
release/platform acceptance or a deployed testmachine result.

## Accepted behavior

The existing `builtin.read_file` now accepts `representation="raw_utf8_preserved"`.
It returns complete captured valid UTF-8 and a host-assigned invocation/content
reference without line prefixes, BOM removal, newline conversion or replacement
decoding. Empty files are valid. Raw ranges and unknown fields fail before file
I/O; invalid UTF-8, UTF-16, binary and oversized reads fail without partial success.
The default numbered preview keeps its existing behavior. See
[the exact input/result contract](../reborn/raw-file-snapshot.md).

Actual registered native callers exercise Unicode/BOM, CR/LF/CRLF,
final/no-final newline, empty files, source-like text, content exceeding the
preview line default, changed source under the same contained invocation,
invalid inputs/encodings/size and denied mounts. Actual WebUI-facade/PostgreSQL
settings publication changes the retained handler's read-byte limit while the
instance global Monty identity remains unchanged. A current stable-Tool policy
block denies the existing raw handler with an authorization outcome.
Actual memory/PostgreSQL filesystem callers reject structured-record reads,
including a record transition after stat and before the bounded read.

## Commands and retained evidence

All Cargo operations are serial from the common repository root under
`/private/tmp/brassclaw-target-cargo.lock`, using Rust **1.96.0**, target
`/Users/ollama/brassclaw-target`, no incremental compilation and no dev/test debug
symbols. The real `monty_worker` binary is retained beside the composition test
executable; there is no test-worker or Rust-loop substitution.

The test build selects `brassclaw_monty_host`,
`brassclaw_first_party_extensions`, `brassclaw_host_runtime` and
`brassclaw_reborn_composition`, `--tests --bins --locked`, with composition
`skills-db,test-support,migrate-from-libsql,bedrock` and `brassclaw_llm/testing`.
Strict Clippy selects the three affected extension/runtime/composition packages
and their test targets with those same features, `--locked -- -D warnings`.

| Archive / executed selector | Result |
| --- | --- |
| `target/accepted-evidence/r10-raw-source-20261023/run-2/first-party-extension-units.log` | 50 passed, none failed/ignored. |
| `run-2/native-coding-callers.log` | 23 passed, none failed/ignored. |
| `run-3/live-webui-coding-profile.log`: `native_live_coding_limits_reach_retained_primitives_from_webui_settings` | 1 passed; command 35.960 s. |
| `run-3/structured-entry-race-both-backends.log`: `native_coding_primitives_protect_structured_entries_on_both_backends` | 1 passed; command 4.976 s. |
| `run-3/affected-strict-lint.log` | Passed in 91.102 s, no suppressed warnings. |
| `run-3` format / whitespace | Passed. |

These are **75 selected tests**. Other compiled targets are not claimed as run.
The successful 73 extension/native tests from run-2 are reused: comparing its
source inventory with run-3 finds only the composition test fixture changed.
Primitive, registration, schemas and those test sources are identical.
`run-3/accepted.json` records that scope, source inventory, commands and all
**60 retained executable hashes**. Its worker SHA-256 is
`569b787d0e4d8a67fd32570e7e9ff23208efae47bcd4aa5dcce14341feb23f66`;
composition tests are
`872a35a17328e541e116a68db31642879b8ba1056f9fdeeb8d633e26645f67b3`.

## Failures retained and corrected

Run-1 rejected one new negative case: the native handler had not enforced its
advertised closed argument set. Unknown provenance fields were ignored before a
missing-path error. The implementation now rejects unknown arguments before I/O;
the assertion was retained. Run-2's new policy test then attempted to publish a
snapshot with mixed rule revisions. The real publisher correctly rejected it.
The fixture now advances every rule to the same generation and asserts an actual
authorization denial after a valid block. No production policy validation or
failure assertion was relaxed; both failed archives remain available.

## Remaining work

Canonical raw Skill/export/ToolSkill/Recipe qualification, independent R10 pure
approval, source/derived persistence and doc-sync/doc-convert migration remain
open under the composition and validator owners. The snapshot is a captured
output identity, not a filesystem-wide atomic checkpoint or refetch permission.
No Prefix-owned workflow or store is changed.

## Final common application and embedded documents

`run-4/accepted.json` identifies the final compiled source, including the changed
embedded Changelog and component guides. The common application/actual worker
build passes (50.250 s), the application version smoke passes, and affected test
compilation passes (113.178 s). The actual native WebUI/settings/policy startup
case passes again (37.416 s); root/composition test Clippy with `-D warnings`
passes (76.412 s), as do formatting and whitespace. Unchanged extension/native
cases and the backend race reuse their named run-2/run-3 evidence; they are not
claimed as repeated tests. All **61 retained executable hashes** are verified.

The canonical target's `debug/brassclaw` and `debug/monty_worker` are restored
from these actual accepted artifacts after disposable-cache cleanup, with hash
verification. Application SHA-256:
`c82e474ae8f7701f2d06cd8017a517897efc650d13529ae9cc26789c766d8972`;
worker SHA-256 remains the value above; final composition-test SHA-256:
`bfff648818ed2dc41c97e5aa480e91541677e77f58e44f8da1c9d0620db63eb1`.
Only the unembedded validation/journal and composition-plan text are finalized
after this check. `run-4/final-source-equivalence.json` verifies the exact allowed
differences; no compiled implementation, embedded guide, dependency or build
configuration changed afterward.

The proposed Markdown R10 checker also passes **15 separately scoped actual
Monty utility evaluations**, recorded in `planned-checker.json` beside these
archives. Valid empty/Unicode/newline text keeps its reference; malformed/nil
UUIDs, wrong digests/versions, incomplete flags and numbered representation fail
with the expected exception. This checks the proposed function only, not a stored
component, typed binding, trusted review/activation or whole Recipe.

No GitHub CI run was created automatically for the consolidation commit:
repository tests are PR/merge-group/manual workflows and releases are tag-driven.
The local results above do not claim a remote full-matrix run.
