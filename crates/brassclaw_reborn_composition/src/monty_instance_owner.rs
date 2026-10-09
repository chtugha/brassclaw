//! Exclusive database-instance ownership candidate for the global VM cutover.
//! Kept behind the caller acceptance gate until global hosting is wired. This
//! guard alone is neither startup readiness nor a dispatch/cancellation lease.
//! The supervisor must retain it until the VM and all hosted calls have stopped,
//! fence dispatch on connection loss, and shut it down before the database.

use brassclaw_pg::PgPool;
use deadpool_postgres::{Client, ClientWrapper};

// Two-int advisory namespace, distinct from PostgreSQL's single-bigint keys.
// One database is the instance's storage boundary; tenant/chat IDs never create
// another root owner. These stable keys must not vary across binary versions.
const INSTANCE_NAMESPACE: i32 = 0x4252_434c;
const MONTY_OWNER: i32 = 0x4d4f_4e54;

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub(crate) enum OwnershipError {
    #[error("global Monty already has an instance owner")]
    AlreadyOwned,
    #[error("global Monty ownership connection failed")]
    Connection,
    #[error("global Monty ownership check capacity is occupied")]
    Busy,
    #[error("global Monty ownership check expired before execution")]
    Deadline,
    #[error("global Monty instance ownership was lost")]
    Lost,
}

pub(crate) struct PgMontyOwner {
    // Permanently detached from the pool before sending the first lock query.
    // Cancellation/drop therefore closes the session rather than recycling a
    // connection holding a reentrant PostgreSQL advisory lock.
    connection: ClientWrapper,
}

impl PgMontyOwner {
    pub(crate) async fn acquire(pool: &PgPool) -> Result<Self, OwnershipError> {
        let pooled = pool.get().await.map_err(|_| OwnershipError::Connection)?;
        let connection = Client::take(pooled);
        let row = connection
            .query_one(
                "SELECT pg_try_advisory_lock($1::int, $2::int)",
                &[&INSTANCE_NAMESPACE, &MONTY_OWNER],
            )
            .await
            .map_err(|_| OwnershipError::Connection)?;
        if !row.get::<_, bool>(0) {
            return Err(OwnershipError::AlreadyOwned);
        }
        Ok(Self { connection })
    }

    /// Actual roundtrip, without reacquiring/incrementing a reentrant lock.
    /// The service supplies a bounded deadline and fences hosted calls on error.
    /// This read does not replace durable attempt fencing at effect dispatch.
    pub(crate) async fn check(&self) -> Result<(), OwnershipError> {
        let row = self
            .connection
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM pg_locks
             WHERE locktype = 'advisory' AND pid = pg_backend_pid()
               AND classid::bigint = $1 AND objid::bigint = $2 AND objsubid = 2
               AND mode = 'ExclusiveLock' AND granted)",
                &[&i64::from(INSTANCE_NAMESPACE), &i64::from(MONTY_OWNER)],
            )
            .await
            .map_err(|_| OwnershipError::Connection)?;
        if row.get::<_, bool>(0) {
            Ok(())
        } else {
            Err(OwnershipError::Lost)
        }
    }

    /// Call only after the root VM and hosted operations are quiescent. Error
    /// remains an error; dropping the detached session still releases ownership
    /// when PostgreSQL observes disconnection, without claiming an acknowledgement.
    pub(crate) async fn release(self) -> Result<(), OwnershipError> {
        let row = self
            .connection
            .query_one(
                "SELECT pg_advisory_unlock($1::int, $2::int)",
                &[&INSTANCE_NAMESPACE, &MONTY_OWNER],
            )
            .await
            .map_err(|_| OwnershipError::Connection)?;
        if row.get::<_, bool>(0) {
            Ok(())
        } else {
            Err(OwnershipError::Lost)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn native_owner_excludes_second_runtime_and_closes_abandoned_session() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let owner = PgMontyOwner::acquire(&rig.pool).await.expect("first owner");
        owner.check().await.expect("live ownership");
        owner
            .check()
            .await
            .expect("check does not reacquire the lock");
        assert!(matches!(
            PgMontyOwner::acquire(&rig.pool).await,
            Err(OwnershipError::AlreadyOwned)
        ));
        owner
            .release()
            .await
            .expect("one unlock releases ownership");
        let abandoned = PgMontyOwner::acquire(&rig.pool).await.expect("next owner");
        drop(abandoned);
        // Closing a connection is asynchronous; no fake immediate drop ack.
        let owner = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match PgMontyOwner::acquire(&rig.pool).await {
                    Ok(owner) => break owner,
                    Err(OwnershipError::AlreadyOwned) => {
                        tokio::time::sleep(Duration::from_millis(10)).await
                    }
                    Err(error) => panic!("ownership connection failed: {error}"),
                }
            }
        })
        .await
        .expect("abandoned session releases the database lock");
        owner.check().await.expect("replacement owner");
        // Exercise actual ownership loss without a simulated database/client.
        owner
            .connection
            .query_one(
                "SELECT pg_advisory_unlock($1::int, $2::int)",
                &[&INSTANCE_NAMESPACE, &MONTY_OWNER],
            )
            .await
            .expect("real lock loss");
        assert!(matches!(owner.check().await, Err(OwnershipError::Lost)));
        assert!(matches!(owner.release().await, Err(OwnershipError::Lost)));
        let owner = PgMontyOwner::acquire(&rig.pool)
            .await
            .expect("lost owner replaced");
        owner.release().await.expect("final release");
    }
}
