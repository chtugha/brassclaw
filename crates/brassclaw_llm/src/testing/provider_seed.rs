//! Unit-test catalogue fixture derived from the authoritative installation
//! migration. This is never a runtime catalogue or a startup seeder.

/// Read the exact definition payload used by V140, rather than retaining a
/// second provider list in a JSON fixture. Native PostgreSQL tests separately
/// execute the migration and verify initialization/provenance semantics.
pub fn installation_provider_definitions() -> Vec<crate::ProviderDefinition> {
    let migration =
        include_str!("../../../brassclaw_pg/migrations/V140__provider_database_initialization.sql");
    let (_, payload) = migration
        .split_once("FROM jsonb_array_elements(''")
        .expect("V140 installation payload must be present");
    let (payload, _) = payload
        .split_once("''::jsonb) entry;")
        .expect("V140 installation payload must terminate");
    // The JSON is inside two SQL string literals. A JSON apostrophe is
    // escaped twice in the migration, hence four SQL apostrophes here.
    serde_json::from_str(&payload.replace("''''", "'"))
        .expect("V140 installation provider payload must be valid")
}
