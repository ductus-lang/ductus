//! End-to-end: `chi build` resolves imports relative to the importing file.

use std::process::Command;

#[test]
fn build_resolves_nested_relative_imports() {
    let dir = std::env::temp_dir().join(format!("chi-import-test-{}", std::process::id()));
    let shared = dir.join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    // main.chi -> shared/measures.chi -> extra.chi (relative to shared/).
    std::fs::write(shared.join("extra.chi"), "base = 50;\n").unwrap();
    std::fs::write(
        shared.join("measures.chi"),
        "import \"extra.chi\";\nstem = 2 base;\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.chi"),
        "import \"shared/measures.chi\";\n\
         glyph g { advance = stem; contour c = (0,0) -- (stem, 0) -- (stem, 10) -- cycle; }\n",
    )
    .unwrap();

    let out = dir.join("out.svg");
    let output = Command::new(env!("CARGO_BIN_EXE_chi"))
        .arg("build")
        .arg(dir.join("main.chi"))
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out.exists());

    std::fs::remove_dir_all(&dir).ok();
}
