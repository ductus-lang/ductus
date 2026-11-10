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
- **Hover**: a name's declaration and its value for the default params,
  e.g. `stem = 40 + 0.14 * weight;` with **96**. Values are shown for
  numbers, params, metrics and, in glyphs, nodes (position, direction,
  width), contours, guides and anchors; points, pens and plain paths have
  none yet. While the file has an error, the values of the last
  evaluation without errors are shown, marked as such. Built-ins and
  `def`s show their signatures.
- **Inlay hints**: the value at the end of each computed numeric binding,
  `stem = 40 + 0.14 * weight;  ▸ 96`. Bindings written as a literal get
  none.
- **Completion** by context: the statements that can start where a
  statement starts (`contour`, `guide` and `anchor` only in a glyph);
  names in scope, functions and the direction constants inside an
  expression; fields after a `.` (`pos`, `left`, `tip`… on a node, `x`
  and `y` on a point). Calls complete with a placeholder per argument
  when the editor supports snippets.

- **Live preview** in the browser: the glyph under the cursor, rendered
  from the unsaved buffer as you type, with the construction (nodes,
  guides, handles) and a slider per param. See [Preview](#preview).

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
outline, `K` to hover, `grr` for references, `grn` to rename, `<C-s>` in
insert mode for signature help, and `]d` / `[d` for diagnostics. For the rest there
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

Inlay hints are off until enabled, for example in the same `LspAttach`
callback:

```lua
vim.lsp.inlay_hint.enable(true, { bufnr = args.buf })
```

To format on save:

```lua
vim.api.nvim_create_autocmd("BufWritePre", {
  pattern = "*.ductus",
  callback = function() vim.lsp.buf.format() end,
})
```

## Preview

The server can serve a page that shows the glyph under the cursor and
re-renders it on every change, convenient on a second monitor. It
listens on `http://127.0.0.1:45974/` ("glyph" on a phone keypad), or on
a free port when that one is taken, for example by a second editor. The
page reconnects by itself when the server restarts.

The page has a glyph selector (follow the cursor, one glyph, or all), a
construction toggle, and a slider per param; slider values apply to the
preview only. While the code has an error, the last rendering stays,
dimmed, under the error.

Neovim needs a command to open the page and an autocmd that tells the
server where the cursor is:

```lua
-- Live preview: :DuctusPreview opens it; the page follows the cursor.
local function ductus_client(bufnr)
  return vim.lsp.get_clients({ bufnr = bufnr, name = "ductus" })[1]
end

vim.api.nvim_create_user_command("DuctusPreview", function()
  local client = ductus_client(0)
  if not client then
    return
  end
  local position = vim.lsp.util.make_position_params(0, client.offset_encoding)
  local command = { title = "Preview", command = "ductus.preview", arguments = { position } }
  client:exec_cmd(command, { bufnr = 0 }, function(err, answer)
    if err then
      vim.notify(err.message, vim.log.levels.ERROR)
    elseif answer.viewers == 0 then
      vim.ui.open(answer.url)
    else
      vim.notify("Ductus preview: " .. answer.url)
    end
  end)
end, {})

vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI" }, {
  pattern = "*.ductus",
  callback = function(args)
    local client = ductus_client(args.buf)
    if client then
      client:notify("ductus/focus", vim.lsp.util.make_position_params(0, client.offset_encoding))
    end
  end,
})
```

`:DuctusPreview` opens the page, or only prints its address when a page
is already connected. To use another port, pass it when configuring the
server:

```lua
vim.lsp.config("ductus", {
  -- …
  init_options = { previewPort = 8080 },
})
```

The page is served to `127.0.0.1` only, and requests naming any other
host are refused.

## Timing

The server logs how long each check and each preview render takes. To see the messages, raise
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
