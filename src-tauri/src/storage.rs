//! What History and Annotate take up on this Mac.
//!
//! Counted from the files as they are, not tracked as they are written: a
//! session the user moved, a folder deleted outside the app, or a database
//! that was vacuumed reads as what is on disk now.

use serde::Serialize;
use std::path::Path;
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    #[ts(type = "number")]
    pub history_bytes: u64,
    #[ts(type = "number")]
    pub annotation_bytes: u64,
}

/// Every byte under `path`, or the length of `path` when it is a file. A path
/// that is not there counts as nothing, so a fresh install reports zero rather
/// than an error.
pub fn bytes_in(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| bytes_in(&entry.path()))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_counts_its_own_length_and_a_folder_counts_everything_under_it() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("2026-11-16-171234");
        std::fs::create_dir_all(nested.join("frames")).unwrap();
        std::fs::write(dir.path().join("history.sqlite3"), vec![0u8; 300]).unwrap();
        std::fs::write(nested.join("annotation.json"), [0u8; 20]).unwrap();
        std::fs::write(nested.join("frames/001.jpg"), [0u8; 1500]).unwrap();
        assert_eq!(bytes_in(&dir.path().join("history.sqlite3")), 300);
        assert_eq!(bytes_in(dir.path()), 1820);
    }

    #[test]
    fn a_missing_path_is_nothing_on_disk_not_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(bytes_in(&dir.path().join("never-written.sqlite3")), 0);
        assert_eq!(bytes_in(&dir.path().join("no-such-folder")), 0);
    }

    #[test]
    fn typescript_contract_matches_rust_storage() {
        let checked_in = include_str!("../../src/storage.generated.ts");
        let compact = |text: &str| {
            text.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .replace(',', ";")
                .replace(";}", "}")
        };
        assert_eq!(
            compact(checked_in),
            compact(&format!("export {}", StorageUsage::decl()))
        );
    }
}
