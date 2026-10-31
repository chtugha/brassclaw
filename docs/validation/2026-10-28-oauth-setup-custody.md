# OAuth setup original custody — 28 October 2026

Status: **storage/callback contract accepted locally; ordinary Google material integration remains open**.

The existing Auth flow custody now retains an optional complete `OAuthSetupOriginal`.
Its definition pins the first identity, account snapshot, usage, challenge, hashes
and timestamps; its preparation reference pins one authenticated encrypted generation.
Old records retain their absent-custody representation. DCR and generic setup
custody cannot be mixed or retroactively attached to an existing legacy flow.
No new registry, Rust workflow loop, Tool or execution grant was introduced.

`AuthFlowManager::prepare_oauth_setup` and `read_oauth_setup` are connected in
the actual PostgreSQL, record-filesystem and in-memory owners. All wrappers and
test doubles implement the new interface explicitly. Native owners authenticate
the encrypted preparation's exact reference/context and reject every existing
flow-stable exchange-use marker, including retired history, before first insertion.
They use the existing absent-flow CAS and reconcile uncertain writes by reading
the exact original. Recovery never selects today's account or replaces a legacy
record. Missing flow alone is not evidence that no exchange occurred.

First insertion can create an awaiting, actually expired or cancelled projection.
Reading an existing original returns its existing state, even when a retirement
argument was supplied: this is readback, not cancellation acknowledgement. The
existing `cancel_flow` operation remains responsible for cancelling a live flow.
Terminal projections cannot rearm. Callback validation checks the retained first
provider, label, scopes, state hash and identity before the first claim or replay.

## Evidence

Receipt: `target/validation/google-custody-20261028/run-1/accepted.json`, SHA256
`fe9d5c13c5fcff21e1f6110de6a601490f3bc8b5b4da8d8530ab65e14c6e4810`.
Owned 26-file manifest SHA256:
`996132b2151af05e955106dc8040a442e245e1dd5ec4ff6c9dce392b62a334a5`.
Selected-source inventory SHA256:
`64beac438ccf2985cc04991c387cfcfb2dee3f5be4b9d6aed90a9e64cf05945e`.
Originals/proposals: `/private/tmp/brassclaw-google-custody-20261028-j_y3w7mg/`.

All **516 actual Rust tests pass, zero ignored**: Auth119, Product207, native
PostgreSQL50, record-filesystem87, public OAuth routes19, DCR32, global Monty1
and architecture boundary1. The public route suite executes actual Chromium.
Formatting, strict connected-feature and standard-feature Clippy (`-D warnings`),
fresh Auth/Product/Composition test-image compilation, actual worker compilation
and whitespace checks pass. The existing global service passes both Match and
No-Match without a Rust-loop fallback.

Both native stores exercise authenticated source rejection, changed generation,
exact repeated reads, independent-owner CAS competition, actually elapsed expiry,
cancel/readback semantics, legacy protection, removed bound accounts, first and
repeated callbacks, live and retired later-use markers, and an actual lost first
terminal-insert acknowledgement. Private encrypted fixture bytes are opaque:
these tests do not certify a Google payload codec or live issuer integration.

The actual Composition image SHA256 is
`d6a50d0dbb87021bae459a3486fb15aac4e605aa6c1b5c80a2e81fa29f7ea5a0`;
worker SHA256 is
`a4387d629652101c90de222af962218f4ca3e153408198ca3be9c21f20311143`.
Runner51803, Screen51802 and final runtime test54910 are verified absent;
the shared Cargo lock was independently acquired and released after completion.
The Prefix Source12 manifest remains unchanged. Its owner received the receipt,
image hashes and next serial source/build window. No commit, push, tag, deployment,
SSH action or target cleanup was performed.

## Remaining existing-owner integration

Connect the ordinary Google producers and `OAuthClientMaterialSource` consumers
to one atomic encrypted original containing the complete first client configuration
and PKCE verifier. Establish the authenticated flow-stable use marker before actual
provider exchange, preserve unresolved effect custody, and retain client-only
refresh lineage before retiring completed raw PKCE. Never substitute current client
configuration after restart, split the initial private writes, or infer that an
absent flow permits another exchange. Old flows without original private bytes need
an explicit compatibility/reconciliation decision. The public process-local verifier
cache and ordinary gate's existing verifier storage are not migrated by this packet.
Full K3–K7, Prefix serving, Validator, Composition, platform, upgrade and deployment
acceptance remain open.
