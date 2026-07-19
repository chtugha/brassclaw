pub mod error;
pub mod migrations;
pub mod pool;

pub use deadpool_postgres::Pool as PgPool;
pub use error::PgError;

/// Re-export the underlying client type for callers that need it.
pub use deadpool_postgres::Client as PgClient;
