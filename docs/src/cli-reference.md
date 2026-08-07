# CLI and file reference

## The ductus binary

```sh
ductus check file.ductus
```

This command is all about parsing. It quickly checks your `file.ductus` for syntax errors, pointing out the first one it finds with handy line and column numbers. If everything looks good, you'll simply see `OK`. Because it doesn't do any heavy lifting like evaluation or rendering, it's super fast — perfect for running every time you hit save.

```sh
ductus build file.ductus [--glyph NAME] [--set PARAM=VALUE ...] [--debug] [-o OUT.svg]
```

This command is where the magic happens: it takes your `file.ductus`, evaluates it, and renders your design directly to an SVG file. By default, it'll save it as `output.svg`.

- `--glyph NAME` — Want to focus on just one part of your design? Use this to render only a specific glyph. Without it, `ductus` will cheerfully render all your glyphs, arranging them neatly side by side in the order they appear in your source file. Each glyph's origin will snap right to a 100-unit grid. Oh, and any `draw` or `fill` output from your sketch will join the party on the same sheet, appearing at x = 0.
- `--set PARAM=VALUE` — This lets you pick a specific instance of your design by setting a parameter to a particular value. You can repeat this option for each parameter you want to adjust. Just a heads up: using an unknown parameter or providing a non-numeric value will trigger an error.
- `--debug` — Enable this for a peek behind the curtain at the construction view. You'll see helpful overlays like the grid, metrics guidelines, and your own guides. Plus, it reveals the skeleton nodes with their derived points, on-curve points, fitted handles, and anchors — all neatly labeled for clarity. Think of the [debugging chapter](debugging.md) as your trusty field guide for interpreting all this visual information.
- The generated SVG will be filled with `currentColor` and thoughtfully includes only a `viewBox`. This means it plays nicely with its surroundings, effortlessly inheriting its size and color from wherever you embed it. One more thing to note: while your source coordinates always work with y-up, the y-axis actually flips during the output stage to match standard SVG conventions.

```sh
ductus inspect file.ductus [--set PARAM=VALUE ...] [--glyph NAME]
```

Curious about the nitty-gritty details of your evaluated file? This command spills all the numeric beans, presenting every number as plain text. You'll see parameters complete with their ranges, metrics, and top-level bindings presented in source order. After that, it dives into each glyph's advance, its numeric bindings (yes, even your handy [probes](debugging.md#probing-numbers) are included), the point counts for each contour and guide, and all your anchor positions.

A quick note on imports: they always find their way by looking relative to your source file's directory. Speaking of handy tools, the online playground actually uses the very same evaluator (compiled right into WebAssembly). It comes equipped with neat sliders for your `params`, an inspector that mirrors what you get with `ductus inspect`, and a convenient debug toggle. However, do remember that it can't handle `import` statements — browsers, bless their hearts, just don't have access to a filesystem.

## A file, completely

Let's explore what a `ductus` source file can hold, diving into each level of its structure. For the truly authoritative reference, you'll find the normative grammar in the [specification](https://github.com/ductus-lang/tree-sitter-ductus/blob/master/Spec.md). Here, we'll give you a friendly overview of its general shape.

**Top level**, in any order. Just a friendly reminder, though: `ductus` works in a single pass, so make sure your names are bound before you try to use them:

| item | form |
|---|---|
| params | `params { name = default in min..max; ... }` |
| metrics | `metrics { name = expr; ... }` — These special names (`upm`, `x`, `cap`, `asc`, `desc`) carry particular meaning for the renderer. |
| pen | `pen name { width = expr; height = expr; angle = expr; }` |
| binding | `name = expr;` — Remember, it's single-assignment, and that rule applies everywhere. |
| node | `node name = point dir expr (width expr [nib expr] | pen name);` |
| macro | `def name(a, b) -> (x, y) { ... }` |
| glyph | `glyph name [: U+XXXX] { ... }` |
| import | `import "file.ductus";` — This is an include-once operation, resolves relatively, and will flag an error if it detects cycles. |
| loop | `for i in a..b { ... }` / `for x in list { ... }` |
| sketch | `draw(path);` `fill(path);` — You'll only find these in sketch files. |

Inside a **glyph body**, things get specific. You'll definitely need an `advance = expr;` (it's required to make your glyph complete). You can also define `contour name = path;` (and remember, contours must `cycle`), `guide name = path;`, and `anchor name = point;`. Beyond those, feel free to include any bindings, nodes, loops, or macro calls you need. Just keep in mind that `def`, `pen`, `params`, and `import` are exclusively for the top level.

A few general syntax notes: comments start with a `#` and continue all the way to the end of the line. Every statement wraps up with a semicolon `;`, and you'll be glad to know that newlines don't carry any special significance — use them freely for readability.

## Keywords and prelude

These words are special and are reserved for `ductus`'s internal use:

```text
params in glyph contour guide anchor node dir width nib pen cycle
corner def for if else and or not import shifted scaled xscaled
yscaled rotated slanted reversed metrics draw fill
```

The prelude thoughtfully sets up some handy direction constants for you: `right = 0`, `up = 90`, `left = 180`, and `down = 270`. These are treated just like ordinary numbers. It also introduces all the [builtins](builtins-reference.md). Just a friendly tip: you shouldn't try to reuse their names when defining your own `def`s.

## Errors, briefly

Nobody likes errors, but when they pop up, `ductus` makes sure you have all the context you need. Every error message comes complete with the file name, line, and column number. You might encounter syntax errors, which `check` and every other command will dutifully report. During evaluation, `ductus` will flag issues like unbound or rebound names, type mismatches, arity errors, unsatisfiable queries, and constructor arguments that are out of range. Then, during glyph validation, it watches out for open contours and any missing `advance` declarations. Finally, the renderer might give you a gentle *warning*: if it spots nested contours sharing the same winding direction, it's almost always a sign that a counter simply forgot to run backwards.