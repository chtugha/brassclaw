# BUILD024 — live manual-token setup lifetimes

The [machine receipt](2026-10-31-manual-setup-lifetimes.json) records actual
sources, images, original logs and commands. SHA-256: `1b983910ef51893b1377903927210d7f1e07bb4359852d2b3f700f079ad852e5`.

`manual_token_setup_ttl_secs` (default600s) and
`extension_manual_token_setup_ttl_secs` (default300s) now follow the existing
acknowledged Rust/Monty settings owner and Auth facade. The generic HTTP routes
and Extension adapter use these values for new original setups. Existing native
original owners retain the first accepted expiry across live edits, concurrent
creation, retries and acknowledgement loss. Extension recovery precedes new
policy capture. Direct native explicit-expiry callers keep their existing contract.
V195 preserves partial/full operator values, complete unrelated INSERT defaults
and old Auth records; positive-u32 validation rejects malformed values. EN/DE
controls explain the scope and original-expiry behavior.

**46 native cases and32architecture cases passed, zero ignored.** Actual PG
route/facade/Extension-adapter cases cover live30/45→1/u32MAX changes, the effective
revision, concurrent first creation, repeated original retention, changed-input
rejection, expired-original nonrenewal and unavailable configured policy. Native
PostgreSQL COMMIT-loss and filesystem custody cases remain covered. The real
manual and resource browser consumers passed, including persisted1777/1555s,
effective settings, EN/DE help and exact complete restoration. Connected and
default strict Clippy passed with `-D warnings`; compilation used the actual
common checkout/target and a fresh worker.

These cases establish scoped local macOS behavior, not real issuer availability,
new GitHub/Linux/Windows acceptance or component approval. OAuth maximum/default,
client expiry and popup/transport/ingress work, remaining K3–K7 and the full plans
remain open. No full-plan, release, Q1/Q2/Pure/S or PrefixFree grant is asserted.
