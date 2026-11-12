//! Existing B03 transaction connection custody, shared by PostgreSQL owners.
//! An unacknowledged transaction must not put its connection back in the pool.
pub struct TransactionClient {
    inner: Option<crate::PgClient>,
    recyclable: bool,
    retirement: Option<std::sync::Arc<retirement::TransactionRetirement>>,
}
impl TransactionClient {
    pub fn from_client(client: crate::PgClient) -> Self {
        Self {
            inner: Some(client),
            recyclable: false,
            retirement: None,
        }
    }
    /// Use only with the original IO owner's retained, bounded retirement
    /// custody. Ordinary transaction users keep their existing constructor.
    pub(super) fn with_retirement(
        client: crate::PgClient,
        retirement: std::sync::Arc<retirement::TransactionRetirement>,
    ) -> Self {
        Self {
            inner: Some(client),
            recyclable: false,
            retirement: Some(retirement),
        }
    }
    pub fn recycle_after_commit(&mut self) {
        self.recyclable = true;
    }
}
impl std::ops::Deref for TransactionClient {
    type Target = crate::PgClient;
    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("owned transaction connection")
    }
}
impl std::ops::DerefMut for TransactionClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().expect("owned transaction connection")
    }
}
impl Drop for TransactionClient {
    fn drop(&mut self) {
        if self.recyclable
            && let Some(retirement) = &self.retirement
        {
            retirement.recycled();
        }
        if !self.recyclable
            && let Some(client) = self.inner.take()
        {
            let client = crate::PgClient::take(client);
            match &self.retirement {
                Some(retirement) => retirement.retire(client),
                None => drop(client),
            }
        }
    }
}

#[path = "transaction_retirement.rs"]
pub mod retirement;
