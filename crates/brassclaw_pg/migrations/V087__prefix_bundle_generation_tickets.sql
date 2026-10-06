-- Add request identity to stored prefix generations and short-lived tickets
-- that keep target scope out of model-authored capability arguments.

ALTER TABLE reborn_basic_prompt_store
    ADD COLUMN generation_id uuid NOT NULL DEFAULT gen_random_uuid();

CREATE TABLE reborn_prefix_scope_tickets (
    ticket_hash     text PRIMARY KEY,
    tenant_id       text NOT NULL,
    user_id         text NOT NULL,
    agent_id        text NOT NULL,
    project_id      text NOT NULL,
    conversation_id text NOT NULL,
    request_id      uuid NOT NULL,
    expires_at      timestamptz NOT NULL,
    consumed_at     timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now(),
    CHECK (length(ticket_hash) = 64),
    CHECK (length(conversation_id) BETWEEN 1 AND 256)
);

CREATE INDEX reborn_prefix_scope_tickets_expiry_idx
    ON reborn_prefix_scope_tickets (expires_at)
    WHERE consumed_at IS NULL;

CREATE TABLE reborn_prefix_scope_leases (
    tenant_id   text NOT NULL,
    user_id     text NOT NULL,
    agent_id    text NOT NULL,
    project_id  text NOT NULL,
    lease_token uuid NOT NULL,
    expires_at  timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, user_id, agent_id, project_id)
);
