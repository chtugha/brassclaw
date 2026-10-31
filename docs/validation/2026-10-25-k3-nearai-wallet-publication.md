# Original NEAR wallet publication — 2026-10-25

**Scoped acceptance passed:113 distinct Rust cases (five final executions and108
qualified unchanged results),19 unchanged JavaScript cases and five unchanged
Python cases. Full K3 remains open.**

The existing provider authentication owner now issues a server-side wallet
challenge before external signing. The32-byte NEP-413 nonce binds the original
mutation ID, provider revision, primary selection and authenticated encrypted-key
reference. Message, recipient and optional validated callback must match exactly.
Matching state is consumed atomically before one exchange. Repetition can read
only its exact committed original proof/receipt; expired, consumed or lost state
cannot initiate another exchange. A browser callback cannot consume wallet state.
This is control-plane authentication infrastructure, not a Tool grant or workflow.

No database connection or settings publication lock spans the external exchange.
A concurrent edit remains possible and prevents stale final publication. Selected
provider HTTP timeout and optional decoded-response capacity apply; redirects and
transport retries are disabled. Credentials, primary selection and original
receipt commit through the existing common provider/secret transaction. Stored
keys no longer shadow the new credential. Original readback reconciles current
desired state without restoring old credentials. Legacy file/live-session state
is untouched by this route. Browser proof JSON stays compatible: absent wallet
metadata is omitted, including when reading an old committed browser receipt.

The SDK now hides saved token/signature Debug and private response/parse data.
A failed legacy HTTP-client constructor remains an error instead of silently
creating a client without its intended timeout. The separate Codex credential
client also explicitly disables transport retry. These changes do not complete
legacy session-file removal, typed provider auth setup or rotating OAuth custody.

Start/complete DTOs, service/facade, HTTP routes/descriptors and low-level API
helpers carry the server identity. Completion without it fails validation. The
existing apiFetch sends once; no signature resubmission helper is introduced.
**A wallet signer UI/connector is still absent and is not claimed here.**

## Actual evidence and applicability

Final archive: `target/accepted-evidence/k3-nearai-wallet-20261025/run-5/`.
Receipt SHA-256: `780ce0293cce4d3513d3845831a2b4e378b49085d0ed25f577cb9dba9283a4ba`.
Current source, exact18 Root files plus five peer sources, commands, binaries,
worker and result logs are retained there. `qualified-reuse.json` identifies the
actual Run3 logs and excludes the one subsequently clarified HTTP test from its
59-case handler result.108 unchanged Root cases,19 JS and five Python cases retain
their exact scope; the changed HTTP case is freshly executed in Run5.

Four actual PostgreSQL/HTTP wallet cases in Run3 cover lost COMMIT acknowledgement,
empty-RAM/rebuilt-owner original readback, existing encrypted key replacement,
changed-request proof, concurrent duplicate, editable settings during a held call,
stale role/definition/key, selected deadline, unknown outcome, expired/lost state,
cross-protocol state, private malformed/rejected/oversized replies and redirects.
The real SDK model peer sees the committed Bearer token. Session-file bytes stay
unchanged; public mutation metadata contains no signature/token. Controlled issuer
replies do not qualify a real account, signature verifier or remote service.

Run3 additionally executes SDK26 (including four new transport/privacy cases),
existing browser3, original provider receipt/global live-model/secret ownership,
legacy/CAS/lease boundaries, callback and settings contracts, descriptor2 and
handler59. The final lexical guard-scope change in the new HTTP case receives
fresh execution. Root and worker use the existing global architecture.

The peer's archived-source projection is independently scoped: five Python cases
and four actual Prefix integration cases, including preserved whole originals.
The final four run freshly in2.35s. Its exact Run4 executable was reused after
complete compiler-input comparison; the missing worker environment was repaired,
not inferred as a source defect. All530 original worker inputs remain unchanged;
worker SHA is `d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.
No Sources UI/backend, availability, activation or serving claim follows.

Current strict Skills lib5.78s, Prefix integration57.82s, Root/Composition/LLM/
Secrets/Product/WebUI tests102.69s and default Composition34.75s pass with
`-D warnings`. Workspace format, whitespace and exact source integrity pass.
Cargo remains serial in screen, inherited canonical target, Rust1.96.0. Required
capacity cleanup preserves actual executable evidence; no source mirror or second
build target is used.

Retained failures: Run1 peer json! syntax; Run2 builds but two SDK fixtures used
file-only save_session without live initialization (corrected to the actual
public hook); Run3 catches producer-result to input explicit-check requirements
in the peer contract; Run4 builds but the shortened runner omits the worker.
Run5 repairs only that runner environment and uses the identical built artifact.
No ignored/zero-selected checks or suppressions count as acceptance.

## Remaining boundaries

At this receipt's source snapshot, a connected review found that fresh
same-provider login cleared an explicit model to the definition default. That
finding is now repaired in the separately scoped
[model-preservation successor](2026-10-25-k3-login-model-preservation.md), with18
fresh cases and current strict checks. This historical receipt itself continues
to cover challenge/credential custody, not the successor's added model behavior. Wallet UI, Codex controller/encrypted
rotating session, DB-owned typed auth setup and other protocol custody remain.
The existing4KiB ingress default and legacy SDK30s client default remain subject
to K7 settings migration. NEP-41332-byte nonce/five-minute issuer validity are wire
constraints, distinct from task/VM resource budgets. No cold-process restart,
full K3/plan, cross-platform deployment or test-machine/release acceptance.
