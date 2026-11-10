# L-01 — original component-revision parser policy

> Historical checkpoint only. Superseded by the [2026-11-10 connected follow-up](2026-11-10-l01-j01-integration-followup.md).
> Its old manifests/attachments describe the pre-integration inputs; do not apply
> them or replace current files with their derived copies.

Status: **base source checkpoint and regression sources only; acceptance UNRUN. Owner-side semantic rebase onto Root’s actual J+I+K integration is REQUIRED before L uptake. This diff is not independently integratable.**
Branch `codex/l01-revision-policy`, based on
`354c05baab8a47429c07a1db6a3447b860bbfd8f`.
Only migration **V213** is added. V211 belongs to J-01; V212 belongs to K-01.
No old migration is edited. Root owns integration, local compilation and tests.

## Architecture and connected behavior

The four component guides, root/current crate guides, development policy,
implementation index and three Repair_bob owners were read for this package.
Recipes/PythonCode retain workflow ownership; this change supplies immutable
storage/resource provenance. No new Tool, Recipe, approval service, execution
fallback, activation, Q1/Q2 result or deployed-support claim is introduced.
The actual Composition acknowledgement owner and existing neutral
`RetainedPreparation` codec are reused. Encoding a codec fixture or constructing
`PreparedReviewContext` does not establish an ACK.

`component-revision/1` exact content and its checksum remain unchanged. V213 adds
nullable original preparation bytes/checksum to the same immutable revision row.
NULL/NULL means the historical depth64/nodes262144/bytes8388608 profile. A captured
row uses its own canonical original `max_value_depth/nodes/bytes`, with checked
native conversions. Depth above64 returns explicit `UnsupportedDepth`; there is
no clamp or claim that64 is an intrinsic JSON limit. Wider owned-value/schema/
worker migration remains open.

The real acknowledged HTTP/Composition writer parses each new candidate and
changed partner under the first accepted capture. The in-memory partner retains
its profile without adding fields to historical subject bytes. Historical subject
restore reads the actual primary/changed-partner rows first. In particular, an
old `/4` subject capture never relabels its historical revision parser policy.
Base/context reads also use their own exact row policy. Original-ID retries use
the actual original row profiles, including when offered newer settings differ.
A direct same-subject retry with a different row profile conflicts. Identical
packaged bytes reuse the old reference and old row policy.

Graph reads retain K-01's explicit graph-owner limits, sorted metadata-first
selection, complete streaming batches, membership and digest checks. Each row's
parser profile is independent of that graph's transport policy. Metadata now has
eight fields: actual field widths (including both original-capture strings) are
checked during collection before requesting content. SQL bounds the small carrier
by the neutral codec's derived maximum, not an arbitrary new cap.

Revision/subject INSERTs use the actual unnamed typed encoder and checked request
sizes before candidate allocation. Parsing uses original `max_value_*`; original
PG transfers use `max_ipc_frame_bytes`. Single revision content is a one-field
DataRow, graph content is the unchanged five-field shape. Intrinsic wire helpers
count unsigned16 field/parameter counts, signed message lengths, text varlena and
whole StringInfo payload plus terminating NUL. A sequence of messages is not one
PG message or an additional aggregate representation quota. Graph sums remain
logically larger and streamed. The shared PG owner defines one MaxAllocSize.

Recipe task schemas, per-code input/result schemas, Tool parameter schemas, pure
component schemas and MCP command/schema metadata consume the actual corresponding
revision's `parsing_limits()`. Runtime inputs retain their separate task/HostCall
limits. Unrelated fixed approval/evidence, retained-selection and native-image
loader records are not blanket-migrated.

Private capture writes `private-work-definition/2`: it binds the actual root row's
original preparation (or explicit legacy NULL pair) inside the existing definition
checksum. `/1` decoding retains its historical shape and legacy parser. All actual restore callers use an async transaction-taking `restore`: its private
decoder cannot return an externally usable definition before the actual immutable
row/profile has been checked. This base has six production restores; J adds the
second terminal-status restore and the original Pure-case restore, for eight.
The four final J/L capture paths all reach `retain_private_definition`, which
verifies the same row/profile before insertion. The root store's narrowly scoped
`verify_original_in_transaction` supports the existing custody transaction's
isolation (including read committed) without selecting a mutable catalogue:
exact immutable row metadata and content cannot change between reads. It uses
the caller's same connection; complete graph selection retains its RR requirement.
 Detached self-declared
