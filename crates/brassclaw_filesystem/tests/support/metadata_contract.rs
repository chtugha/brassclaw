//! Body-free projection over actual storage implementations, never a fake FS.
use brassclaw_filesystem::{
    Capability, CasExpectation, Entry, EntryIndexMetadata, EntryMetadata, IndexKey, IndexValue,
    Page, RecordKind, RootFilesystem,
};
use brassclaw_host_api::VirtualPath;
use std::sync::Arc;

pub(crate) async fn assert_metadata_contract(root: Arc<dyn RootFilesystem>) {
    assert!(root.capabilities().has(Capability::EntryMetadata));
    let base = format!("/projects/meta_!%{}", uuid::Uuid::new_v4());
    let prefix = VirtualPath::new(&base).unwrap();
    let mut expected = Vec::new();
    let key = IndexKey::new("selected").unwrap();
    let mut selected = std::collections::BTreeMap::new();
    let kind = RecordKind::new("projection_fixture").unwrap();
    for index in 0..(Page::MAX_LIMIT as usize * 2 + 17) {
        let path = VirtualPath::new(format!("{base}/{index:05}")).unwrap();
        let body = if index == 0 {
            vec![b'x'; 2 * 1024 * 1024]
        } else {
            vec![b'x']
        };
        let mut entry = Entry::record(kind.clone(), &serde_json::Value::Null).unwrap();
        entry.body = body;
        entry
            .indexed
            .insert(key.clone(), IndexValue::I64(index as i64));
        selected.insert(path.clone(), Some(IndexValue::I64(index as i64)));
        root.put(&path, entry, CasExpectation::Absent)
            .await
            .unwrap();
        expected.push(EntryMetadata {
            path,
            kind: Some(kind.clone()),
        });
    }
    // Names outside the prefix and an inferred directory are not entries.
    let sibling = VirtualPath::new(format!("{base}-sibling/file")).unwrap();
    root.put(
        &sibling,
        Entry::bytes(b"outside".to_vec()),
        CasExpectation::Absent,
    )
    .await
    .unwrap();
    for lookalike in [
        base.replace('_', "x"),
        base.replace('%', "wild"),
        base.replace('!', ""),
    ] {
        let path = VirtualPath::new(format!("{lookalike}/outside")).unwrap();
        root.put(
            &path,
            Entry::bytes(b"outside".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    }
    assert!(root.get_metadata(&prefix).await.unwrap().is_none());
    for name in ["Z", "a", "ü", "😀"] {
        let path = VirtualPath::new(format!("{base}/{name}")).unwrap();
        let mut entry = Entry::bytes(vec![b'y'; 4096]);
        let value = match name {
            "Z" => Some(IndexValue::Bool(true)),
            "a" => Some(IndexValue::Text("plain".into())),
            "ü" => Some(IndexValue::Bytes(vec![0, 255])),
            _ => None,
        };
        if let Some(value) = &value {
            entry.indexed.insert(key.clone(), value.clone());
        }
        selected.insert(path.clone(), value);
        entry.indexed.insert(
            IndexKey::new("large").unwrap(),
            IndexValue::Bytes(vec![7; 1024 * 1024]),
        );
        root.put(&path, entry, CasExpectation::Absent)
            .await
            .unwrap();
        expected.push(EntryMetadata { path, kind: None });
    }
    expected.sort_by(|a, b| a.path.cmp(&b.path));
    let mut actual = Vec::new();
    let mut offset = 0;
    loop {
        let page = root
            .query_metadata(&prefix, Page::new(offset, Page::MAX_LIMIT))
            .await
            .unwrap();
        let received = page.len();
        assert!(received <= Page::MAX_LIMIT as usize);
        actual.extend(page);
        if received < Page::MAX_LIMIT as usize {
            break;
        }
        offset += received as u64;
    }
    assert_eq!(actual, expected);
    assert!(root.capabilities().has(Capability::EntryIndexMetadata));
    let mut actual_selected = Vec::new();
    let mut offset = 0;
    loop {
        let page = root
            .query_index_metadata(&prefix, &key, Page::new(offset, Page::MAX_LIMIT))
            .await
            .unwrap();
        let received = page.len();
        actual_selected.extend(page);
        if received < Page::MAX_LIMIT as usize {
            break;
        }
        offset += received as u64;
    }
    assert_eq!(
        actual_selected,
        expected
            .iter()
            .map(|metadata| EntryIndexMetadata {
                metadata: metadata.clone(),
                value: selected.get(&metadata.path).unwrap().clone(),
            })
            .collect::<Vec<_>>()
    );
    assert!(
        root.query_index_metadata(&prefix, &key, Page::first(0))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        root.query_index_metadata(&prefix, &key, Page::new(expected.len() as u64, 1))
            .await
            .unwrap()
            .is_empty()
    );
    let absent = IndexKey::new("absent").unwrap();
    assert!(
        root.query_index_metadata(&prefix, &absent, Page::default())
            .await
            .unwrap()
            .iter()
            .all(|entry| entry.value.is_none())
    );

    for entry in [&expected[0], expected.last().unwrap()] {
        assert_eq!(
            root.get_metadata(&entry.path).await.unwrap().as_ref(),
            Some(entry)
        );
        assert_eq!(
            root.query_metadata(&entry.path, Page::first(1))
                .await
                .unwrap(),
            vec![entry.clone()]
        );
    }
    assert!(
        root.query_metadata(&prefix, Page::first(0))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        root.query_metadata(&prefix, Page::new(expected.len() as u64, 1))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        root.get_metadata(&VirtualPath::new(format!("{base}/missing")).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        root.get(&expected[0].path)
            .await
            .unwrap()
            .unwrap()
            .entry
            .body
            .len(),
        2 * 1024 * 1024
    );
}
