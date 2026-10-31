# Exact K3-2: manual model setup draft HTTP and WebUI — 30 October 2026

The [machine receipt](2026-10-30-exact-model-setup-ui.json) binds sixteen changed
sources, the actual source manifests, images, worker and logs. This extends the
[V188 native draft owner](2026-10-29-exact-model-setup-draft.md) after the
[DCR handoff](2026-10-29-dcr-http-settings.md). No migration is added; V188/V189
and all eight native draft sources are unchanged. This is a scoped consumer
acceptance, **not complete Exact/bootstrap/design/Prefix-Free acceptance**.

## Connected behavior

Two operator routes share the existing descriptor/auth/body/rate policies:
GET/PUT `/api/webchat/v2/prefix-model-specifications/{specification}/setup-draft`.
GET reads the current view, or an explicitly requested exact `revision`, including
original revision 0. Unknown query fields and malformed revisions fail. PUT accepts
only the existing typed original operation UUID, whole-draft expected revision and
explicit field edits. The handlers use the existing product facade/service and
native model owner; they never access stores directly or approve a component.

The existing Model Designs view mounts the editor when a captured pair's details
open. It shows the captured configured context (or Unknown), all four editable
nullable construction values, provenance and draft revision. Original configured
context is not a verified model limit. These are pending design inputs, not a
second canonical Provider policy. Prefix/framing 0 remains valid; set prompt/output
values are positive. Blank is explicit null; missing fields are invalid. Direct
input events retain clear-to-null and explicit same-value manual intent. Saving
adds a native draft revision only. Generate/accept readiness is unchanged.

The browser creates an original secure operation UUID once per manual save. An
unresolved request retains its original basis/body/UUID across view remounts and
locks further edits. Explicit retry coalesces the same original operation. A lost
acknowledgement is checked against its exact native successor; another operation
in that slot proves Conflict and requires explicit reload while preserving dirty
inputs. There is no silent rebase or automatic new action ID. A confirmed receipt
is followed by a normal current read; historical retry results never become the
current head. If that read fails, retry only re-reads it. View epochs fence obsolete
async updates. No draft body is persisted in browser settings,
and no second backend queue, registry, grant or approval store is introduced.

## Actual validation

Final run-7 reuses the exact source-matched run-6 image, 16 Node cases (8 new
intent/shape cases and 8 existing model-view regressions) and 9 module syntax checks.
It executes the complete real native component-deployment
integration extended with draft HTTP assertions and actual Chromium, the exact
route-descriptor contract and affected connected/default strict Clippy checks.
No ignored case is counted as acceptance. The machine receipt carries exact counts
from the logs and the complete source/image/worker bindings.

The actual operator middleware, facade and PostgreSQL prove unauthenticated
GET/PUT rejection, exact initial/history reads, required null/zero, unknown/malformed
input rejection, stale/changed-intent conflicts, immutable original retry after a
newer head and preservation of current data. Chromium runs the production editor,
React and existing API transport against that real router. It verifies save and
current readback, concurrent-writer conflict without discarding local inputs,
original retry after lost committed acknowledgements and remount, locked fields,
explicit same-value input plus null clear, acknowledged-save/head-read failure
without another PUT, and invalid prompt 0 rejection. Only transport acknowledgements
are cut; no storage or success response is fabricated. Faults persist through
Chromium's transparent idempotent HTTP retries until the explicit test retry.

Earlier runs remain failed intermediate evidence: run-1 exposed CLI eval encoding;
run-2 was closed after transparent HTTP retry resolved a one-shot fault; run-3
exposed another snapshot-format comparison; run-4 reached successful original
retry and exposed empty-input event handling plus a brittle ref comparison.
Run-5 proved the clear but exposed the host-dependent select-all shortcut in
the same-value test; the final probe selects the existing field contents directly.
The final editor uses direct input events, and the browser probe uses exact DOM
conditions and format-independent refs. Run-6 passes every new HTTP/browser case,
then stops at the surrounding fixture's missing model-asset environment. Run-7
uses Root's original six offline artifacts with their exact provenance/file hashes,
without downloading or fabricating model data. No failed run is relabelled passed.

Seven changed Rust files pass rustfmt. Only additions against this package's
captured beforecopies are checked for whitespace, preserving incoming Root bytes.
Documentation links and final source readback are checked. The canonical current
worker remains `ece271358a476130cb09d695e321eb485e00e1670e89a4704aad4098dee4cb7c`;
old companions are never copied over it.

## Remaining work and ownership

Use/discovery and durable Use→job binding, native checked B1/B2 automatic import,
qualified Prefix-Free child/review dispatch, model/design generation and release,
full Prefix-Q1/Q2 and reference-safe retention remain open. The manual writer is
not a shortcut for model-result provenance or autoimport, and reserves remain
construction inputs rather than new global request defaults. No deployment,
public activation, Git commit/push or full I1/I3/I4/Exact completion is claimed.
After the terminal receipt, Source/Cargo returns explicitly to Root/Auth; executable
and embedded Exact inputs remain frozen until the next handoff.
