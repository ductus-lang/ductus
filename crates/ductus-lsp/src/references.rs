//! Find references: the definition resolver run over every candidate
//! identifier, keeping those that resolve to the binding.

use crate::definition::{Target, definition};
use crate::importer::Read;
use crate::syntax;
use ductus_core::parse::parse_lenient;
use std::path::PathBuf;

/// Every identifier in `files` that resolves to `binding`, the binding's
/// own name included, in file and document order.
pub fn references(binding: &Target, files: &[PathBuf], read: Read) -> Vec<Target> {
    let Some(source) = read(&binding.path) else {
        return Vec::new();
    };
    let name = &source[binding.range.clone()];
    if name.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for file in files {
        let Some(source) = read(file) else {
            continue;
        };
        if !source.contains(name) {
            continue;
        }
        let tree = parse_lenient(&source);
        for node in syntax::identifiers(tree.root_node(), &source, name) {
            let target = definition(&tree, &source, file, node.start_byte(), read);
            if target.as_ref() == Some(binding) {
                found.push(Target {
                    path: file.clone(),
                    range: node.byte_range(),
                });
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::Path;

    const LIB: &str = "/virtual/lib.ductus";
    const MAIN: &str = "/virtual/main.ductus";

    /// The lines (1-based) of `name`'s references, per file, for the
    /// binding found at the first `binding` occurrence in `in_file`.
    fn lines(files: &[(&str, &str)], in_file: &str, binding: &str) -> Vec<(String, usize)> {
        let texts: HashMap<PathBuf, String> = files
            .iter()
            .map(|(path, text)| (PathBuf::from(path), text.to_string()))
            .collect();
        let read = |path: &Path| texts.get(path).cloned();
        let source = &texts[Path::new(in_file)];
        let start = source.find(binding).unwrap();
        let name_len = binding.find([' ', '(']).unwrap_or(binding.len());
        let target = Target {
            path: PathBuf::from(in_file),
            range: start..start + name_len,
        };
        let paths: Vec<PathBuf> = files.iter().map(|(path, _)| PathBuf::from(path)).collect();
        references(&target, &paths, &read)
            .into_iter()
            .map(|site| {
                let text = &texts[&site.path];
                let line = text[..site.range.start].matches('\n').count() + 1;
                let file = site
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                (file, line)
            })
            .collect()
    }

    #[test]
    fn skips_shadowed_and_member_uses() {
        let main = "stem = 1;\n\
                    glyph a {\n  advance = stem;\n}\n\
                    glyph b {\n  stem = 2;\n  advance = stem;\n}\n\
                    glyph c {\n  node stem2 = (0, 0) dir up pen pen(stem);\n  x = stem2.stem;\n}\n";
        let found = lines(&[(MAIN, main)], MAIN, "stem =");
        let main = |line| ("main.ductus".to_string(), line);
        assert_eq!(found, [main(1), main(3), main(10)]);
    }

    #[test]
    fn follows_imports_and_def_bodies() {
        let lib = "unit = 10;\ndef f(c) -> p {\n  p = c * unit;\n}\n";
        let main = "import \"lib.ductus\";\nx = unit + f(1);\n";
        let found = lines(&[(LIB, lib), (MAIN, main)], LIB, "unit =");
        let lib = |line| ("lib.ductus".to_string(), line);
        assert_eq!(found, [lib(1), lib(3), ("main.ductus".to_string(), 2)]);
    }

    #[test]
    fn def_calls() {
        let lib = "def f(c) -> p {\n  p = c;\n}\n";
        let main = "import \"lib.ductus\";\nx = f(1);\ny = f(2);\n";
        let found = lines(&[(LIB, lib), (MAIN, main)], LIB, "f(c)");
        let main = |line| ("main.ductus".to_string(), line);
        assert_eq!(found, [("lib.ductus".to_string(), 1), main(2), main(3)]);
    }

    #[test]
    fn files_that_do_not_import_are_ignored() {
        let lib = "unit = 10;\n";
        let other = "unit = 3;\nx = unit;\n";
        let found = lines(&[(LIB, lib), (MAIN, other)], LIB, "unit =");
        assert_eq!(found, [("lib.ductus".to_string(), 1)]);
    }
}
