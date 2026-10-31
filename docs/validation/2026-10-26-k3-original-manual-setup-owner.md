# Original manual setup owner and explicit continuation — 2026-10-26

Scoped backend/facade acceptance: [terminal receipt](../../target/accepted-evidence/k3-original-manual-setup-owner-20261026/run-3/accepted.json), SHA-256 `8a6457d514805e62f08bae816d0f57fc5fb55058fc9d148f41b44b7fa137a260`.
The exact20-file change manifest, full source inventory, validated sources, actual command logs and eight preserved Cargo images accompany it. Independent final inspection verified all20 author hashes,9,744 source entries (9,169 applicable runtime/package entries), exact applicable membership and all eight images. No source/build mirror or second Cargo target was used.

## Implemented boundary

`ManualTokenSetupIdentity` holds independent caller-retained interaction/flow IDs. The existing high-level manual-flow port and product-auth facade now expose original setup, pair-fenced raw submission and explicit token-free confirmed-reference continuation. Arbitrary unrelated lower ports return unavailable for original custody rather than invent a replacement operation.

The native owner creates the interaction/flow metadata pair atomically using the existing PostgreSQL tables, consistent one-statement original readback and flow/interaction/account lock order. The filesystem owner retains its original interaction first and reconciles the exact CAS-protected flow projection; this is not a multi-record transaction. Both preserve first accepted expiry, full original scope/definition and prechallenge account snapshot. Changed definition/scope/pair fails; pending or terminal state is returned honestly without TTL extension, current-account substitution, cancellation resurrection or unknown-write cleanup. Pending/callback lifecycle states cannot impersonate a valid awaiting manual setup.

Strong raw submission checks the whole selected pair before credential publication and still verifies the supplied raw input on every replay. Reference continuation accepts no raw input, requires actual original publication and the matching completed flow, and resumes the existing continuation without changing today's account. A retained intent alone is insufficient. The native reader requires a whole-flow original; no lower account-only result is silently upgraded. The owning fake models its complete publication atomically under the same state lock while retaining lower-operation semantics.

## Actual validation

281 distinct Rust cases were freshly executed, zero failed or ignored: Auth86, real filesystem Memory/PostgreSQL16, native manual14, generic coordinator3, other durable53, ordinary global Monty1, public facade/routes107 and the applicable existing architecture program1. The seven new cases are two native, four Memory/PostgreSQL-backed filesystem and one public facade case. They cover actual lost setup COMMIT acknowledgement, independent repeated reads/first expiry, changed definition/scope, raw pair mismatch before effects, confirmed reference after continuation failure/account revocation, terminal setup replay and cancellation while the actual first flow projection is paused. Existing actual publication write-boundary and ordinary global Match/No-Match cases also passed.

Strict affected root/Composition/Auth/Secret tests lint81.63s, PostgreSQL without RootLLM16.37s, default Composition18.69s, format4.63s and whitespace passed. Composition library test build199.78s, public programs4.91s, Auth contract program1.93s. The worker Cargo build0.46s was cached; this is not a fresh-worker-compilation claim. All commands were serial in Screen with inherited canonical target and the shared Cargo lock. There was no cleanup.

R1 is preserved as a failed exhaustive-match diagnostic for `Pending`; R2 is preserved as the introduced collapsible-if lint. The causes and an inaccessible sibling helper in a new test were repaired before R3; no warning suppression or skipped acceptance was introduced. R3 is the complete terminal successful batch.

## Remaining work

Existing public HTTP, browser and extension consumers still use their legacy original-identity contracts. Their migration is not established by this backend receipt. The private HTTP draft requires IDs/invocation and has a separate reference route; the private browser draft stores only redacted original correlation, splits raw/reference actions and preserves exact raw text. They remain unapplied and need actual public HTTP/controller/reload/cancellation/response-correlation acceptance together.

The connected extension path currently chooses fresh setup IDs/expiry, reselects today's credential projection during retry and trims supplied raw text. Migrate its existing product port, payload, adapter and browser together with retained per-requirement identity and prechallenge binding. Do not declare whole manual setup or K3 complete beforehand. DCR unknown-registration cleanup, final credential lifetime/generation fences, acknowledged timeout/cap settings, remaining K3–K7 and full plans remain open. Prefix production design/formatter/serving remains owned by the other chat. No Git/tag/push/release/SSH was performed by this package.
