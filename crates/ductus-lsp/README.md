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
outline, `]d` / `[d` for diagnostics, and `vim.lsp.buf.definition()` and
`vim.lsp.buf.format()` for the rest. To format on save:

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
