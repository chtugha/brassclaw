use crate::common::pg_rig;

#[tokio::test]
async fn monty_new_defaults_preserve_stored_operator_values() {
    let rig = pg_rig().await;
    let client = rig.pool.get().await.unwrap();
    client
        .execute(
            "INSERT INTO reborn_monty_vm_settings
         (tenant_id, user_id, agent_id, project_id, max_duration_secs, token_budgets_enabled)
         VALUES ('legacy', 'owner', 'agent', 'project', 300, true)",
            &[],
        )
        .await
        .unwrap();
    let migration =
        include_str!("../../../brassclaw_pg/migrations/V090__monty_new_settings_defaults.sql");
    client.batch_execute(migration).await.unwrap();
    client.batch_execute(migration).await.unwrap();
    let row = client
        .query_one(
            "INSERT INTO reborn_monty_vm_settings (tenant_id, user_id, agent_id, project_id)
         VALUES ('new', 'owner', 'agent', 'project')
         RETURNING max_duration_secs, token_budgets_enabled",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, i32>(0), 600);
    assert!(!row.get::<_, bool>(1));
    let old = client.query_one(
        "SELECT max_duration_secs, token_budgets_enabled FROM reborn_monty_vm_settings
         WHERE tenant_id = 'legacy' AND user_id = 'owner' AND agent_id = 'agent' AND project_id = 'project'", &[]
    ).await.unwrap();
    assert_eq!(old.get::<_, i32>(0), 300);
    assert!(old.get::<_, bool>(1));
    let rust_defaults = brassclaw_product_workflow::default_monty_vm_settings();
    assert_eq!(rust_defaults.max_duration_secs, 600);
    assert!(!rust_defaults.token_budgets_enabled);
}
