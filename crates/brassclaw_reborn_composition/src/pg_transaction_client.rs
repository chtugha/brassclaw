//! Existing B03 transaction connection custody, shared by PostgreSQL owners.
//! An unacknowledged transaction must not put its connection back in the pool.
pub(crate) struct TransactionClient {
    inner: Option<brassclaw_pg::PgClient>,
    recyclable: bool,
}
impl TransactionClient {
    pub(crate) fn from_client(client: brassclaw_pg::PgClient) -> Self {
        Self {
            inner: Some(client),
            recyclable: false,
        }
    }
    pub(crate) fn recycle_after_commit(&mut self) {
        self.recyclable = true;
    }
}
impl std::ops::Deref for TransactionClient {
    type Target = brassclaw_pg::PgClient;
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
        if !self.recyclable {
            if let Some(client) = self.inner.take() {
                drop(brassclaw_pg::PgClient::take(client));
            }
        }
    }
}
