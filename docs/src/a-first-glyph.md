# A first glyph

A sketch draws whatever you like, wherever you like. A glyph is a promise:
it has a name, a width, and closed contours that a font renderer will fill.
This chapter builds the simplest glyph there is — a sans-serif `l`, one
rectangle — and then makes it parametric, which is the whole point of
writing it in chi.

## A complete file

```chi
metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; }

glyph l : U+006C {
  advance = 250;
  stem = 60;
  lsb = (advance - stem) / 2;

  contour main =
      (lsb, 0) -- (lsb + stem, 0)
   -- (lsb + stem, asc) -- (lsb, asc) -- cycle;
}
```

Piece by piece:

- **`metrics`** names the font-level measures. Five keys mean something to
  the renderer — `upm` (units per em), `x` (x-height), `cap` (cap height),
  `asc` (ascender) and `desc` (descender, negative) size the sheet and draw
  the guidelines in debug view. Any other key is simply a named number
  available to all following code.
- **`glyph l : U+006C { ... }`** opens a glyph named `l`, encoded at
  U+006C. The codepoint is optional — ligatures and other unencoded glyphs
  just omit it — and a name that isn't a valid identifier is written as a
  string: `glyph "ka.mlym" : U+0D15`.
- **`advance`** is the one required binding in every glyph: its horizontal
  advance width in font units. Everything else in a glyph body is ordinary
  code — `stem` and `lsb` here are local bindings, and the origin sits at
  the left sidebearing point on the baseline, so `lsb` is where ink
  starts.
- **`contour`** binds a filled contour. Its path **must** end in `cycle`;
  an open contour is an error, not a warning, because there is no sensible
  way to fill one.

One convention to start observing now: this contour runs
**counterclockwise** — bottom edge rightward, up the right side, back along
the top. Chi fills with the nonzero winding rule, so outer contours go
counterclockwise and counters (the hole in an `o`) go clockwise. With a
single rectangle nothing yet depends on it; from the chapter on nodes
onward, everything does.

## Building it

The `chi` binary checks and renders source files:

```sh
chi check l.chi                 # parse errors only, no rendering
chi build l.chi -o l.svg        # every glyph, one sheet
chi build l.chi --glyph l --debug -o l-debug.svg
```

`--debug` turns on the construction view — the same rendering used
throughout this book: metrics guidelines, the grid, on-curve points, and
the fitted handles.

## Making it parametric

So far the `l` is a drawing with extra steps. The parametric version
starts by declaring what varies — a `params` block gives each design axis
a default and a range — and deriving the stem from it:

```chi norender acc
params { weight = 400 in 100..900; }

metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; }

stem = 40 + 0.14 * weight;
```

`weight` is now a design axis: this file describes every value from 100
to 900 at once, and an *instance* is selected from outside the source —
`chi build l.chi --set weight=700`, or a slider in the playground.
`stem` is an ordinary binding that happens to depend on it.

The glyph itself barely changes — `stem` moves out to the file level, and
the advance grows with it so the sidebearings hold:

```chi acc
glyph l : U+006C {
  advance = stem + 190;
  lsb = (advance - stem) / 2;

  contour main =
      (lsb, 0) -- (lsb + stem, 0)
   -- (lsb + stem, asc) -- (lsb, asc) -- cycle;
}
```

That render used the default, `weight = 400`. The same source at
`--set weight=150`:

```chi set="weight=150"
```

and at `--set weight=900`:

```chi set="weight=900"
```

Same file, same four points, same topology — only the numbers moved. That
invariance is chi's interpolation-compatibility rule at work, and it is
what will let a designspace be sampled from one source.

Finally, the debug view of the default instance, now with something to
show: the metrics lines from the `metrics` block, the baseline, and the
four on-curve points of the contour:

```chi debug
```

A rectangle exercises none of chi's curve machinery — no `..` joints, no
directions, no nodes. The next part of the book works through the language
those need: expressions and mediation, paths and the curve fitter, and the
parameter system this chapter only touched.
