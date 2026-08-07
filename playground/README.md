# ductus playground

A browser playground for **Ductus**, the parametric type design language:
an editor with live SVG preview, param sliders generated from the source's
`params` block, per-glyph selection, and a debug view showing skeleton
nodes, guides, metrics lines and control handles.

Everything runs client-side: `ductus-core` (parser, evaluator, renderer) is
compiled to WebAssembly.

## Running

`pkg/` (wasm) and `dist/` (bundled app) are generated, not committed — build
them once (see Development below), then serve statically:

```sh
python3 -m http.server 8613     # or any static file server
# open http://localhost:8613
```

## Development

```sh
bun install
bun run build           # bundle src/ -> dist/
bun run sync-wasm       # rebuild ../crates/ductus-wasm -> pkg/ (needs wasm-pack, clang)
bun run sync-examples   # copy ../../tree-sitter-ductus/examples -> examples/
```

Syntax colors come from `highlights.scm` in the `tree-sitter-ductus` crate,
which Cargo.lock pins to a git commit. To change the colors: push the query
change to the grammar repository, run `cargo update -p tree-sitter-ductus`,
then `bun run sync-wasm` and `bun run build`. For local grammar work, point
the dependency at your checkout with a `[patch]` entry in the workspace
`Cargo.toml`.

`sync-examples` requires a sibling checkout of `tree-sitter-ductus` next to
this repository.

## License

MIT