policy cannot establish provenance. Historical restoration does not substitute
today's startup root. The factory's real global-root source checksum and worker
identity checks remain technical admission gates.

Private `/2` serialization now uses a borrowed wire view: existing
`count_json_bytes` counts full escaping under the Work's ORIGINAL
`max_value_bytes`, checked native/PG representation and the original
`max_ipc_frame_bytes` before root strings are copied or output is allocated.
A fallible exact reservation and bounded writer emit the counted output. The
actual private INSERT uses that exact checked unnamed typed representation.
V213 checks complete `/2` stored size against the same original Work policy.
Canonical `/2` restore compares Serde output directly against borrowed original
bytes with a streaming writer; it creates no second canonical output buffer.
Historical `/1` bytes receive neither this new value gate nor canonical rewrite.

The live HTTP body cap remains independently effective before decoding a
submission ID. A previously large request can therefore receive413 after that
cap is reduced. A retry that reaches the original-ID store returns its original
receipt; GET/direct historical reads retain the original. The new native case
asserts both behaviors explicitly, rather than inventing transport admission.

## Required J-01 integration attachment

**J must be integrated first. This standalone base diff is not a complete,
independently integratable package.** V213 now derives from the actual latest
J V211 `verify_private_preparation_original` replacement. Its full additional
OriginalPureCaseV1/prefix/validator domain suffix is retained byte-for-byte;
only envelope /1-/2 shape and original root/profile guards change. V211 itself
is not edited or copied into this branch.

Root supplied read-only `/private/tmp/brassclaw-j01-implementation` sources.
The tracked complete attachment is
[`patches/2026-11-09-l01-j01-connected-originals.patch`](patches/2026-11-09-l01-j01-connected-originals.patch),
with its exact input manifest
[`patches/2026-11-09-l01-j01-inputs.sha256`](patches/2026-11-09-l01-j01-inputs.sha256).
Derived, formatted final copies are also in
`/private/tmp/brassclaw-l01-j01-connected` (J input tree remains untouched).
This patch is against those explicit J inputs, not an already partially
cherry-picked L merge. It uses zero-context hunks (`git apply --unidiff-zero`);
verify the exact input manifest first. The derived files are design references,
never replacements for newer integrated files. It records the intended semantic changes at eleven overlapping
files for the L owner's required J-first rebase, including:

- Both J-shaped pure schema sites: moved shared helper and new
  `InspectedPureSelection::invocation`, using each actual root's original
  profile, retaining genuine HostCall `bind_with_limits` for input values.
- Full `/2` private definition codec retaining J's exact `OriginalPureCaseV1`
  enum and request/work/run/checksum guards, with mandatory async same-transaction
  immutable-root verification before every restore returns.
- All eight production restores: delivery; live completion; new terminal-status
  readback; Source readback; three catalogue paths; original Pure-case Work.
  No nested pool get and no current startup-root substitution.
- Existing old-codec tests receive an actual caller-owned snapshot; private ACK
  matchers use the actual `/2` writer.
- J's genuine native return/exception/contract-defect test now checks applied
  V213, its actual original Pure-case `/2` definition/domain/profile fields and
  the transactional restore after its real native completion. No SQL-generated
  Case/effect substitutes for that invocation.

After Root supplies its actual J-integrated head, L-01 must rebase/integrate that
head on this owner branch and resolve all eleven semantic connections against
the then-current J+I+K source. The attachment is an aid only: its older J input
can lack newer OriginalReview/watchdog/pool-custody corrections. Never copy its
whole derived files over newer main files. Do not apply the old pure helper
hunk twice or replace J's case branches. Root awaits the coherent owner-side
follow-up before L uptake/Run26.
Root also combines J's `MAX_ALLOC_SIZE`, `maximum_text_content_bytes`/
`text_record_fits` and L's unnamed/DataRow functions in one `wire_capacity` owner.
The source-only attachment has parsed/formatted, but compilation and all runtime
acceptance, particularly the genuine J case under V213, remain UNRUN.

## Regression sources and supervisor acceptance

All selectors below are **written but UNRUN**, including test compilation.
Only one new Commonboot test combines the connected larger-revision cases.
The separate upgrade case uses native PG without booting a global runtime.
Root should use the smallest relevant native batch, serial local target/lock and
an isolated external TMPDIR per process, as already instructed.

