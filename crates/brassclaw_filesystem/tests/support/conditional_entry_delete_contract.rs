//! Shared actual-owner behavior; no replacement backend or simulated success.
use brassclaw_filesystem::{Capability, CasExpectation, Entry, EntryBodyChecksum, RootFilesystem};
use brassclaw_host_api::VirtualPath;

pub(super) async fn verify<F: RootFilesystem>(first: &F, second: &F) {
    assert!(first.capabilities().has(Capability::ConditionalEntryDelete));
    assert!(
        second
            .capabilities()
            .has(Capability::ConditionalEntryDelete)
    );
    let path = VirtualPath::new("/tenants/conditional-entry-delete/value").unwrap();
    let initial = first
        .put(
            &path,
            Entry::bytes(b"original body".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    let checksum = EntryBodyChecksum::for_body(b"original body");
    let advanced = second
        .put(
            &path,
            Entry::bytes(b"advanced body".to_vec()),
            CasExpectation::Version(initial),
        )
        .await
        .unwrap();
    assert_ne!(advanced, initial);
    assert!(
        !first
            .delete_if_snapshot(&path, initial, checksum)
            .await
            .unwrap()
    );
    assert_eq!(
        first.get(&path).await.unwrap().unwrap().entry.body,
        b"advanced body"
    );
    second.delete(&path).await.unwrap();
    assert_eq!(
        second
            .put(
                &path,
                Entry::bytes(b"recreated body".to_vec()),
                CasExpectation::Absent
            )
            .await
            .unwrap(),
        initial
    );
    assert!(
        !first
            .delete_if_snapshot(&path, initial, checksum)
            .await
            .unwrap()
    );
    let recreated = first.get(&path).await.unwrap().unwrap();
    assert_eq!(recreated.entry.body, b"recreated body");
    let checksum = EntryBodyChecksum::for_body(&recreated.entry.body);
    let (a, b) = tokio::join!(
        first.delete_if_snapshot(&path, recreated.version, checksum),
        second.delete_if_snapshot(&path, recreated.version, checksum)
    );
    assert_ne!(
        a.unwrap(),
        b.unwrap(),
        "exactly one actual selected-leaf deletion"
    );
    assert!(first.get(&path).await.unwrap().is_none());
    assert!(
        !first
            .delete_if_snapshot(&path, recreated.version, checksum)
            .await
            .unwrap()
    );
    let child = VirtualPath::new("/tenants/conditional-entry-delete/parent/child").unwrap();
    let version = first
        .put(
            &child,
            Entry::bytes(b"child".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    let parent = VirtualPath::new("/tenants/conditional-entry-delete/parent").unwrap();
    assert!(
        !first
            .delete_if_snapshot(&parent, version, EntryBodyChecksum::for_body(b"child"))
            .await
            .unwrap()
    );
    assert_eq!(
        first.get(&child).await.unwrap().unwrap().entry.body,
        b"child"
    );
}
