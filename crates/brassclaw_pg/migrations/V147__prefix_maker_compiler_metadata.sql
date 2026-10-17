-- A registered filename is metadata, never execution authority. Preserve any
-- operator-supplied adapter, and retain revisions when the shipped gap is filled.
UPDATE reborn_prefix_profiles
SET compiler_entry='prefix-maker-compiler.py', revision=revision+1
WHERE name='prefix-maker' AND compiler_entry IS NULL AND revision=1;
