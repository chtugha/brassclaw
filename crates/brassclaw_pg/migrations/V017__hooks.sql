-- V017__hooks.sql
-- DDL verbatim from crates/brassclaw_hooks_postgres/migrations/V1__predicate_state.sql.
-- IF NOT EXISTS guards added for refinery idempotency (see §3 history reconciliation).
--
-- scope_hash: BYTEA (raw blake3 digest) — NOT TEXT (encoding overhead, space waste).
-- key_hash: same BYTEA rationale.
-- event_id: TEXT, NOT UUID — event IDs are blake3 64-char hex digests which exceed
--   UUID capacity; changing to UUID would silently truncate and cause phantom dedup.
-- hooks_* tables intentionally keep no brassclaw_ prefix for backward compatibility
-- with existing deployments.

CREATE TABLE IF NOT EXISTS hooks_predicate_invocations (
    scope_hash   BYTEA       NOT NULL,
    key_hash     BYTEA       NOT NULL,
    event_id     TEXT        NOT NULL,
    occurred_at  TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (key_hash, event_id)
);
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_key_ts_idx
    ON hooks_predicate_invocations (key_hash, occurred_at);
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_scope_idx
    ON hooks_predicate_invocations (scope_hash);
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_scope_key_idx
    ON hooks_predicate_invocations (scope_hash, key_hash);
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_ts_idx
    ON hooks_predicate_invocations (occurred_at);

CREATE TABLE IF NOT EXISTS hooks_predicate_values (
    scope_hash   BYTEA       NOT NULL,
    key_hash     BYTEA       NOT NULL,
    event_id     TEXT        NOT NULL,
    occurred_at  TIMESTAMPTZ NOT NULL,
    value        NUMERIC     NOT NULL,
    PRIMARY KEY (key_hash, event_id)
);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_key_ts_idx
    ON hooks_predicate_values (key_hash, occurred_at);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_scope_idx
    ON hooks_predicate_values (scope_hash);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_scope_key_idx
    ON hooks_predicate_values (scope_hash, key_hash);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_ts_idx
    ON hooks_predicate_values (occurred_at);
