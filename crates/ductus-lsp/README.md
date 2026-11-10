# ductus-lsp

A language server for Ductus. It runs as a subcommand of the `ductus`
binary and talks the Language Server Protocol over stdin and stdout:

```sh
ductus lsp
```

## Features

- **Diagnostics** on every change: all syntax errors, then the first
  lowering or evaluation error (undefined or rebound names, type errors,
  open contours, unreadable imports). Evaluation uses each param's default
  value.
- **Formatting** with the same formatter as `ductus format`. A file with
  syntax errors is left untouched.
- **Document symbols**: params, metrics, defs, glyphs with their
  contours, guides, anchors, nodes and bindings, and top-level bindings.
- **Go to definition** for names, `def` calls, `def` return names, and
  the path in an `import` statement. Follows imports into other files.
- **Find references** and **rename**, across every `.ductus` file under
  the workspace folder (hidden directories, `target/` and
  `node_modules/` are skipped), or, without one, the directories of the
  open files. A rename that would change what any name refers to, or
  bind a name twice in one scope, is refused with the place it would
  break.
- **Workspace symbols**: params, metrics, defs, glyphs and top-level
  bindings of those same files, matched by subsequence (`bnp` finds
  `broad_nib_pen`).
- **Signature help** for built-in functions and `def`s, with the
  argument under the cursor highlighted. A `def`'s description is the
  comment directly above it.
- **Completion** by context: the statements that can start where a
  statement starts (`contour`, `guide` and `anchor` only in a glyph);
  names in scope, functions and the direction constants inside an
  expression; fields after a `.` (`pos`, `left`, `tip`… on a node, `x`
  and `y` on a point). Calls complete with a placeholder per argument
  when the editor supports snippets.

Imports read unsaved text from open buffers before the file on disk.

## Install

```sh
cargo install --path crates/ductus-cli
```

## Neovim

Neovim 0.11 or later. In `init.lua`:

```lua
vim.filetype.add({ extension = { ductus = "ductus" } })

vim.lsp.config("ductus", {
  cmd = { "ductus", "lsp" },
  filetypes = { "ductus" },
  root_markers = { ".git" },
})
vim.lsp.enable("ductus")
```

With that, `:checkhealth vim.lsp` lists the server for a `.ductus`
buffer. Neovim's default mappings then work: `gO` for the document
outline, `grr` for references, `grn` to rename, `<C-s>` in insert mode
for signature help, and `]d` / `[d` for diagnostics. For the rest there
are `vim.lsp.buf.definition()`, `vim.lsp.buf.workspace_symbol()` and
`vim.lsp.buf.format()`. The workspace folder is the directory holding
`.git`, as set by `root_markers`.

Completion works through `<C-x><C-o>` once the server attaches. To have
it pop up while typing, and calls expand to argument placeholders:

```lua
vim.api.nvim_create_autocmd("LspAttach", {
  callback = function(args)
    vim.lsp.completion.enable(true, args.data.client_id, args.buf, { autotrigger = true })
  end,
})
```

To format on save:

```lua
vim.api.nvim_create_autocmd("BufWritePre", {
  pattern = "*.ductus",
  callback = function() vim.lsp.buf.format() end,
})
```

## Timing

The server logs how long each check takes. To see the messages, raise
the log level and open the log:

```lua
vim.lsp.log.set_level("info")
```

```vim
:LspLog
```

## Limitations

- Evaluation stops at its first error, so a file shows at most one
  evaluation error at a time.
- An error inside an imported file is reported on the `import` line.
- A file is not re-checked when a file it imports changes; edit it to
  re-check.
- `file:` URIs with Unix paths only.
