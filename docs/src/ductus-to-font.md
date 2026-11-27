# From Ductus to a compiled font

> **Draft:** This chapter outlines the workflow for turning `.ductus` sources into a compiled OpenType/variable font using UFOs, a `.designspace` file, and `fontc`. It will be revised as we build out the Kali font project.

## Overview

`ductus` is a parametric geometry language: it defines `params`, `metrics`, `glyph` outlines, `advance` widths, and `anchor`s. Compiling a shippable font also requires packaging metadata (names, copyright, OS/2 flags, `STAT` axis records), OpenType layout rules (`GSUB`/`GPOS` features, kerning), and a `.designspace` that connects multiple masters.

Rather than bloating `.ductus` with font-packaging syntax, `ductus` handles the geometry and exports UFOs directly via `ductus-ufo` (or `ductus ufo`), while a font project combines those master UFOs with its metadata, features, and `.designspace` and compiles them with `fontc`.

## Direct `Document` → UFO export

Because `ductus` already evaluates outlines in font coordinates (Y-up, baseline at `0`) and fits them to cubic Béziers, exporting to UFO does not go through SVG:

- `metrics` (`upm`, `asc`, `desc`, `x`, `cap`) populate `fontinfo.plist`.
- Each `glyph` becomes a `.glif` with `width = advance`, Unicode codepoints, `anchor`s, and `contour`s.
- Fitted cubic segments map to `norad::Contour` points, preserving `smooth` and corner points.

From the CLI, you can export any `.ductus` file to a UFO:

```sh
ductus ufo sources/kali.ductus --family Kali --style Regular -o sources/ufo/Kali-Regular.ufo
```

Or evaluate different masters by overriding `params`:

```sh
ductus ufo sources/kali.ductus --set pen_height=40  --family Kali --style Thin  -o sources/ufo/Kali-Thin.ufo
ductus ufo sources/kali.ductus --set pen_height=150 --family Kali --style Bold  -o sources/ufo/Kali-Bold.ufo
```

## Multi-master workflow with `fontc`

For a full font project, a dedicated Rust builder (depending on `ductus-core` and `ductus-ufo`) or a `Makefile` ties the pieces together:

1. **Master UFOs**: Evaluate the `.ductus` entry file at each master's parameter values (for example, mapping `pen_height` to the `wght` axis) and write the master UFOs.
2. **Font metadata & features**: Populate `fontinfo.plist`, `groups.plist`, `kerning.plist`, and `features.fea` from the project's configuration.
3. **Designspace**: Connect the master UFOs in a `.designspace` file defining the axes, sources, and named instances.
4. **Compile**: Run `fontc` on the `.designspace` to produce the variable or static TTF:

```sh
fontc -o fonts/Kali-variable.ttf Kali.designspace
```

5. **Post-process & test**: Add `STAT`/`prep` tables if needed, compress to WOFF2, and verify with `hb-view` proofs and `fontspector`.
