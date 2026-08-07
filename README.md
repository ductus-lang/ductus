# ductus

Reference implementation of **Ductus**, a language for parametric type design.
Glyph outlines are written as explicit contours whose points are expressions
over named design parameters; a two-parameter spline solver places the curve
handles. The language is specified in the
[tree-sitter-ductus](../tree-sitter-ductus) repository's `Spec.md`.

- `curve-fitter` — the two-parameter spline solver (Raph Levien's method).
- `ductus-core` — library: parsing (tree-sitter), AST lowering, evaluation,
  curve fitting and SVG rendering. No CLI dependencies, so it can target
  WebAssembly for the playground.
- `ductus-cli` — the `ductus` binary.

## Usage

```sh
ductus check glyphs.ductus
ductus build glyphs.ductus -o out.svg
ductus build glyphs.ductus --glyph o --set weight=700 --debug -o o-bold.svg
```

`--debug` renders metrics guidelines, construction guides, skeleton nodes
with their derived edge points, anchors, on-curve points and control handles.

## Development

```sh
cargo test
cargo run --bin ductus -- build ../tree-sitter-ductus/examples/o.ductus --debug
```

## License

MIT
