---
paths:
  - "crates/brassclaw_agent_loop/**"
  - "crates/brassclaw_turns/**"
  - "crates/brassclaw_host_runtime/**"
  - "crates/brassclaw_engine/**"
---
# Tool Evidence and Side-Effect Verification

The most dangerous user-visible bug class is **claim/evidence drift**: the agent narrates "message sent" / "file attached" / "tool installed" with no corresponding side effect. This rule documents the target code invariants that make the rule enforceable at the tool layer.

## Agent Loop Side-Effect Gate

The agent loop (`crates/brassclaw_agent_loop/`) should classify user turns for side-effect intent and a model-final turn that lacks at least one successful tool call matching the intent should surface "action not performed" rather than the agent's narration.

The prompt-side guidance in `crates/brassclaw_engine/src/executor/prompts/` is the primary defence until a hard rejection gate lands. Reference: engine roadmap.

## Empty-Fast Outputs Are Errors (tool-author convention)

A tool that completes in `< 1 ms` **and** returns empty content is almost always a silent failure. Tool authors must treat this shape as an error at the tool implementation: return a descriptive error rather than a successful empty response.

## External-Effect Tools Must Read Back

A tool whose side effect is visible only to an external system (Telegram send, Slack post, file write, extension install, OAuth completion) MUST read back the effect before returning success:

- Send operations → capture and return the message ID from the API response; error if the response lacks one.
- `file_write` → re-stat and return the actual byte count; error on mismatch.
- `extension_install` → confirm the extension is now present and active in the live registry.
- OAuth completion → perform a minimal authenticated read against the provider before declaring success.

A tool without a read-back path is claim-only. Include an `unverified: true` key in the JSON result body and a clear hedge in the text output ("submitted; delivery not confirmed") so downstream layers and the user can see it.

## Setup UI Round-Trip

Save / Install / Connect buttons in the setup UI must issue a read-back verification immediately after the write succeeds and render the read-back value (or explicit error) to the user — not a local optimistic checkmark. A UI success state with no corresponding backend read-back is the same bug class as agent claim drift.
