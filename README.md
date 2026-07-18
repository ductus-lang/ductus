# chi playground

A browser playground for **Chi**, the parametric type design language:
an editor with live SVG preview, param sliders generated from the source's
`params` block, per-glyph selection, and a debug view showing skeleton
nodes, guides, metrics lines and control handles.

Everything runs client-side: `chi-core` (parser, evaluator, renderer) is
compiled to WebAssembly.

## Running

The repository is self-contained — `pkg/` (wasm) and `dist/` (bundled app)
are committed:

```sh
python3 -m http.server 8613     # or any static file server
# open http://localhost:8613
```

## Development

```sh
bun install
bun run build           # bundle src/ -> dist/
bun run sync-wasm       # rebuild ../chi/chi-wasm -> pkg/ (needs wasm-pack, clang)
bun run sync-examples   # copy ../tree-sitter-chi/examples -> examples/
```

Requires the sibling checkouts `../chi` and `../tree-sitter-chi`.

## License

MIT
