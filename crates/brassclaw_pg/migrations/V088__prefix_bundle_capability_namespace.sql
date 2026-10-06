-- The Python namespace and component names stay host.*. Runtime capability
-- identities must use the owning provider's builtin.* namespace. Repair only
-- the two known system mappings, preserving UUIDs, references and overrides.
UPDATE reborn_tools
SET capability_id = 'builtin.sweep_validated_components'
WHERE source = 'system'
  AND name = 'host.sweep_validated_components'
  AND capability_id = 'host.sweep_validated_components';

UPDATE reborn_tools
SET capability_id = 'builtin.store_prefix_bundle'
WHERE source = 'system'
  AND name = 'host.store_prefix_bundle'
  AND capability_id = 'host.store_prefix_bundle';
