# chi

Reference implementation of **Chi**, a language for parametric type design.
Glyph outlines are written as explicit contours whose points are expressions
over named design parameters; a two-parameter spline solver places the curve
handles. The language is specified in the
[tree-sitter-chi](../tree-sitter-chi) repository's `Spec.md`.

- `chi-core` — library: parsing (tree-sitter), AST lowering, evaluation,
  curve fitting and SVG rendering. No CLI dependencies, so it can target
  WebAssembly for the playground.
- `chi-cli` — the `chi` binary.

## Usage

```sh
chi check glyphs.chi
chi build glyphs.chi -o out.svg
chi build glyphs.chi --glyph o --set weight=700 --debug -o o-bold.svg
```

`--debug` renders metrics guidelines, construction guides, skeleton nodes
with their derived edge points, anchors, on-curve points and control handles.

## Development

```sh
cargo test
cargo run --bin chi -- build ../tree-sitter-chi/examples/o.chi --debug
```

## License

MIT
