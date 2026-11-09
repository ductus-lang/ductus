//! End-to-end rendering tests: source -> document -> SVG.

use ductus_core::doc::Document;
use ductus_core::eval::{NoImporter, evaluate};
use ductus_core::lower::lower;
use ductus_core::parse::parse;
use ductus_core::render::{RenderOptions, render};
use std::collections::HashMap;

fn build(source: &str) -> Document {
    let tree = parse(source).expect("parses");
    let program = lower(&tree, source).expect("lowers");
    evaluate(&program, &HashMap::new(), &NoImporter).expect("evaluates")
}

const O_SOURCE: &str = r#"
params { weight = 400 in 100..900; contrast = 0.85 in 0.6..1.0; }
metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; over = 10 + weight / 200; }
stem = 40 + 0.14 * weight;
hair = contrast * stem;
def bowl(c, rx, ry) -> (n, w, s, e) {
  node n = c + (0,  ry) dir left  pen pen(hair);
  node w = c + (-rx, 0) dir down  pen pen(stem);
  node s = c + (0, -ry) dir right pen pen(hair);
  node e = c + (rx, 0)  dir up    pen pen(stem);
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
fn renders_the_o_glyph() {
    let document = build(O_SOURCE);
    let svg = render(&document, &RenderOptions::default()).unwrap();

    assert!(svg.starts_with("<svg"), "starts with <svg");
    assert!(svg.contains("viewBox"));
    assert!(svg.contains("glyph-o"));
    assert!(svg.contains("fill-rule=\"nonzero\""));
    // Outer and inner contour in one path element: two subpaths.
    let d_starts = svg.matches("M").count();
    assert!(d_starts >= 2, "expected two subpaths, svg: {svg}");
    // No debug artifacts by default.
    assert!(!svg.contains("class=\"debug\""));
}

#[test]
fn debug_mode_adds_guides_nodes_and_handles() {
    let document = build(O_SOURCE);
    let options = RenderOptions {
        glyph: None,
        debug: true,
    };
    let svg = render(&document, &options).unwrap();
    assert!(svg.contains("class=\"debug\""));
    assert!(svg.contains("stroke-dasharray"), "guide is dashed");
    assert!(svg.contains("<circle"), "node and control markers");
    assert!(svg.contains("<text"), "node labels");
    // Node names from the bowl def.
    for name in ["n", "w", "s", "e"] {
        assert!(svg.contains(&format!(">{name}</text>")), "label for node {name}");
    }
}

#[test]
fn selecting_a_missing_glyph_is_an_error() {
    let document = build(O_SOURCE);
    let options = RenderOptions {
        glyph: Some("q".to_string()),
        debug: false,
    };
    let error = render(&document, &options).unwrap_err();
    assert!(error.message.contains("q"));
}

#[test]
fn renders_a_sketch() {
    let document = build("draw((0, 0) {45} .. (300, 400) .. {down} (500, 300));\nfill((0,0) -- (100,0) -- (50,80) -- cycle);\n");
    let svg = render(&document, &RenderOptions::default()).unwrap();
    assert!(svg.contains("stroke=\"currentColor\""), "draw() strokes the spine");
    assert!(svg.contains("fill=\"currentColor\""), "fill() fills");
}

#[test]
fn line_only_glyph_renders() {
    let source = r#"
metrics { upm = 1000; asc = 750; desc = -220; }
glyph l {
  advance = 230;
  contour main = (20, 0) -- (210, 0) -- (210, 750) -- (20, 750) -- cycle;
}
"#;
    let document = build(source);
    let svg = render(&document, &RenderOptions::default()).unwrap();
    assert!(svg.contains("glyph-l"));
}
