# OAuth setup original metadata — 28 October 2026

Status: **Auth contract foundation checked; Google runtime integration remains open**.

`OAuthSetupDefinition` records the first scoped flow identity, usage/previous
selection, exact update-account snapshot, public authorization challenge, hashes
and original timestamps. `OAuthSetupOriginal` associates it with an authenticated
encrypted generation. Context excludes its own reference to avoid a circular
identity. Raw verifier, client secret, code and token fields are not part of these
Auth records. Private client/PKCE bytes must be captured atomically by the existing
encrypted owner when this contract is connected.

Structural validation rejects incomplete or mismatched bindings/snapshots and
changed original identity/generation. Live reconstruction rejects elapsed expiry.
Cancelled/expired projections preserve first identity/timestamps and carry no
challenge, credential result, callback code or emitted continuation. The existing
callback-domain boundary rejects those terminal projections. These constructors
do not persist a flow or establish authority. The concrete owner still must verify
encrypted custody, preserve selected material, fence prior effects and win its
existing absent-record CAS. No new store, Tool, registry or activation API exists.

Receipt: `target/validation/google-setup-metadata-20261028/run-1/accepted.json`,
SHA256 `08a8656214ac5374362a5cc055ad46fea5cba82231d79dfacbd0f2fc0e39c06f`.
Three-file owned manifest SHA256:
`4deb5aeda7a3a6d2a15f3303296efbc8c0958075f5b06de51913019c3321fe07`.
Selected-source inventory SHA256:
`afea964349652818e20564c7a1ad03721355e584ba4f623b21a9f2af4afdf450`.
Originals and proposals: `/private/tmp/brassclaw-google-setup-20261028-wzsh6rx6/`.

All **116 actual Auth tests pass, zero ignored**: library5, public contracts95,
DCR contracts12 and new setup contracts4. Auth formatting, strict all-target Clippy,
actual test-image compilation and whitespace pass. The new cases cover original
retargeting, snapshot/generation replacement, terminal callback rejection and raw
private-field rejection. No native backend, browser, restart or Google issuer test
is claimed for this new contract. The serial Screen runner is terminal; its process
handles are absent and the shared Cargo lock is verified free. Prefix sources and
the accepted DCR-owned file set were unchanged.

Continue K3 step4 in the existing Auth/SecretStore/provider owners: retain this
metadata beside the original flow, add concrete authenticated original admission/
readback/retirement, capture complete private Google client plus verifier atomically,
then connect public/gate producers, callback and refresh material consumers. Missing
flow alone is not proof no exchange occurred; later authenticated artifacts must
prevent reconstruction. Preserve first IDs/configuration/expiry across unknown
writes and restarts. The current process-local public verifier cache, ordinary gate
verifier storage and current static-client exchange are not migrated by this packet.
Full K3, Prefix, Validator, Composition, upgrade and deployment gates remain open.
