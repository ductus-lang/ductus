//! Document-level integration tests: parse -> lower -> evaluate.

use chi_core::doc::Document;
use chi_core::eval::{EvalError, Importer, NoImporter, evaluate};
use chi_core::lower::lower;
use chi_core::parse::parse;
use std::collections::HashMap;

fn build(source: &str) -> Document {
    build_with(source, &HashMap::new()).expect("source should evaluate")
}

fn build_with(source: &str, overrides: &HashMap<String, f64>) -> Result<Document, EvalError> {
    let tree = parse(source).expect("source should parse");
    let program = lower(&tree, source).expect("source should lower");
    evaluate(&program, overrides, &NoImporter)
}

fn build_err(source: &str) -> EvalError {
    build_with(source, &HashMap::new()).expect_err("source should fail")
}

const O_SOURCE: &str = r#"
params {
  weight   = 400 in 100..900;
  contrast = 0.85 in 0.6..1.0;
}
metrics {
  upm = 1000; x = 500; cap = 700; asc = 750; desc = -220;
  over = 10 + weight / 200;
}

stem = 40 + 0.14 * weight;
hair = contrast * stem;

def bowl(c, rx, ry) -> (n, w, s, e) {
  node n = c + (0,  ry) dir left  width hair;
  node w = c + (-rx, 0) dir down  width stem;
  node s = c + (0, -ry) dir right width hair;
  node e = c + (rx, 0)  dir up    width stem;
}

glyph o : U+006F {
  advance = 2 stem + 380;
  center = (advance / 2, x / 2);

  ry = x / 2 + over - hair / 2;
  rx = advance / 2 - 55 - stem / 2;

  (n, w, s, e) = bowl(center, rx, ry);
  guide spine = n.pos .. w.pos .. s.pos .. e.pos .. cycle;

  contour outer = n.right .. w.right .. s.right .. e.right .. cycle;
  contour inner = n.left  .. e.left  .. s.left  .. w.left  .. cycle;

  anchor top = (advance / 2, x + over);
}
"#;

#[test]
fn evaluates_the_o_example() {
    let document = build(O_SOURCE);

    assert_eq!(document.params.len(), 2);
    assert_eq!(document.params[0].name, "weight");
    assert_eq!(document.params[0].value, 400.0);
    assert_eq!(document.metrics.get("x"), Some(500.0));
    assert_eq!(document.metrics.upm(), 1000.0);

    assert_eq!(document.glyphs.len(), 1);
    let glyph = &document.glyphs[0];
    assert_eq!(glyph.name, "o");
    assert_eq!(glyph.unicode, Some(0x6F));

    // stem = 40 + 0.14*400 = 96; advance = 2*96 + 380 = 572
    assert!((glyph.advance - 572.0).abs() < 1e-9, "advance = {}", glyph.advance);

    assert_eq!(glyph.contours.len(), 2);
    assert!(glyph.contours.iter().all(|(_, path)| path.closed));
    assert_eq!(glyph.contours[0].1.points.len(), 4);
    assert_eq!(glyph.guides.len(), 1);
    assert_eq!(glyph.anchors.len(), 1);
    assert_eq!(glyph.nodes.len(), 4, "bowl nodes are visible in the glyph scope");

    // Topmost outer point: n.right with dir=left (nib 270) is
    // n.pos - hair/2 * unitvec(270) = n.pos + (0, hair/2), and the skeleton
    // radius keeps the outline extreme at x + over.
    let outer = &glyph.contours[0].1;
    let top = outer.points[0].pos;
    let over = 10.0 + 400.0 / 200.0;
    assert!((top.y - (500.0 + over)).abs() < 1e-9, "outer top = {}", top.y);
}

#[test]
fn param_overrides_change_derived_geometry() {
    let overrides = HashMap::from([("weight".to_string(), 900.0)]);
    let document = build_with(O_SOURCE, &overrides).unwrap();
    // stem = 40 + 0.14*900 = 166; advance = 2*166 + 380 = 712
    assert!((document.glyphs[0].advance - 712.0).abs() < 1e-9);
    // Same topology as the default instance.
    assert_eq!(document.glyphs[0].contours[0].1.points.len(), 4);
}

#[test]
fn out_of_range_override_is_an_error() {
    let overrides = HashMap::from([("weight".to_string(), 5000.0)]);
    let error = build_with(O_SOURCE, &overrides).unwrap_err();
    assert!(error.message.contains("range"), "{}", error.message);
}

#[test]
fn unknown_override_is_an_error() {
    let overrides = HashMap::from([("wdth".to_string(), 100.0)]);
    let error = build_with(O_SOURCE, &overrides).unwrap_err();
    assert!(error.message.contains("wdth"), "{}", error.message);
}

#[test]
fn def_with_single_return() {
    let document = build(
        "def double(a) -> b { b = 2 * a; }\nx = double(21);\nglyph g { advance = x; }\n",
    );
    assert_eq!(document.glyphs[0].advance, 42.0);
}

#[test]
fn def_body_cannot_see_glyph_locals() {
    let error = build_err(
        "def f() -> a { a = hidden + 1; }\nglyph g { hidden = 5; advance = f(); }\n",
    );
    assert!(error.message.contains("hidden"), "{}", error.message);
}

#[test]
fn recursion_is_rejected() {
    let error = build_err("def f(a) -> b { b = f(a); }\nx = f(1);\n");
    assert!(error.message.contains("recursive"), "{}", error.message);
}

