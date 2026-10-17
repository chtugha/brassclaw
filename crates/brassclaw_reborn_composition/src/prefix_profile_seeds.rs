//! Installation-owned profile definitions. Register before qualification, not
//! when a WebUI service is constructed. Metadata never enables compilation or
//! activation, and an upgrade never overwrites an operator's existing record.
use brassclaw_pg::PgPool;

const PROFILES: [(&str, &str, &str, &str, i32); 6] = [
    (
        "base-prompt",
        "BrassClaw Base Prompt",
        "BrassClaw architecture, current documentation and approved components.",
        "base-promt-compiler.py",
        0,
    ),
    (
        "defensive",
        "Defensive / Servers and Networks",
        "Linux, Debian, Ubuntu, firewalls, routers, defensive security and incident response.",
        "Defensive-compiler.py",
        1,
    ),
    (
        "homeassistant",
        "Home Assistant / MQTT / Modbus / YAML",
        "Home Assistant configuration, automations, MQTT and Modbus with YAML authoring.",
        "Homeassistant-compiler.py",
        2,
    ),
    (
        "tomedo",
        "tomedo / API / macOS / PostgreSQL",
        "tomedo integrations, REST API, macOS, PostgreSQL, GOÄ, EBM and coding; excludes clinical knowledge.",
        "tomedo-compiler.py",
        3,
    ),
    (
        "sempai",
        "Sempai / Prompt Review and Component Authoring",
        "Prompt diagnosis and reusable BrassClaw component authoring with validation contracts.",
        "sempai-compiler.py",
        4,
    ),
    (
        "prefix-maker",
        "Prefix-Maker / Prefix Design",
        "Evidence, formatting, model-specific token layout and cache-aware prefix design.",
        "prefix-maker-compiler.py",
        5,
    ),
];

pub(crate) async fn retain(pool: &PgPool) -> Result<(), crate::RebornBuildError> {
    let mut client = pool.get().await.map_err(|_| invalid())?;
    let tx = client.transaction().await.map_err(|_| invalid())?;
    for (name, label, description, entry, order) in PROFILES {
        tx.execute(
            "INSERT INTO reborn_prefix_profiles(name,label,description,compiler_entry,sort_order)
            VALUES($1,$2,$3,$4,$5) ON CONFLICT(name) DO NOTHING",
            &[&name, &label, &description, &entry, &order],
        )
        .await
        .map_err(|_| invalid())?;
        let row = tx
            .query_one(
                "SELECT revision FROM reborn_prefix_profiles WHERE name=$1",
                &[&name],
            )
            .await
            .map_err(|_| invalid())?;
        if row.get::<_, i64>(0) <= 0 {
            return Err(invalid());
        }
    }
    tx.commit().await.map_err(|_| invalid())
}
fn invalid() -> crate::RebornBuildError {
    crate::RebornBuildError::InvalidConfig {
        reason: "installation prefix profile registration failed; verify schema migrations and retained records".into(),
    }
}
