# B3-09 provider mutation original-auth context

Date: 2026-11-03. Scope: browser transport custody and caller behavior only.
This is not native Auth/issuer/PostgreSQL, installed-asset or full-plan acceptance.

## Cause and resulting behavior

The shared provider helper previously indexed pending requests only by operation
and target, retained exact POST bytes (including a newly entered API key), and
used apiFetch's current bearer for follow-up calls. After authentication changed,
an old request could project success, inspect/reconcile automatically using the
new bearer, or supply its original bytes to a later same-target invocation.

The helper now reuses api.js's existing nonsecret origin/auth nonce context and
captured-token comparison. Pending keys include that context. The original
context is checked before POST, after its response and throughout automatic
reference read/reconciliation. Loss of current context rejects that old submit
with its original mutation ID instead of reporting success or restarting it.
Recognized abandoned entries discard reusable POST bytes and retain reference
metadata. Current-context status lists do not adopt other contexts' originals.

A deliberately submitted new edit has its own request bytes and mutation ID;
it cannot reuse the old API key or identity. Existing backend revision/selection
CAS, exact original outcome storage and operator authentication remain unchanged.
This does not infer cancellation, rollback or absence of an external effect.

An auth-context fence is not an additional permission rule. Explicit reference
inspection/reconciliation of a known older mutation remains available to the
current authenticated operator and sends no old provider payload. Its current
observer is fenced across the network wait. A replayable body is not restored
even if an original auth observation subsequently becomes current again.

## Source review and checks

The patch was prepared/tested outside the checkout during another chat's Cargo
run. Source review caught an over-restrictive initial draft that also blocked
explicit reference inspection under a new auth context; that restriction was
removed before final acceptance. Tests now cover both absence of automatic
cross-context recovery and successful explicit reference-only inspection.

The final tests ran directly from the integrated repository, without temporary
source loaders or path overrides:

```sh
node --test crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-mutations.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-bootstrap.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-mutation-auth.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/codex-login.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/provider-login.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/hooks/useProviderLogin.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/components/provider-mutation-status.test.mjs
```

Result: 73 tests, 73 passed, zero failed/cancelled/skipped. See the
[actual final output](2026-11-03-b3-mutation-auth.final-ui.log) and
[source identities](2026-11-03-b3-mutation-auth.json).
The earlier four-case [repository log](2026-11-03-b3-mutation-auth.repository.log)
is retained as historical intermediate evidence, not the final explicit-reference
contract's acceptance. Intermediate scratch runs are not additional distinct
accepted cases.

The new settings caller tests exercise the actual settings-api module, actual
mutation helper and actual apiFetch with controlled storage/HTTP responses.
They verify bearer use at each boundary, no automatic requests after context
change, distinct explicit successor bodies/IDs, no API keys in browser storage,
and logout rejection before HTTP. Existing suites cover login/status/ordering,
uncertain outcomes, same-original retries and canonical query publication.
The mutation suite additionally checks that retired bytes cannot become another
POST and that reference-only recovery still works.

## Build and architecture boundary

Local Cargo/test execution was terminal and the common lock was free before
integration. Both application stages verified before/after source hashes under
the common lock. No Cargo, cleanup, remote compilation or source upload occurred.
No Rust/validator/PG/Modal implementation was changed by this work package.

These JavaScript assets require a later coordinated native build before they
are installed. No real browser rendering or provider/issuer/database behavior
is certified here. B3 native owner/refresh, original account/head readiness and
remaining review projections still require their named connected acceptance.
