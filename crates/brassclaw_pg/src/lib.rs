pub mod error;
pub mod migrations;
pub mod pool;

pub use deadpool_postgres::Pool as PgPool;
pub use error::PgError;

/// Re-export the underlying client type for callers that need it.
pub use deadpool_postgres::Client as PgClient;

/// Re-export the row type for callers that work with query results directly.
pub use deadpool_postgres::tokio_postgres::Row as PgRow;
