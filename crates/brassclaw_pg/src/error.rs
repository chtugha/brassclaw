use thiserror::Error;

#[derive(Debug, Error)]
pub enum PgError {
    #[error("failed to connect to PostgreSQL: {0}")]
    Connect(String),

    #[error("pool build error: {0}")]
    Pool(String),

    #[error("migration failed: {0}")]
    Migration(String),

    #[error("query error: {0}")]
    Query(String),

    #[error("invalid connection URL: {0}")]
    InvalidUrl(String),
}

impl From<tokio_postgres::Error> for PgError {
    fn from(e: tokio_postgres::Error) -> Self {
        Self::Query(e.to_string())
    }
}

impl From<deadpool_postgres::PoolError> for PgError {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        Self::Pool(e.to_string())
    }
}

impl From<deadpool_postgres::BuildError> for PgError {
    fn from(e: deadpool_postgres::BuildError) -> Self {
        Self::Pool(e.to_string())
    }
}
