-- V069__reborn_python_code_includes.sql
-- Step C.4.5.2 — Common component syntax: PythonCode (class 22) includes.
--
-- A PythonCode body may carry `{{component_name}}` structural-include
-- placeholders (F-HI-2=A) the composer (C.4.5.17) inlines at compose time —
-- the referenced mini-PythonCode component's body (one function each, like an
-- include). The machine-readable form of that relationship is this `includes`
-- column: a JSONB array of component UUIDs (`Vec<Uuid>` in Rust) the composer
-- fetches + name-matches to the placeholder tokens.
--
-- This mirrors the recipe's machine form (`StepEntry.include: Vec<Uuid>` in
-- the IBS — Phase A / C.4.5.1) and reuses the class-23 `GenericComponent::extra`
-- precedent only transiently on the Q1 save path; the canonical store is this
-- column. Variables (`{{vars.NAME}}` / `{{user_input}}`) flow from the recipe /
-- caller — PythonCode carries NO `variable_patterns` field, only this include
-- list. Referential placeholder<->include matching is deferred to Phase I/N
-- (requires a pool); Q1 validates structure only (well-formedness + non-nil
-- UUIDs — Fork 2-B=B).
--
-- Default '[]' so every pre-existing row stays valid (no includes).

ALTER TABLE reborn_python_code
    ADD COLUMN IF NOT EXISTS includes JSONB NOT NULL DEFAULT '[]'::jsonb;
