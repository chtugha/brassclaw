use brassclaw_filesystem::{CasExpectation, Entry, FileType, RootFilesystem};
use brassclaw_host_api::VirtualPath;

/// Exercise real backend storage, including several descendant rows belonging
/// to one direct child. A LIMIT applied before deduplication loses siblings.
pub(super) async fn assert_bounded_directory_contract(filesystem: &dyn RootFilesystem) {
    let parent = VirtualPath::new("/projects/bounded_%!/ü").unwrap();
    for index in 0..100 {
        filesystem
            .put(
                &VirtualPath::new(format!("{}/alpha/{index:03}.txt", parent.as_str())).unwrap(),
                Entry::bytes(b"child".to_vec()),
                CasExpectation::Absent,
            )
            .await
            .unwrap();
    }
    for name in ["beta.txt", "界.txt"] {
        filesystem
            .put(
                &VirtualPath::new(format!("{}/{name}", parent.as_str())).unwrap(),
                Entry::bytes(b"sibling".to_vec()),
                CasExpectation::Absent,
            )
            .await
            .unwrap();
    }
    filesystem
        .put(
            &VirtualPath::new("/projects/boundedAX/ü/foreign.txt").unwrap(),
            Entry::bytes(b"outside the literal wildcard-containing prefix".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    let complete = filesystem.list_dir(&parent).await.unwrap();
    assert_eq!(complete.len(), 3);
    assert_eq!(complete[0].name, "alpha");
    assert_eq!(complete[0].file_type, FileType::Directory);
    for limit in [0, 1, 2, 3, 4] {
        let bounded = filesystem.list_dir_bounded(&parent, limit).await.unwrap();
        assert_eq!(bounded.len(), limit.min(complete.len()));
        assert!(bounded.windows(2).all(|pair| pair[0].name < pair[1].name));
        assert!(bounded.iter().all(|entry| complete.contains(entry)));
        if limit >= complete.len() {
            assert_eq!(bounded, complete);
        }
    }
}