#[test]
fn open_contour_is_an_error() {
    let error = build_err("glyph g { advance = 10; contour c = (0,0) -- (1,1); }\n");
    assert!(error.message.contains("closed"), "{}", error.message);
}

#[test]
fn glyph_without_advance_is_an_error() {
    let error = build_err("glyph g { x = 1; }\n");
    assert!(error.message.contains("advance"), "{}", error.message);
}

#[test]
fn duplicate_glyph_is_an_error() {
    let error = build_err("glyph g { advance = 1; }\nglyph g { advance = 2; }\n");
    assert!(error.message.contains("already defined"), "{}", error.message);
}

#[test]
fn sketch_draw_and_fill() {
    let document = build("draw((0,0) .. (10, 10));\nfill((0,0) -- (10,0) -- (5,5) -- cycle);\n");
    assert_eq!(document.sketch.len(), 2);
}

#[test]
fn fill_requires_closed_path() {
    let error = build_err("fill((0,0) -- (10,0));\n");
    assert!(error.message.contains("closed"), "{}", error.message);
}

#[test]
fn draw_inside_glyph_is_an_error() {
    let error = build_err("glyph g { advance = 1; draw((0,0) .. (1,1)); }\n");
    assert!(error.message.contains("contour"), "{}", error.message);
}

#[test]
fn glyph_locals_do_not_leak() {
    // 'center' is glyph-local: a later glyph may reuse the name, and the top
    // level may bind it afterwards without an immutability clash.
    let document = build(
        "glyph a { advance = 1; center = 5; }\nglyph b { advance = 2; center = 6; }\ncenter = 7;\n",
    );
    assert_eq!(document.glyphs.len(), 2);
}

// ---- import ----------------------------------------------------------------

/// In-memory importer: path -> source, canonical id = the path itself.
struct MapImporter(HashMap<&'static str, &'static str>);

impl Importer for MapImporter {
    fn load(&self, path: &str, _from: Option<&str>) -> Result<(String, String), String> {
        self.0
            .get(path)
            .map(|source| (path.to_string(), source.to_string()))
            .ok_or_else(|| format!("no such file: {path}"))
    }
}

fn build_with_files(
    source: &str,
    files: &[(&'static str, &'static str)],
) -> Result<Document, EvalError> {
    let importer = MapImporter(files.iter().copied().collect());
    let tree = parse(source).expect("source should parse");
    let program = lower(&tree, source).expect("source should lower");
    evaluate(&program, &HashMap::new(), &importer)
}

#[test]
fn import_shares_measures_params_and_defs() {
    let document = build_with_files(
        "import \"measures.chi\";\nglyph g { advance = stem + pad(10); }\n",
        &[(
            "measures.chi",
            "params { weight = 400 in 100..900; }\nstem = weight / 4;\ndef pad(k) -> r { r = 2 k; }\n",
        )],
    )
    .expect("evaluates");
    assert_eq!(document.glyphs[0].advance, 120.0);
    // Imported params register on the document (playground sliders).
    assert_eq!(document.params.len(), 1);
    assert_eq!(document.params[0].name, "weight");
}

#[test]
fn import_is_include_once() {
    // Diamond: both imports pull in measures.chi; the second is a no-op
    // rather than an immutable-rebinding error.
    let document = build_with_files(
        "import \"a.chi\";\nimport \"b.chi\";\nglyph g { advance = stem + a_pad + b_pad; }\n",
        &[
            ("a.chi", "import \"measures.chi\";\na_pad = stem;\n"),
            ("b.chi", "import \"measures.chi\";\nb_pad = 2 stem;\n"),
            ("measures.chi", "stem = 100;\n"),
        ],
    )
    .expect("evaluates");
    assert_eq!(document.glyphs[0].advance, 400.0);
}

#[test]
fn import_cycle_is_an_error() {
    let error = build_with_files(
        "import \"a.chi\";\n",
        &[
            ("a.chi", "import \"b.chi\";\n"),
            ("b.chi", "import \"a.chi\";\n"),
        ],
    )
    .expect_err("cycle should fail");
    assert!(error.message.contains("cycle"), "{}", error.message);
    assert!(error.message.contains("a.chi"), "{}", error.message);
}

#[test]
fn error_in_imported_file_carries_the_file_name() {
    let error = build_with_files(
        "import \"broken.chi\";\n",
        &[("broken.chi", "x = missing;\n")],
    )
    .expect_err("should fail");
    assert!(error.message.contains("broken.chi"), "{}", error.message);
    assert!(error.message.contains("missing"), "{}", error.message);
}

#[test]
fn import_without_an_importer_is_an_error() {
    let error = build_err("import \"other.chi\";\n");
    assert!(error.message.contains("not supported"), "{}", error.message);
}

#[test]
fn guide_and_contour_names_are_bindings() {
    // A guide can be referenced (and queried) by later code in the glyph.
    let document = build(
        "glyph g {\n\
           advance = 100;\n\
           guide skel = (0, 0) -- (100, 50);\n\
           tip = point(skel, 1);\n\
           anchor end = tip;\n\
         }\n",
    );
    assert_eq!(document.glyphs[0].anchors[0].1, kurbo::Point::new(100.0, 50.0));
}

#[test]
fn metrics_are_usable_in_following_code() {
    let document = build("metrics { x = 500; }\nglyph g { advance = x * 2; }\n");
    assert_eq!(document.glyphs[0].advance, 1000.0);
}

#[test]
fn builtin_cannot_be_redefined_as_def() {
    let error = build_err("def min(a) -> b { b = a; }\n");
    assert!(error.message.contains("built-in"), "{}", error.message);
}
