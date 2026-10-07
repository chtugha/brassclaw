-- Preserve the actual proposal origin used by PgSempaiProposalSink. V052's
-- source constraint previously rejected every such PythonCode insertion.
-- This label is not approval: authored proposals remain pending and enter Q1.
-- Existing system-seed rules and existing rows are unchanged.
ALTER TABLE reborn_python_code
    DROP CONSTRAINT IF EXISTS reborn_python_code_source_check;
ALTER TABLE reborn_python_code
    ADD CONSTRAINT reborn_python_code_source_check
    CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system',
                     'sempai_proposal'));
