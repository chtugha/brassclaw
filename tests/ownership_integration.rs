//! Integration tests for the ownership model.
//!
//! Tests get_or_create_user, migrate_default_owner, tenant isolation, and ChannelPairingStore.
//! Uses libSQL file-backed tempdir — no PostgreSQL required.
//!
//! Note: `new_memory()` does NOT share schema across separate `connect()` calls
//! in libsql (each call gets its own connection and a new in-memory DB). All
//! tests here use `new_local` with a `tempfile::TempDir` so all connections
//! within the same test share the migrated schema.

#[cfg(feature = "libsql")]
mod tests {
    use brassclaw::db::libsql::LibSqlBackend;
    use brassclaw::db::{ChannelPairingStore, Database, UserRecord, UserStore};
    use brassclaw::ownership::{UserId, UserRole};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Create a file-backed test DB with migrations applied.
    async fn setup_db() -> (LibSqlBackend, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("ownership_test.db");
        let db = LibSqlBackend::new_local(&db_path)
            .await
            .expect("test DB creation failed");
        db.run_migrations().await.expect("run migrations");
        (db, dir)
    }

    async fn create_user(db: &LibSqlBackend, id: &str, role: &str) {
        db.get_or_create_user(UserRecord {
            id: id.to_string(),
            role: role.to_string(),
            display_name: id.to_string(),
            status: "active".to_string(),
            email: None,
            last_login_at: None,
            created_by: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            metadata: serde_json::Value::Null,
        })
        .await
        .expect("user creation failed");
    }

    // -----------------------------------------------------------------------
    // Bootstrap tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_bootstrap_creates_owner_user() {
        let (db, _dir) = setup_db().await;

        // Owner does not exist yet
        assert!(db.get_user("henry").await.unwrap().is_none());

        // Create the owner via get_or_create_user (atomic upsert)
        create_user(&db, "henry", "admin").await;

