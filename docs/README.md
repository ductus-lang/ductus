# The Chi Book

Tutorial documentation for chi, built with [mdBook](https://rust-lang.github.io/mdBook/)
and the `chi-docs` preprocessor from this workspace.

```sh
cargo install mdbook   # once
make build             # or: make serve, for live reload at :3000
```

## Writing examples

Every fenced ` ```chi ` block is compiled by `chi-docs` (via chi-core) at
book-build time and replaced by a figure: highlighted source beside the
rendered SVG. A block that fails to compile fails the build, with the
chapter file and line in the message.

Options after `chi` in the fence's info string:

| option | effect |
|---|---|
| `glyph=o` | render only this glyph |
| `set="weight=700 contrast=0.8"` | param overrides for this instance |
| `debug` | render guides, nodes, metrics lines and handles |
| `norender` | show the code only, skip compiling (fragments) |
| `hide` | append to the chapter's hidden preamble; show nothing |
| `acc` | show the block *and* append it to the preamble |

`hide` and `acc` blocks accumulate per chapter, in order, and every other
block compiles as preamble + block — so a chapter can bind params and
metrics once and keep each visible example small. Mind what accumulates:
a glyph (or a sketch `draw`/`fill`) in the preamble renders in **every**
later figure of the chapter — order standalone sketch sections before an
accumulated glyph, or their sheets overlap. `norender acc` shows
setup code (a `params` block, say) without rendering it. An **empty**
visible block renders the preamble alone: an output-only figure, used to
show another instance (`set=`) or view (`debug`) of what the chapter
already built.

Figure layout and the syntax palette live in `chi.css`; the highlight
classes come from `tree-sitter-chi/queries/highlights.scm`, the same
query the playground uses.

## Notes

- `book.toml` invokes the preprocessor as `cargo run -q -p chi-docs --`.
  The trailing `--` matters: mdBook appends `supports html` to the
  command, and mdBook silently skips a preprocessor whose supports-check
  fails.
- mdBook 0.5 renamed the book JSON's `sections` to `items`; chi-docs
  accepts both.
