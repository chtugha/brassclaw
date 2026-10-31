# Review accounting preserves native submission evidence — 31 October 2026

[The receipt](2026-10-31-review-unsubmitted.json) binds two reviewed sources,
the actual archived images and the sole terminal Screen run. Compilation,
format, five distinct tests/zero ignored and connected/default strict Clippy
pass. BUILD-011's actual worker is reused with unchanged dependency inputs.

The existing Sempai accounting path now retains the gateway's actual
`confirmed_unsubmitted` evidence before converting its error. Only that evidence
can invoke the existing accountant's supported unsubmitted reconciliation.
Unsupported reconciliation and unknown outcomes keep their conservative failure
path and holds. There is no new ledger, permission, retry or error-category rule.

The actual `GovernorBackedAccountant` and `InMemoryResourceGovernor`, with native
PostgreSQL task/model records and the existing SDK loopback peer, exercise two
cases. A real preparation guard returns BudgetExceeded before transport; both
distinct primary/review reservations are released, no request is sent and no
spend is manufactured. A real SDK response with finish reason Length also causes
BudgetExceeded; only the unsubmitted primary reservation is released, while the
review retains its unresolved token reservation and original provider evidence.
The test checks the actual reservation/release identities, native dispatch
counts, peer requests, original finish reason and balances. Error names alone
therefore cannot turn submitted work into zero spend.

Existing ordinary related custody, explicit original output settings with
discretionary budgets disabled, and interceptor protocol cases also pass. The
two accounting modes explicitly enable their preparation budget. This is caller
accounting acceptance with the real in-memory governor, not durable-governor
crash recovery, remote inference quality or provider billing verification.

The current native review remains pre-primary. Actual sealed Free/Serving
qualification, full I1/I2, post-primary/future Recipe semantics, compatibility/
deprecated bridges, K3–K7/validator/library/recovery/platform/release and complete
plans remain open. No version, tag, commit, push or deployment is claimed.
