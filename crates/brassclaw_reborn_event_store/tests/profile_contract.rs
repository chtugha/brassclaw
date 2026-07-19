use brassclaw_reborn_event_store::{
    RebornEventStoreConfig, RebornEventStoreError, RebornProfile, build_reborn_event_stores,
};
use secrecy::SecretString;

#[cfg(not(feature = "postgres"))]
#[tokio::test]
async fn unavailable_postgres_backend_error_does_not_leak_secret_config() {
    let result = build_reborn_event_stores(
        RebornProfile::Production,
        RebornEventStoreConfig::Postgres {
            url: SecretString::new(
                "postgres://event_user:RAW_PASSWORD_SENTINEL_3162@example.invalid/db"
                    .to_string()
                    .into_boxed_str(),
            ),
        },
    )
    .await;

    let error = result
        .err()
        .expect("postgres adapter is unavailable when the feature is disabled");
    assert!(matches!(
        error,
        RebornEventStoreError::BackendUnavailable {
            backend: "postgres"
        }
    ));
    let displayed = error.to_string();
    assert!(!displayed.contains("RAW_PASSWORD_SENTINEL_3162"));
    assert!(!displayed.contains("example.invalid"));
    assert!(!displayed.contains("postgres://"));
    let debug = format!("{error:?}");
    assert!(!debug.contains("RAW_PASSWORD_SENTINEL_3162"));
    assert!(!debug.contains("example.invalid"));
    assert!(!debug.contains("postgres://"));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn production_postgres_rejects_remote_sslmode_disable_before_connecting() {
    let result = build_reborn_event_stores(
        RebornProfile::Production,
        RebornEventStoreConfig::Postgres {
            url: SecretString::new(
                "postgres://event_user:RAW_PASSWORD_SENTINEL_3162@db.example.com/events?sslmode=disable"
                    .to_string()
                    .into_boxed_str(),
            ),
        },
    )
    .await;

    let error = result
        .err()
        .expect("remote postgres sslmode=disable must fail closed before connect");
    assert!(matches!(
        error,
        RebornEventStoreError::RemotePostgresClearTextDisabled
    ));
    let displayed = error.to_string();
    assert!(!displayed.contains("RAW_PASSWORD_SENTINEL_3162"));
    assert!(!displayed.contains("db.example.com"));
    assert!(!displayed.contains("postgres://"));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_connection_failure_does_not_fall_back_or_leak_secret_config() {
    let result = build_reborn_event_stores(
        RebornProfile::Production,
        RebornEventStoreConfig::Postgres {
            url: SecretString::new(
                "postgres://event_user:RAW_PASSWORD_SENTINEL_3162@example.invalid/db"
                    .to_string()
                    .into_boxed_str(),
            ),
        },
    )
    .await;

    let error = result
        .err()
        .expect("postgres adapter should try to connect and fail closed");
    assert!(
        !matches!(
            error,
            RebornEventStoreError::BackendUnavailable {
                backend: "postgres"
            }
        ),
        "postgres feature must enable the concrete adapter"
    );
    let displayed = error.to_string();
    assert!(!displayed.contains("RAW_PASSWORD_SENTINEL_3162"));
    assert!(!displayed.contains("example.invalid"));
    assert!(!displayed.contains("postgres://"));
    let debug = format!("{error:?}");
    assert!(!debug.contains("RAW_PASSWORD_SENTINEL_3162"));
    assert!(!debug.contains("example.invalid"));
    assert!(!debug.contains("postgres://"));
}