- Composition `native_l01_acknowledged_revision_profiles_keep_bytes_nodes_pairs_after_reduction`:
  actual ACK and mounted authenticated HTTP; >8MiB UTF-8 class22 revision,
  >262144 nodes, two large changed function roots, exact immutable profiles,
  original read/graph/facade retry after all three value limits shrink, fresh
  rejection without partial heads, separate HTTP413, actual eight-field metadata
  response at N/N-1 with real acknowledged graph transfer settings.
- Composition `native_l01_v212_upgrade_preserves_legacy_row_and_preexisting_v4_subject`:
  real V212 DB, actual V213 migration/repeated migration, original migration
  checksums, legacy row deeper than its `/4` capture48, exact old subject bytes,
  identical packaged row reuse, parser-N store/readback with separate IPC/graph
  budgets, and partial provenance rejected by the real DB constraint. Codec
  data here asserts no runtime ACK or approval.
- Existing Composition `native_k01_real_http_pool_one_held_begin_commit_and_unknown_ack_keep_original`
  is extended with actual original row-policy/content atomicity assertions;
  its original transport/cancellation/pool-one oracles remain intact.
- Existing Composition
  `private_same_id_capture_race_ready_reconstruction_and_worker_fallback_keep_original_bytes`
  now checks genuine detached `/2`, legacy `/1` reconstruction against the real
  row, and rejection of a correctly hashed self-relabelled root profile. Three
  existing private producer ACK matchers now identify the actual `/2` writer.
- Skills integration tests
  `original_revision_parser_bytes_nodes_and_depth_have_exact_separate_bounds`
  and `association_schema_uses_its_revision_original_nodes_without_a_legacy_reparse`:
  exact parser bytes/nodes/depth, explicit unsupported depth, larger association
  schema. These neutral codec fixtures supply no ACK/evidence.
- Engine `retained_schema_consumers_use_each_revision_original_profile_in_a_mixed_graph`:
  real retained preparation with legacy Recipe and captured large pure result
  schema. It asserts preparation, not activation or dispatch.
- Composition `command_metadata_uses_original_recipe_profile_without_claiming_mcp_qualification`:
  >8MiB command metadata uses original Recipe limits and real command binding;
  it asserts neither discovery publication nor MCP-call qualification.
- Composition `private_v2_complete_encoding_counts_escaped_root_and_compares_canonical_without_second_copy`:
  complete escaped-root representation at exact original byte N and N-1,
  original capture's own decimal budget encoding, streaming canonical comparison
  and trailing noncanonical rejection. Neutral serialization mechanics only.
- PG `unnamed_revision_requests_and_all_row_shapes_count_complete_payloads`:
  exact request/one-/five-/eight-field sizes, overflow, unsigned count width,
  individual text and whole DataRow/StringInfo boundaries without GiB allocation.

The older function-subject upgrade fixture now explicitly projects historical
revision columns into its V101 table, rather than SELECT * from the extended
table. No historical byte normalization or trigger disabling was added.

## Checks and limits

Performed here: affected API/source inspection, pinned postgres-protocol0.6.12
and tokio-postgres unnamed/bind encoder inspection, changed-file Rustfmt and
`git diff --check`. The final source manifest records byte SHA-256 values.
No local Cargo, test compilation, tests, DB, SSH, provider or Modal job was run.
There is no demonstrated >=60s useful non-test build requiring Modal here; no
cloud cost/provisioning was introduced. Formatting is not runtime acceptance.

Remaining acceptance includes Root's actual native results on the REQUIRED integrated J/L
shape, the attached genuine original Pure-case/V213 regression and new J invocation schema consumer, and the applicable existing
K ACK/capture/graph and private producer cases. No platform-wide, all-Bob-plan,
full-v3, large-image loader, broad depth>64, approval publication or crash-recovery
completion is claimed. Evidence256/64MiB and native-image loader quotas remain
separate owners, as recorded in the proposal inventory.

Final source-only checks passed: changed-file `rustfmt --check`, all 208 tracked
pre-V213 migrations byte-equal to the base, J V211 complete domain suffix
byte-equal in V213, and static caller inventory requiring the existing
transaction/await at six base and eight integrated production restores. The
11-file J-derived attachment passed Rustfmt parsing and `git apply --check
--unidiff-zero` against a separate copy of its hash-verified J inputs. The J tree
was not edited. Source manifest: `2026-11-09-l01-revision-policy.sources.sha256`
(26 paths). The final staged whitespace check passed before this review checkpoint.
No compilation, test, DB or runtime result is implied by these source checks.
