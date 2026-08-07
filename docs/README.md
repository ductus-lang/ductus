# The Ductus Book

Tutorial documentation for ductus, built with [mdBook](https://rust-lang.github.io/mdBook/)
and the `ductus-docs` preprocessor from this workspace.

```sh
cargo install mdbook   # once
make build             # or: make serve, for live reload at :3000
```

## Writing examples

Every fenced ` ```ductus ` block is compiled by `ductus-docs` (via ductus-core) at
book-build time and replaced by a figure: highlighted source beside the
rendered SVG. A block that fails to compile fails the build, with the
chapter file and line in the message.

Options after `ductus` in the fence's info string:

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

Figure layout and the syntax palette live in `ductus.css`; the highlight
classes come from `tree-sitter-ductus/queries/highlights.scm`, the same
query the playground uses.

## Voice

Written for a colleague at the next desk, not an audience in a hall.
The habits that keep it that way:

- **You** draws things; **we** figure things out together. The book may
  have opinions, and should say so plainly when something is fiddly,
  surprising, or was learned the hard way.
- One landing per chapter. If every section ends on a quotable maxim,
  none of them land. Most paragraphs should just end.
- Concrete over abstract: a pen, a slider, a coordinate — not "the
  machinery", "the discipline", "the promise".
- Vary the rhythm. Short flat sentences are allowed. So are asides.
- A few `> **Try it**` blockquotes per part, pointing at the
  playground, only where an experiment genuinely teaches something the
  prose can't.
- Reference chapters are exempt: reference should be dry.

## Notes

- `book.toml` invokes the preprocessor as `cargo run -q -p ductus-docs --`.
  The trailing `--` matters: mdBook appends `supports html` to the
  command, and mdBook silently skips a preprocessor whose supports-check
  fails.
- mdBook 0.5 renamed the book JSON's `sections` to `items`; ductus-docs
  accepts both.