        let user = db
            .get_user("henry")
            .await
            .unwrap()
            .expect("owner should exist");
        assert_eq!(user.id, "henry");
        assert_eq!(user.role, "admin");
        assert_eq!(user.status, "active");
    }

    #[tokio::test]
    async fn test_bootstrap_get_or_create_is_idempotent() {
        let (db, _dir) = setup_db().await;

        // Call twice — should not error or duplicate
        create_user(&db, "henry", "admin").await;
        create_user(&db, "henry", "admin").await;

        // Exactly one row
        let user = db
            .get_user("henry")
            .await
            .unwrap()
            .expect("owner should exist");
        assert_eq!(user.id, "henry");
    }

    #[tokio::test]
    async fn test_bootstrap_rewrites_default_user_id() {
        let (db, _dir) = setup_db().await;

        // Create a 'default' user (the pre-ownership placeholder)
        create_user(&db, "default", "member").await;

        // Insert a settings row with user_id = 'default'
        {
            let conn = db.connect().await.unwrap();
            conn.execute(
                "INSERT INTO settings (user_id, key, value, updated_at) \
                 VALUES ('default', 'test_migration_key', '\"test_value\"', \
                         strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
                (),
            )
            .await
            .expect("insert settings row");
        }

        // Create the real owner
        create_user(&db, "henry", "admin").await;

        // Run migrate_default_owner
        db.migrate_default_owner("henry").await.unwrap();

        // The settings row should now be under 'henry'
        let conn = db.connect().await.unwrap();
        let mut rows = conn
            .query(
                "SELECT user_id FROM settings WHERE key = 'test_migration_key'",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("row should exist");
        let user_id: String = row.get(0).unwrap();
        assert_eq!(
            user_id, "henry",
            "migrate_default_owner should rewrite 'default' to real owner"
        );
    }

    #[tokio::test]
    async fn test_migrate_default_owner_is_idempotent() {
        let (db, _dir) = setup_db().await;
        create_user(&db, "henry", "admin").await;

        // Run twice — should not error
        db.migrate_default_owner("owner-bootstrap-test")
            .await
            .unwrap();
        db.migrate_default_owner("henry").await.unwrap();

        // Still exactly one henry row
        let user = db.get_user("henry").await.unwrap();
        assert!(user.is_some());
    }

    #[tokio::test]
    async fn test_migrate_default_owner_no_default_rows() {
        let (db, _dir) = setup_db().await;
        create_user(&db, "henry", "admin").await;

        // No 'default' rows to migrate — should succeed without error
        db.migrate_default_owner("henry").await.unwrap();
    }

    #[tokio::test]
    async fn test_migrate_default_owner_succeeds_on_fresh_migrated_db() {
        let (db, _dir) = setup_db().await;

        // Fresh installs include ownerless tables like `dynamic_tools`; the
        // bootstrap rewrite should still succeed without assuming every table
        // in the schema carries a `user_id` column.
        db.migrate_default_owner("henry").await.unwrap();
    }

    // -----------------------------------------------------------------------
    // ChannelPairingStore tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_pairing_different_users_are_independent() {
        let (db, _dir) = setup_db().await;
        create_user(&db, "alice", "member").await;
        create_user(&db, "bob", "member").await;

        let req_a = db
            .upsert_pairing_request("telegram", "tg-alice", None)
            .await
            .unwrap();
        let req_b = db
            .upsert_pairing_request("telegram", "tg-bob", None)
            .await
            .unwrap();

        db.approve_pairing("telegram", &req_a.code, "alice")
            .await
            .unwrap();
        db.approve_pairing("telegram", &req_b.code, "bob")
            .await
            .unwrap();

        let alice_id = db
            .resolve_channel_identity("telegram", "tg-alice")
            .await
            .unwrap()
            .expect("alice should be linked");
        let bob_id = db
            .resolve_channel_identity("telegram", "tg-bob")
            .await
            .unwrap()
            .expect("bob should be linked");

        assert_eq!(alice_id.as_str(), "alice");
        assert_eq!(bob_id.as_str(), "bob");
        assert_ne!(alice_id, bob_id);
    }

    #[tokio::test]
    async fn test_pairing_channels_are_isolated() {
        let (db, _dir) = setup_db().await;
        create_user(&db, "alice", "member").await;

        // Same external_id across two different channels
        let req_telegram = db
            .upsert_pairing_request("telegram", "user-999", None)
            .await
            .unwrap();
        let _req_slack = db
            .upsert_pairing_request("slack", "user-999", None)
            .await
            .unwrap();

        // Approve only telegram
        db.approve_pairing("telegram", &req_telegram.code, "alice")
            .await
            .unwrap();

        // telegram resolves; slack does not
        assert!(
            db.resolve_channel_identity("telegram", "user-999")
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            db.resolve_channel_identity("slack", "user-999")
                .await
                .unwrap()
                .is_none()
        );
    }

    // -----------------------------------------------------------------------
    // UserId unit-level sanity
    // -----------------------------------------------------------------------

    #[test]
    fn test_user_id_equality_and_display() {
        let a = UserId::from_trusted("alice".into(), UserRole::Regular);
        let b = UserId::from_trusted("alice".into(), UserRole::Regular);
        let c = UserId::from_trusted("bob".into(), UserRole::Regular);

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.as_str(), "alice");
        assert_eq!(a.to_string(), "alice");
    }

    #[test]
    fn test_user_id_new_validates() {
        assert!(UserId::new("", UserRole::Regular).is_err());
        assert!(UserId::new("   ", UserRole::Regular).is_err());
        assert!(UserId::new("alice", UserRole::Owner).is_ok());
    }

    #[test]
    fn test_user_role_from_db_role_covers_owner_admin_regular() {
        assert_eq!(UserRole::from_db_role("owner"), UserRole::Owner);
        assert_eq!(UserRole::from_db_role("admin"), UserRole::Admin);
        assert_eq!(UserRole::from_db_role("regular"), UserRole::Regular);
        // Legacy value from the old two-variant enum must keep deserializing.
        assert_eq!(UserRole::from_db_role("member"), UserRole::Regular);
    }

    #[test]
    fn test_owner_is_admin_regular_is_not() {
        let owner = UserId::from_trusted("root".into(), UserRole::Owner);
        assert!(owner.is_admin(), "Owner must satisfy is_admin()");
        assert!(owner.is_owner());
        assert!(!owner.is_regular());

        let reg = UserId::from_trusted("alice".into(), UserRole::Regular);
        assert!(!reg.is_admin());
        assert!(!reg.is_owner());
        assert!(reg.is_regular());
    }

}
