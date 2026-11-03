# Bob 3 Auth follow-ups to 77b31f122

The leading chat requested these changes after reviewing the isolated Auth
commit. No local Cargo or additional paid Modal build is authorized/used for
these follow-ups; the leading chat owns combined native compilation/execution.
These are source changes with pending native acceptance.

## Native identity/index conflict checks

Migration's conflict readback now compares the actual indexed `user_id` as well
as JSON for accounts, flows, interactions and requirement selections. Refresh
readback compares actual `account_id`, nullable `refresh_handle`,
`blocks_refresh` and JSON. Primary-key tenant/id/scope selectors remain exact.
`FOR UPDATE` retains the compared native row through the cutover transaction;
identical JSON cannot hide a conflicting index or overwrite it.

Written actual-PG test (uncompiled/unexecuted here):
`native_auth_migration_rejects_same_json_with_conflicting_native_indexes`.
It independently corrupts all four user indexes and all three refresh indexes
while retaining identical JSON, calls the actual import helpers and requires
conflict with no overwrite. This is SQL identity/index fidelity, not provider
or issuer acceptance. Existing full-import tests remain applicable integration
selectors after recompilation.
