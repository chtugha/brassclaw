-- Transport correlation is evidence, never a component approval or Tool grant.
CREATE TABLE IF NOT EXISTS brassclaw_mcp_exchanges (
    exchange_id uuid PRIMARY KEY,
    parent_run_id uuid NOT NULL,
    owner_scope jsonb NOT NULL,
    token_hash text NOT NULL UNIQUE,
    catalogue_id uuid NOT NULL REFERENCES brassclaw_monty_boot_catalogues(catalogue_id),
    qualification_checksum text NOT NULL REFERENCES brassclaw_mcp_command_qualifications(checksum),
    advertised_tools jsonb NOT NULL,
    active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    disconnected_at timestamptz,
    UNIQUE(exchange_id,parent_run_id,owner_scope)
);
CREATE TABLE IF NOT EXISTS brassclaw_mcp_chat_calls (
    call_id uuid PRIMARY KEY,
    exchange_id uuid NOT NULL REFERENCES brassclaw_mcp_exchanges(exchange_id),
    parent_run_id uuid NOT NULL,
    owner_scope jsonb NOT NULL,
    request_id jsonb NOT NULL CHECK (jsonb_typeof(request_id) IN ('string', 'number')),
    command_checksum text NOT NULL CHECK (command_checksum ~ '^[0-9a-f]{64}$'),
    tool_name text NOT NULL,
    command text NOT NULL CHECK (length(command) > 0),
    phase smallint NOT NULL DEFAULT 0 CHECK (phase BETWEEN 0 AND 3),
    terminal_response jsonb,
    accepted_message_ref text,
    reply_message_ref text,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(parent_run_id, owner_scope, request_id),
    FOREIGN KEY(exchange_id,parent_run_id,owner_scope) REFERENCES brassclaw_mcp_exchanges(exchange_id,parent_run_id,owner_scope),
    CHECK ((phase < 2 AND terminal_response IS NULL) OR
           (phase >= 2 AND terminal_response IS NOT NULL AND jsonb_typeof(terminal_response) = 'object' AND accepted_message_ref IS NOT NULL))
);
CREATE OR REPLACE FUNCTION protect_mcp_chat_correlation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'MCP correlation evidence must be retained' USING ERRCODE='23514';
    END IF;
    IF NEW.call_id <> OLD.call_id OR NEW.exchange_id <> OLD.exchange_id OR
       NEW.request_id <> OLD.request_id OR NEW.parent_run_id <> OLD.parent_run_id OR NEW.owner_scope <> OLD.owner_scope OR NEW.tool_name <> OLD.tool_name OR
       NEW.command <> OLD.command OR NEW.command_checksum <> OLD.command_checksum OR NEW.created_at <> OLD.created_at OR
       NEW.phase < OLD.phase OR NEW.phase > OLD.phase + 1 OR
       (OLD.terminal_response IS NOT NULL AND (NEW.terminal_response IS DISTINCT FROM OLD.terminal_response OR NEW.accepted_message_ref IS DISTINCT FROM OLD.accepted_message_ref OR NEW.reply_message_ref IS DISTINCT FROM OLD.reply_message_ref)) THEN
        RAISE EXCEPTION 'MCP correlation identity/result cannot be replaced' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER protect_mcp_chat_correlation BEFORE UPDATE OR DELETE
    ON brassclaw_mcp_chat_calls FOR EACH ROW EXECUTE FUNCTION protect_mcp_chat_correlation();

CREATE OR REPLACE FUNCTION protect_mcp_exchange_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'MCP exchange evidence must be retained' USING ERRCODE='23514';
    END IF;
    IF NEW.exchange_id <> OLD.exchange_id OR NEW.parent_run_id <> OLD.parent_run_id OR
       NEW.owner_scope <> OLD.owner_scope OR NEW.token_hash <> OLD.token_hash OR
       NEW.catalogue_id <> OLD.catalogue_id OR NEW.qualification_checksum <> OLD.qualification_checksum OR
       NEW.advertised_tools <> OLD.advertised_tools OR NEW.created_at <> OLD.created_at OR
       (NOT OLD.active AND NEW.active) OR
       (OLD.disconnected_at IS NOT NULL AND NEW.disconnected_at IS DISTINCT FROM OLD.disconnected_at) THEN
        RAISE EXCEPTION 'MCP exchange identity cannot be replaced or reopened' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER protect_mcp_exchange_identity BEFORE UPDATE OR DELETE
    ON brassclaw_mcp_exchanges FOR EACH ROW EXECUTE FUNCTION protect_mcp_exchange_identity();
CREATE OR REPLACE FUNCTION forbid_mcp_correlation_truncate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'MCP correlation evidence must be retained' USING ERRCODE='23514';
END $$;
CREATE TRIGGER forbid_mcp_calls_truncate BEFORE TRUNCATE ON brassclaw_mcp_chat_calls
    FOR EACH STATEMENT EXECUTE FUNCTION forbid_mcp_correlation_truncate();
CREATE TRIGGER forbid_mcp_exchanges_truncate BEFORE TRUNCATE ON brassclaw_mcp_exchanges
    FOR EACH STATEMENT EXECUTE FUNCTION forbid_mcp_correlation_truncate();
