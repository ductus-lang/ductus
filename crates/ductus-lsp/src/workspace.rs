//! The `.ductus` files under the workspace folders.

use std::path::{Path, PathBuf};

/// Every `.ductus` file under `roots`, canonical and sorted. Hidden
/// directories, `target` and `node_modules` are skipped; symbolic links to
/// directories are not followed, so the walk cannot loop.
pub fn ductus_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in roots {
        walk(root, &mut files);
    }
    files.sort();
    files.dedup();
    files
}

fn walk(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !(name.starts_with('.') || name == "target" || name == "node_modules") {
                walk(&path, files);
            }
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "ductus") {
            files.push(path.canonicalize().unwrap_or(path));
        }
    }
}
