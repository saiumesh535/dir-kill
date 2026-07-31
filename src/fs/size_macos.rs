//! macOS-only directory size via `getattrlistbulk` (batched metadata syscalls).

use anyhow::Result;
use getattrlistbulk::{DirReader, ObjectType};
use std::path::{Path, PathBuf};

/// Apparent size of a directory tree using bulk directory reads.
///
/// Does not follow symlinks. Skips `.` / `..`. Errors reading a subdirectory
/// are ignored (same soft-fail spirit as the jwalk path).
pub fn calculate_directory_size(path: &Path) -> Result<u64> {
    if !path.is_dir() {
        return Ok(0);
    }

    let mut total = 0u64;
    let mut stack: Vec<PathBuf> = vec![path.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match DirReader::new(&dir)
            .name()
            .object_type()
            .size()
            .follow_symlinks(false)
            .buffer_size(256 * 1024)
            .read()
        {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };

            match entry.object_type {
                Some(ObjectType::Regular) => {
                    total += entry.size.unwrap_or(0);
                }
                Some(ObjectType::Directory) => {
                    if entry.name == "." || entry.name == ".." {
                        continue;
                    }
                    stack.push(dir.join(&entry.name));
                }
                // Symlinks and special nodes: do not follow / do not count as dirs.
                _ => {}
            }
        }
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn sizes_nested_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("a/f.txt"), b"hello").unwrap();
        fs::write(root.join("a/b/g.txt"), b"world!!").unwrap();

        let size = calculate_directory_size(root).unwrap();
        assert_eq!(size, 5 + 7);
    }

    #[test]
    fn matches_jwalk_fallback_on_temp_tree() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("pkg/lib")).unwrap();
        fs::write(root.join("pkg/a.js"), vec![1u8; 100]).unwrap();
        fs::write(root.join("pkg/lib/b.js"), vec![2u8; 50]).unwrap();

        let bulk = calculate_directory_size(root).unwrap();
        let jwalk = crate::fs::calculate_directory_size_jwalk_fallback(root).unwrap();
        assert_eq!(bulk, jwalk);
        assert_eq!(bulk, 150);
    }
}
