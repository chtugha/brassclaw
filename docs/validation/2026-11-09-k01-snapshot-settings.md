# K-01: retained graph preparation and HTTP admission

Source implementation on `codex/k01-snapshot-settings`, based on
`e00c867e62a039c9b8bc8389e7a25da20d42fb12`. This is a package handoff, not native
acceptance, installed uptake, component approval or full-v3 completion.

## Ownership and contracts

The existing acknowledged Monty settings owner supplies preparation. Host API
owns the pure `task-preparation-capture/1` codec; Composition keeps its existing
`PreparationCapture` facade and native UtilityLimits check. ResourceStore I/O
settings remain a separate current technical deadline. Preparation carries no
Tool permission, Work claim or approval. No new Recipe or workflow primitive is
introduced. Skills owns exact immutable revision reads; PostgreSQL owns protocol
representation and transaction connection custody.

New review submissions and authored catalogues write `/4` with exact original
preparation bytes/checksum. Their actual readers and V212 predicates are changed
together. V212 is reserved to K-01; V211 belongs to J-01 and is absent from this
branch. Integrate the coordinated migration sequence before native acceptance.
The existing Source facade signatures and F-01 same-transaction evidence/custody
logic are preserved for integration with J-01.

Legacy submissions retain their fixed graph/whole-subject profile. Legacy
catalogues `/2` and `/3` retain their recorded graph bounds intersected with the
historical graph gate; `/1` keeps its historical graph semantics while a fresh
task can use current acknowledged technical preparation. An old max_value_bytes
never acquires new PostgreSQL wire meaning. Store original-ID reads precede fresh task
settings/worker preparation; HTTP transport capture occurs before collection. Running/restored selections do not read latest.

## Connected implementation

- Exact graph reads require explicit SnapshotReadLimits at every call. All
  metadata batches complete before any content batch in the same repeatable-read
  snapshot. Streaming rows verify complete sorted membership, identity, lengths,
  checksums and closure. Logical selections are neither truncated nor limited to
  one driver request. Checked wire arithmetic and fallible graph-vector preflight
  precede body retrieval. A maximal legacy 8 MiB revision retains its additional
  117 bytes of row framing.
- The one existing transaction custody wrapper is shared from brassclaw_pg;
  uncertain/cancelled transactions detach their connection. Authoring retains
  one write and fresh exact readback after unknown commit acknowledgement.
- The three component preparation routes use a named finite policy. Actual
  operator authentication and rate checks precede capture and collection. The
  server accepts one acknowledged capture before reading a streamed body and
  passes it through the existing facade to the store. The default static host
  cap does not silently narrow this policy; an explicit host cap still does.
  Both declared and streamed lengths are bounded, including embedded extractor
  use. Capture failure does not substitute desired settings or a legacy default.
- English/German help and submission receipts expose the relevant settings and
  accepted preparation revision. No pagination or partial graph result is added.

## Validation performed here

Only source inspection, rustfmt parsing/formatting, JavaScript `node --check`,
`git diff --check`, caller inventory and exact changed-file hashing. No Cargo
compilation, test compilation, test execution, PostgreSQL migration application,
provider setup, component activation or remote build was performed. Literal `/1`
goldens are source fixtures checked by added tests, not executed acceptance.

The supervisor owns local Cargo and tests. No optional Modal job was launched:
the activated Apple controller supports a CLI build, while the smallest useful
check and its warm local duration had no credible >=60-second estimate. A heavier
build was not selected merely to qualify for cloud spending. Native target
cleanup remains supervisor-owned; this package did not alter target directories.

## Required native acceptance (unrun)

Compile the combined source once with the supervisor's supported native feature
set, then run the existing affected suites and these new selectors:

- `original_preparation_v1_goldens_preserve_legacy_bytes_and_strictness`
- `native_k01_acknowledged_http_graphs_above_fixed_caps_preserve_originals`
- `native_k01_real_streamed_batches_keep_complete_membership_and_original_limits`
- `native_k01_real_http_pool_one_held_begin_commit_and_unknown_ack_keep_original`
- `native_k01_desired_without_ack_and_streamed_http_keep_first_effective_capture`

The new native sources use actual factory settings/ACK, mounted HTTP, PostgreSQL
rows and the existing wire fault transport. They cover >4096 revisions, >64 MiB
graphs, >14 MiB HTTP/>16 MiB subjects, exact 8 MiB legacy revision bytes, paired
Skill/code count and atomic rollback, streamed batching, legacy restoration after
settings decrease, auth-before-collection, stale desired settings, accepted
capture retention and held/lost transaction acknowledgement. They do not invent
successful approval or provider responses. Existing task-preparation, qualified
Source, authored-catalogue, function review, deployment and descriptor tests also
need supervisor selection; adding sources is not evidence they pass.

## Remaining independent boundaries

The per-revision parser and V101 8 MiB limit, independent review/evidence quotas,
arbitrary larger shipped qualification and historical native-image loading remain
existing-owner follow-ons. Graph admission does not resize these contracts. Wire
and graph bounds are finite technical gates, not a promise that arbitrary native
heap allocation succeeds. This package makes no claim of general recovery,
approval migration or public deployment acceptance.

## Root review correction: exact HTTP and graph byte boundaries

Follow-up to `94946166df1c178d4cdbad88944dcd8173b0c063`, authorized after the
other implementation slot became idle. The existing
`native_k01_acknowledged_http_graphs_above_fixed_caps_preserve_originals`
selector now also contains both previously missing boundary cases. It reuses
its actual factory/runtime, ACK settings owner, operator authenticator, mounted
ComponentPreparation route and PostgreSQL; no second bootstrap or fake service
is added.

For each Content-Length and two-chunk transport, an explicit host cap N below the
acknowledged value cap permits a valid JSON request of exactly N bytes and
persists its submission with the accepted revision. Appending one legal JSON
whitespace byte yields 413 with the effective byte limit visible in the response.
The graph case computes S from the candidate's exact revision text and two
already retained leaf texts. At S-1 it expects 400 and zero candidate head,
revision, submission, queue request and queue binding rows. At S it expects
success and a complete snapshot whose exact revision-text sum is S. A subsequent
ACK decrease to S-1 must preserve the same-ID receipt, original subject bytes and
original capture bytes.

The descriptor resolver comment now says all three policy variants. The facade
contract was inspected read-only: its whole override is skills-db gated; without
that feature the trait default rejects. In the mounted legacy/non-global path,
no catalogue means capture returns None and middleware selects the explicit
legacy profile. No actual caller producing Some with no owner was found, so no
speculative production guard or warning suppression is added.

Follow-up checks are rustfmt on the two affected Rust files and git diff --check
only. No compilation, tests, DB, migration or Modal job was run. The selector is
unchanged; the supervisor must execute its expanded source. These additions do
not supply a native PASS or broaden the package's independent boundaries.
