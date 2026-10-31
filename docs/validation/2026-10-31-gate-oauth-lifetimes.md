# BUILD027 — acknowledged task-gate OAuth lifetime

The [machine receipt](2026-10-31-gate-oauth-lifetimes.json) retains exact source,
worker, test images, original attempts and log identities. SHA-256: `9b805da5a6200aa3972c7d9ead7c50bdc900ba4e9d20990994b048132b4a8c13`.

New Google and DCR challenges for waiting task gates use the same acknowledged
`turn_gate_oauth_setup_ttl_secs` setting (default600, positive u32). The first
stored original determines expiry across concurrent preparation, retries and
live edits. Expired originals remain expired. Configured unavailable policy
prevents new preparation; standalone explicit-expiry contracts stay unchanged.
V196 fills missing settings once, preserving explicit values and unrelated defaults.
EN/DE controls include help; real browser changes are acknowledged by Rust/Monty.

91 distinct native cases pass (19 fresh plus36 settings and36 unchanged DCR
cases reused),32 architecture cases, zero ignored, connected/default strict Clippy.
Coverage includes actual PostgreSQL/filesystem/HTTP, global retained deployment,
base/Sempai and native WebUI. The new regression corrects its original erroneous
expectation of an error for valid Expired metadata. A packaging-only retry places
the exact existing fresh worker beside the unchanged test image, as required by
the production resolver; no Cargo rebuild, fallback or fabricated success.

These local macOS results do not close full K3, standalone OAuth default/max/client
contracts, remaining ingress/polling controls, general approval, PrefixFree,
CI/platform/deployment or the complete implementation plans. Original failed
attempts remain historical failures with their actual diagnostics.
