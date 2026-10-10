# brassclaw_resources guardrails

The [simplified v3 target](../../simplified_v3.md) supersedes scope-based resource
authority below while preserving technical quotas and execution identity.
`LiveMontyTaskSettings` publishes one coherent revision to Rust and Monty
consumers. `SharedMontyTaskBudget` gives both readers the same compute account
and terminal failure state. Task compute consumption remains outside settings
and survives updates and waits. The neutral task contract does not require an
allocation count: Monty 1.0 removed that limit. Existing legacy DB/operator
values remain intact until the explicit upgrade migration; do not represent
them as an enforced Monty 1.0 resource. Active compute is executing VM time.
Preparation/compilation and Rust value adaptation are separate observations;
they do not debit that duration. Queue/idle/external waits enter none of these clocks;
external-call deadlines and shared heap are separate. `AdaptiveMontyHeapBudget`
calculates finite targets from measured additional capacity and reserve; it
does not probe the OS, reclaim live continuations or claim production wiring.
The default sizes once at startup and holds a configurable finite budget.
Automatic measurement is optional. Elevated pressure alone does not pause
admission; missing samples prevent growth while existing finite-budget work
continues with sufficient configured headroom. Critical pressure or proved
insufficient capacity can apply backpressure. Pending reductions stay visible.

- Own resource reservation, reconciliation, release, and quota accounting.
- No costed or quota-limited work should execute without an active reservation or explicit documented exception.
- Do not import runtimes, dispatcher, capabilities, approvals, processes, events, or product workflow crates.
- Preserve tenant/user/project scope in every reservation and receipt.
- Keep accounting deterministic and safe under concurrent reservations.

Before production wiring, implement the Monty 1.0 upgrade gate in Phase 3a.
The calculator does not establish allocator-backed Monty heap isolation.
The shared task account does not establish interpreter preemption, measurement
or WebUI runtime acknowledgement. Record non-overlapping active segments once
through the hosting owner; checking copied counters in two consumers is invalid.
`MontyTaskClock` debits differences in an owned interpreter's cumulative clock
exactly once per cursor. Preserve the cursor across resumes and limit changes;
each nested interpreter has a separate cursor attached to the same task account.
A regression is terminal, never a reset. A baseline excludes earlier execution.
Hosting must still prove final/error clock recovery and task attribution; do not
feed a global multi-task execution clock into an individual task account.
