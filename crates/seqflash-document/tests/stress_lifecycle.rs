//! Multi-document open/close lifecycle stress (M8: no handle leaks under churn).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use seqflash_document::DocumentList;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn rapid_open_close_many_files() {
    let dir = tempdir().expect("tempdir");
    let mut paths = Vec::new();
    for i in 0..32 {
        let p = dir.path().join(format!("f{i}.fa"));
        let mut f = std::fs::File::create(&p).expect("create");
        writeln!(f, ">s{i}").unwrap();
        writeln!(f, "ACGT").unwrap();
        paths.push(p);
    }

    let mut list = DocumentList::new();
    // Open all, then close all, repeated.
    for _ in 0..4 {
        let mut ids = Vec::new();
        for p in &paths {
            let id = list.open(p).expect("open");
            ids.push(id);
        }
        assert_eq!(list.len(), paths.len());
        for id in ids {
            assert!(list.close(id));
        }
        assert_eq!(list.len(), 0);
    }
}

#[test]
fn reopen_same_path_after_close() {
    let dir = tempdir().expect("tempdir");
    let p = dir.path().join("once.fa");
    std::fs::write(&p, b">a\nAC\n").unwrap();

    let mut list = DocumentList::new();
    let id1 = list.open(&p).unwrap();
    assert!(list.close(id1));
    let id2 = list.open(&p).unwrap();
    assert_ne!(id1, id2, "ids must not be reused");
    assert!(list.get(id2).is_some());
}
