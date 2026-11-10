//! Import resolution that sees unsaved editor buffers.

use ductus_core::eval::Importer;
use std::path::{Path, PathBuf};

/// Reads a file's text: the open document when there is one, else disk.
pub type Read<'a> = &'a dyn Fn(&Path) -> Option<String>;

/// The file `path` names when imported from `from`: relative to the
/// importing file's directory, canonicalized when it exists so that one
/// file reached by different routes has one identity.
pub fn resolve(from: &Path, path: &str) -> PathBuf {
    let joined = from.parent().unwrap_or(Path::new("")).join(path);
    joined.canonicalize().unwrap_or(joined)
}

pub struct WorkspaceImporter<'a> {
    /// The file being evaluated; top-level imports resolve against it.
    pub root: &'a Path,
    pub read: Read<'a>,
}

impl Importer for WorkspaceImporter<'_> {
    fn load(&self, path: &str, from: Option<&str>) -> Result<(String, String), String> {
        let from = from.map_or(self.root, Path::new);
        let resolved = resolve(from, path);
        let source =
            (self.read)(&resolved).ok_or_else(|| format!("cannot read {}", resolved.display()))?;
        Ok((resolved.to_string_lossy().into_owned(), source))
    }
}
