# Exact K3-2: native model setup drafts — 29 October 2026

The [machine receipt](2026-10-29-exact-model-setup-draft.json) binds eight changed
sources, V188, the original source manifests, images, current worker and logs.
This consumes [Root's OAuth transport handoff](2026-10-29-oauth-token-transport.md)
and the [pure prefill candidate](2026-10-29-exact-bootstrap-prefill.md).
**The complete bootstrap/import/UI/design/Free implementation is not accepted.**

## Delivered owner and service

V188 adds append-only, unaccepted setup revisions under the existing
`PgPrefixModelDesigns` owner. The original specification row is the serialization
address. No Provider head, accepted model/prefix design, component approval,
second provider catalogue, task registry, job queue or lock service is added.
All old specification bytes and migrations through V187 are preserved.

The existing InterceptorConfigService and Reborn facade expose get/update methods.
A get may request the current view or an exact revision; explicit0 returns the
untouched original configuration view, without a stored operation. Missing
positive revisions never choose another view. Original configured limits retain
configuration provenance; unknown values stay unknown after later Provider edits.
Namespace/reference/provenance/hash/shape errors fail closed.

An update requires the caller's original nonnil operation UUID, whole-draft
expected revision and nonempty distinct field edits. Explicit null clears a value;
missing is invalid. The four nullable token fields have exact integer shape/bounds,
with0 allowed for Prefix/framing and positive set prompt/output values. Any explicit
manual edit marks Manual, including an unchanged value or clear-to-null. This
protects operator intent against later automatic prefill. It changes no canonical
Provider policy and does not prove budget fit, endpoint facts or design readiness.

ReadCommitted plus the original specification-row lock serializes competing
updates. Whole-revision CAS fences the complete four-field snapshot; no partial
merge/rebase occurs. The immutable row carries the original operation, basis,
normalized edits and complete draft bytes/checksum. A retry reads that exact old
operation after later changes without replay. A separate stale action with equal
values cannot claim the old receipt; altered basis/payload under the same ID
conflicts. The actual INSERT-returned bytes/checksum are verified before commit.
SQL protects predecessor continuity, uniqueness and immutability.

Nullable output values and stored operation IDs are required fields. Hash-consistent
rows with missing values or false configuration origin are rejected. A current
corrupt row never silently selects an older one; explicitly requested historical
views/original operations remain distinct intentional reads.

## Consumer boundary

No HTTP route, browser control or automated import is added. The existing product
service is exercised with real PG and works without an inference endpoint or
orchestrator. Later UI must keep original intent IDs stable across retries, re-read
its current owner and avoid publishing an old retry receipt as the current head.
Running jobs must retain exact draft revisions and use the historical reader.

The [precise contract](../plans/Exact_V3_definition_update_at_K3-2.md#176-nativer-unangenommener-setupentwurf-und-cas--2026-10-29)
requires native automatic prefill to check original Use/pair/provider/job/source/
rule provenance and the complete read set from the pure candidate. The manual
writer is not a shortcut for autoimport or a fact/approval verifier. Future import
must reuse this model owner rather than add a shadow store or label LLM data Manual.
Reserves are draft construction inputs, not new global request defaults.

## Validation and evidence

Final run-4 executes ten distinct cases freshly with zero ignored: five setup
cases (one pure serde/schema case plus four actual PG cases) and the existing
five model tests (three PG and two pure cases). The actual tests prove concurrent
CAS, immutable history/original retry, exact old/initial/missing revision semantics,
protected null/zero, canonical Provider preservation, namespace rejection,
statement immutability, hash-consistent corrupt-origin/missing-value rejection,
and the actual existing product service without model/VM. PostgreSQL applies the
real V188 schema; no historical production migration/constraint is weakened.

Composition connected lib/tests, Product root-feature lib/tests and relevant
Product/Composition default lib strict Clippy pass with -D warnings. Seven Rust
sources pass rustfmt; all eight source files and new docs pass whitespace/link
checks. The current actual worker is601b976da41c65bb48d237b32db0bc6975e2b30056dae28c86f84ac2f9e5f205;
old91a… was never copied over the canonical target. No live model submission,
public HTTP/UI acceptance, deployment or Git push is claimed.

Run-1 exposed an incorrect pool-transaction adapter borrow; it was corrected to
the actual tokio transaction and explicit isolation. Run-2 passed the initial ten
cases/stricts; run-3 added and passed exact historical reads. Before acceptance,
review found missing nullable output values could default and semantic-equal
stale edits could be mistaken for retries. Required nullable decoding, real
operation identity and real PG regressions close those issues in run-4. Earlier
source/image/log scopes are retained as history, not substituted for final cases.

## Remaining work and handoff

HTTP/UI, Use/discovery, native correlated B1/B2 import and any automatic canonical
Save/readback remain open. Budget/context/encoding, actual design Recipe checks,
manual design release, Prefix-Q1/Q2, protected physical GC and active Free child/
review workflows remain open. This closes a native data-owner building block,
not I1/I3 or full Exact. No activation follows a draft save.

Source/Cargo returns explicitly to Root/Auth after this package. Exact freezes
shared executable/embedded inputs until the next explicit handoff.
