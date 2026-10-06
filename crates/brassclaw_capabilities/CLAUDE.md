# brassclaw_capabilities guardrails

- Own caller-facing `CapabilityHost` invoke/resume/spawn workflow.
- Use the neutral `CapabilityDispatcher` port; do not add a normal dependency on concrete `brassclaw_dispatcher` or runtime crates.
- `CapabilityHost` is the single caller-facing authority path for invoke/resume/spawn: host-runtime adapters, built-ins, custom packages, and external runtimes must enter through this workflow rather than adding parallel authorization/approval dispatch paths.
- Host authorization must use the trust-aware contract (`TrustAwareCapabilityDispatchAuthorizer`) with a policy-derived `TrustDecision`; do not wire production `CapabilityHost` with grant-only authorization that bypasses trust ceilings.
- Do not absorb process lifecycle/result APIs; those belong in `brassclaw_processes::ProcessHost`.
- Legacy approval resume must validate and claim the matching fingerprinted lease before dispatch. The [simplified v3 target](../../simplified_v3.md) removes operation approvals: instance policy rejects those resume paths before lease claiming.
- Authorization denial or unsupported/failed obligations must fail before runtime dispatch, process start, or approval lease claim.
- Keep obligation handling behind a seam; built-in obligation implementations belong in later host-runtime/obligation slices.

Instance dispatch/spawn rechecks policy after awaited obligation preparation.
Admission returns the applied settings revision; changed technical obligations
require new preparation, and denial aborts prepared effects before dispatch.
Settings publication and admission snapshot reads share one synchronization
boundary. Later revocation affects subsequent admissions, including those in
already running Recipes; it does not replay a call already admitted. Worker
attempt freshness remains a separate execution-ownership requirement.
