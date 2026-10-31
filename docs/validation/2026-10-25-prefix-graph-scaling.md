# Wave-stable Prefix graph scaling — 2026-10-25

**Scoped source acceptance passed.** The existing pure dependency checker now
uses adjacency lists, remaining dependency counts and sorted ready waves instead
of repeated pending-list scans/removals and resolved-list membership searches.
Closure expansion keeps its ordered list while using a set for membership.
Traversal is linear in nodes/edges apart from canonical wave sorting. No source,
mandatory group, budget band or optional-cycle validation was removed.

Sorted waves deliberately preserve the earlier order. An ordinary heap-based
topological traversal could insert a newly ready child before an independent
member of the current wave and change the rendered/token prefix. The new
implementation waits for the complete current wave before sorting the next.

## Actual checks

Evidence: `target/accepted-evidence/prefix-graph-scaling-20261025/run-1/`.
Accepted receipt SHA-256:
`aac8504fe4014abe7522b98c9cba8757fc4f900ae95c32d97b86ee80926ec144`.

- **15 Python regressions pass.** The new ordering test covers all 4,096 directed
  edge subsets between four groups without self-edges, comparing valid orders
  with an independent reference and rejecting cycles. The 1,001-group reverse
  chain verifies order/closure without recursion or a group cutoff. A cycle in
  optional groups is rejected even with an independent mandatory root ready.
- **Three actual native integration cases pass**, zero ignored/filtered. The
  existing Monty/real-tokenizer case now also executes the 1,001-group traversal
  and closure in the archived trusted utility worker. Both retained Recipe
  variants and exact native bindings/preload remain covered by assembly/inspection.
- Strict Composition integration lint, separate Skills library lint and default
  Composition lint pass with `-D warnings`; current formatting, whitespace and
  source integrity pass. This is not a newly executed full Root/HTTP/model suite.
  Public Rust interfaces, the source-only compiler procedures and serving owners
  were unchanged, so those broader claims were not retested or expanded.
- Differential checks against the exact archived older implementation give
  **identical complete candidate banks, reference texts and tokenizer probes**
  for the unit-design fixture, namespace-rule fixture and the 1,001-group chain.

The single local CPython measurement for graph validation plus closure on the
same 1,001-group input is **1.05465s before / 0.00267s after**, with identical
order and closure. This is a narrow helper benchmark, not a whole-compiler,
GPU-prefill, model-accuracy or cache-throughput measurement. Raw timing/source
identities and candidate byte-equivalence records are sealed as supplemental
development evidence in the receipt. No timing threshold is used as a test.

The actual worker bytes and dependency source applicability were verified from
the previous accepted package; no newly built worker/application pair is claimed.
Current changed source copies, test executable, original serial runner and all
raw logs are retained. Needed existing executables were preserved before any
capacity cleanup. The canonical target and shared lock were used.

## Remaining boundaries

This improves the existing reference-selection implementation and leaves its
input/output formats and qualification boundaries intact. Complete candidate
rendering/tokenization still measures every candidate, including nonmonotonic
token counts; no additive estimate or silent truncation was introduced. Native
transport/value limits and the need for qualified batching for oversized candidate
banks remain. This is not acceptance of a model-specific design, proof of a
complete authoritative source inventory, ordinary Generate admission, quality
qualification or serving activation. The Prefix plan still owns those gates.
