# brassclaw_resources guardrails

The [simplified v3 target](../../simplified_v3.md) supersedes scope-based resource
authority below while preserving technical quotas and execution identity.
`LiveMontyTaskSettings` publishes one coherent revision to Rust and Monty
consumers. Task compute/allocation consumption remains outside settings and must
survive updates and waits. Active compute excludes queue/idle/external waits;
external-call deadlines and shared heap are separate. `AdaptiveMontyHeapBudget`
calculates finite targets from measured additional capacity and reserve; it
does not probe the OS, reclaim live continuations or claim production wiring.
Measurement failure/pending reductions require visible state and backpressure.

- Own resource reservation, reconciliation, release, and quota accounting.
- No costed or quota-limited work should execute without an active reservation or explicit documented exception.
- Do not import runtimes, dispatcher, capabilities, approvals, processes, events, or product workflow crates.
- Preserve tenant/user/project scope in every reservation and receipt.
- Keep accounting deterministic and safe under concurrent reservations.
