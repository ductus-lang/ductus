# Glyph organization

A glyph so far has been a name and some contours. Building a *font's*
worth of them raises the organizational questions: what may a glyph be
called, what belongs inside it, and how does a family of files share a
design. Chi's answers are all things you have already met, arranged.

## Names and codepoints

```chi norender
glyph o : U+006F { ... }          # encoded, identifier name
glyph "ka.mlym" : U+0D15 { ... }  # names that aren't identifiers
glyph "t_t" { ... }               # unencoded: ligatures, alternates
```

The codepoint is optional — a ligature or a stylistic alternate simply
omits it — and a string name covers everything an identifier can't say,
like the `script`-suffixed conventions common in complex-script fonts.
One glyph, one block; the sheet renders them in source order.

## What lives in a glyph body

Four statement kinds carry meaning to the renderer, everything else is
ordinary code:

- `advance = expr;` — required, the horizontal advance.
- `contour name = path;` — filled ink; must end in `cycle`.
- `guide name = path;` — construction geometry: skeletons, diagonals,
  alignment lines. Never filled, drawn only in the debug view, and —
  because guide names are ordinary path bindings — queryable with
  `directionpoint` and friends. A guide is documentation that the
  compiler checks and the tooling draws.
- `anchor name = point;` — a named position for mark attachment:
  where an accent sits over this glyph, where a matra clips on. Chi
  itself only records and displays them (debug view, `chi inspect`);
  attachment logic belongs to the font compiler downstream.

## Sharing a design: one file, then several

Related glyphs in one file share everything above them — that is just
scoping. The playground's `latin_set.chi` builds an entire broad-nib
lowercase in a single file this way: one `params` block, one pen,
glyph after glyph.

When one file becomes unwieldy, `import` (from the language chapters)
is the organizational tool: put the axes, metrics, stroke weights and
construction macros in a foundation file, and let each glyph file — or
a whole-family file — import it. This chapter's example foundation:

```chi norender
# examples/family.chi
params { weight = 400 in 100..900; }

metrics {
  upm = 1000; x = 500; cap = 700; asc = 750; desc = -220;
  over = 10 + weight / 200;
}

stem = 40 + 0.14 * weight;
hair = 0.85 * stem;

def bowl(c, rx, ry) -> (n, w, s, e) { ... }
```

Two glyphs built on it — and note how much of each glyph is *not* here,
because the foundation carries it:

```chi acc
import "examples/family.chi";

glyph o : U+006F {
  advance = 2 stem + 380;
  center = (advance / 2, x / 2);
  ry = x / 2 + over - hair / 2;
  rx = advance / 2 - 55 - stem / 2;

  (n, w, s, e) = bowl(center, rx, ry);
  guide spine = n.pos .. w.pos .. s.pos .. e.pos .. cycle;

  contour outer = n.right .. w.right .. s.right .. e.right .. cycle;
  contour inner = n.left  .. e.left  .. s.left  .. w.left  .. cycle;

  anchor top = (advance / 2, x + over);
}

glyph c : U+0063 {
  advance = 2 stem + 330;
  center = (advance / 2 + 30, x / 2);
  ry = x / 2 + over - hair / 2;
  rx = advance / 2 - 40 - stem / 2;

  (n, w, s, e) = bowl(center, rx, ry);
  guide spine = n.pos .. w.pos .. s.pos;

  contour main = n.right .. w.right .. s.right
              -- s.left .. w.left .. n.left -- cycle;
}
```

The sheet shows both glyphs, advanced side by side in source order.
The `c` reuses the `o`'s bowl and simply doesn't travel the right side
— same nodes, different contour. This is the macros chapter's pattern
(macros return geometry, callers write the path) doing family-scale
work.

A workable layout for a real family, then:

```text
family.chi          # imports everything below; chi build family.chi
foundation.chi      # params, metrics, stroke weights
pens.chi            # pen blocks
constructions.chi   # bowl(), arch(), serif() macros
latin.chi           # glyph blocks
malayalam.chi       # glyph blocks
```

Import is include-once and relative to the importing file, so the glyph
files can also be built alone — each starts with
`import "foundation.chi";` and gets the same design space.

## Anchors and guides in the debug view

The debug rendering labels what the body declared — the spine guides,
each node, and the `o`'s `top` anchor floating above the ring:

```chi glyph=o debug
```

Everything on that sheet except the two contours is metadata: visible
scaffolding for the designer, invisible to the final render. The next
chapter is about the one promise all this organization has to keep
across the design space.
