#![cfg(feature = "postgres")]

use brassclaw_filesystem::{PostgresRootFilesystem, RootFilesystem};

#[cfg(feature = "postgres")]
#[test]
fn postgres_root_filesystem_implements_root_filesystem_contract() {
    fn assert_root<T: RootFilesystem>() {}
    assert_root::<PostgresRootFilesystem>();
}
