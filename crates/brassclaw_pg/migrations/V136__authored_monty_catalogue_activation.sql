-- A generation retains exact approved selections. Only the current pointer is
-- mutable; replacement never deletes artifacts held by an older task.
CREATE TABLE brassclaw_monty_active_catalogue (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    catalogue_id uuid NOT NULL REFERENCES brassclaw_monty_boot_catalogues(catalogue_id)
);
CREATE TABLE brassclaw_monty_catalogue_activations (
    activation_id uuid PRIMARY KEY,
    actor text NOT NULL CHECK (length(actor) BETWEEN 1 AND 2048),
    request_bytes text NOT NULL CHECK (octet_length(request_bytes) <= 67108864),
    request_checksum text NOT NULL CHECK (request_checksum = encode(sha256(convert_to(request_bytes, 'UTF8')), 'hex')),
    catalogue_id uuid NOT NULL REFERENCES brassclaw_monty_boot_catalogues(catalogue_id),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TRIGGER protect_monty_catalogue_activation BEFORE UPDATE OR DELETE OR TRUNCATE
    ON brassclaw_monty_catalogue_activations FOR EACH STATEMENT
    EXECUTE FUNCTION brassclaw_protect_monty_boot_catalogue();
