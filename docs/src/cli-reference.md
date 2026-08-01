# CLI and file reference

## The chi binary

```sh
chi check file.chi
```

Parse only. Reports the first syntax error with line and column; prints
`OK` otherwise. No evaluation, no rendering — cheap enough to run on
every save.

```sh
chi build file.chi [--glyph NAME] [--set PARAM=VALUE ...] [--debug] [-o OUT.svg]
```

Evaluate and render to SVG (default `output.svg`).

- `--glyph NAME` — render only that glyph; all glyphs otherwise, side
  by side in source order, each origin snapped to the 100-unit grid.
  Sketch `draw`/`fill` output shares the sheet, drawn at x = 0.
- `--set PARAM=VALUE` — select an instance; repeatable, one per param.
  Unknown params are an error, as is a non-numeric value.
- `--debug` — the construction view: grid, metrics guidelines, guides,
  skeleton nodes with derived points, on-curve points, fitted handles,
  anchors, all labeled. The [debugging chapter](debugging.md) is the
  field guide to reading it.
- The SVG is filled with `currentColor` and carries only a `viewBox`,
  so it inherits size and color from wherever it is embedded. The
  y-axis flip happens at output — source coordinates are always y-up.

```sh
chi inspect file.chi [--set PARAM=VALUE ...] [--glyph NAME]
```

Every number in the evaluated file, as text: params with ranges,
metrics, top-level bindings in source order, then each glyph's advance,
its numeric bindings ([probes](debugging.md#probing-numbers) included),
per-contour and per-guide point counts, and anchor positions.

Imports resolve relative to the source file's directory. The playground
runs the same evaluator (compiled to WebAssembly) with sliders for
`params`, an inspector matching `chi inspect`, and a debug toggle — but
rejects `import`, since the browser has no filesystem.

## A file, completely

Everything a chi source file may contain, at each level. The normative
grammar is the
[specification](https://gitlab.com/chi-lang/tree-sitter-chi/-/blob/main/Spec.md);
this is the shape of it.

**Top level**, in any order (single-pass: names must be bound before
use):

| item | form |
|---|---|
| params | `params { name = default in min..max; ... }` |
| metrics | `metrics { name = expr; ... }` — `upm x cap asc desc` mean something to the renderer |
| pen | `pen name { width = expr; height = expr; angle = expr; }` |
| binding | `name = expr;` — single-assignment, everywhere |
| node | `node name = point dir expr (width expr [nib expr] | pen name);` |
| macro | `def name(a, b) -> (x, y) { ... }` |
| glyph | `glyph name [: U+XXXX] { ... }` |
| import | `import "file.chi";` — include-once, relative, cycles error |
| loop | `for i in a..b { ... }` / `for x in list { ... }` |
| sketch | `draw(path);` `fill(path);` — sketch files only |

**Glyph body**: `advance = expr;` (required), `contour name = path;`
(must `cycle`), `guide name = path;`, `anchor name = point;`, plus any
bindings, nodes, loops and macro calls — but not `def`, `pen`, `params`
or `import`, which are top-level only.

**Comments** run from `#` to end of line; statements end with `;`;
newlines are insignificant.

## Keywords and prelude

Reserved words:

```text
params in glyph contour guide anchor node dir width nib pen cycle
corner def for if else and or not import shifted scaled xscaled
yscaled rotated slanted reversed metrics draw fill
```

The prelude binds the direction constants — `right = 0`, `up = 90`,
`left = 180`, `down = 270` — as ordinary numbers, and the
[builtins](builtins-reference.md), whose names a `def` may not reuse.

## Errors, briefly

Everything is reported with file, line and column: syntax errors from
`check` and every command; unbound or rebound names, type mismatches,
arity errors, unsatisfiable queries and out-of-range constructor
arguments from evaluation; open contours and missing `advance` from
glyph validation. The renderer adds one *warning*: nested contours that
share a winding direction, which is almost always a counter that
forgot to run backwards.
