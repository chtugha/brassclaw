//! Existing B03 connection custody; canonical implementation lives with PG so
//! lower immutable stores and Composition use the same discard/recycle owner.
pub(crate) use brassclaw_pg::transaction_client::TransactionClient;
