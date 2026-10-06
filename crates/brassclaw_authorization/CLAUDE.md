# brassclaw_authorization guardrails

## Simplified v3 target

[simplified_v3.md](../../simplified_v3.md) §§1.1/9 supersede the scoped grant
and operation-approval requirements below for the v3 cutover. Instance tools
use one live, versioned allow/block policy plus technical rules. ToolSkill
bindings grant no permission. `LiveInstanceToolPolicy` publishes complete
generations with compare-and-publish; prepared admission reads the current
revision and returns it to the caller. Missing/failed policy reads fail closed.
No settings lock is held across tool execution. Durable settings publication
and production wiring remain composition responsibilities; the new source alone
does not certify removal of legacy approval paths. Existing grants/leases below
describe transitional implementations and migration evidence.

- Own grant matching, lease state, and dispatch/spawn authorization decisions.
- Do not execute capabilities, persist run-state, resolve approvals, reserve resources, prompt users, or import runtime/process/dispatcher/capability workflow crates.
- Authorization is default-deny and resource-owner/invocation scoped (tenant/user/agent/project/mission/thread plus invocation where applicable).
- Filesystem-backed leases must use async filesystem calls, not nested `block_on`.
- The filesystem lease store writes via bounded compare-and-swap (`CasExpectation::Version` with a retry budget) over versioned roots, giving cross-process safety; its per-owner keyed mutation locks add in-process serialization on top. Only byte-only/`Unsupported` roots (no version support) degrade to process-local serialization alone — those are not safe for real concurrent cross-process callers.
- Fingerprinted approval leases are resume-only authority and must not become ambient grants.
