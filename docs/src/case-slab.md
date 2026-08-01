# A slab-serifed stem

The simplest study first, and the only one with no skeleton at all: a
capital stem with a bracketed slab serif, written as one direct outline.
It shows how far plain paths and the automatic line/curve corners go
before nodes are needed at all — and how serif proportions stay
parametric when they are ordinary expressions of the stem.

## The design

A slab serif is a rectangle under the stem; the *bracket* is the small
curve easing the serif into the stem so the joint doesn't read as a
right angle. Four numbers describe it, all derived from `stem` so the
serif keeps its proportions as weight moves:

```chi norender acc
params { weight = 400 in 100..900; }

metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; }

stem = 40 + 0.14 * weight;

sl = 30 + 0.45 * stem;    # serif overhang beyond the stem
sh = 24 + 0.20 * stem;    # serif slab height
br = 26 + 0.30 * stem;    # bracket size
```

## The outline

Counterclockwise from the serif's bottom-left corner. Every corner
along the slab is a line/line corner and needs no annotation; the two
brackets are smooth *because* their entry and exit directions are
constrained — `{left} .. {up}` carries the curve from horizontal into
vertical:

```chi acc glyph=stem
glyph stem {
  advance = stem + 2 sl + 100;
  xl = (advance - stem) / 2;    # stem left edge
  xr = xl + stem;               # stem right edge

  contour main =
      (xl - sl, 0) -- (xr + sl, 0) -- (xr + sl, sh)
   -- (xr + br, sh) {left} .. {up} (xr, sh + 2br)
   -- (xr, cap) -- (xl, cap)
   -- (xl, sh + 2br) {down} .. {left} (xl - br, sh)
   -- (xl - sl, sh) -- cycle;
}
```

Read the right bracket: the path arrives along the serif's top heading
`{left}`, curves, and leaves heading `{up}` into the stem — one segment,
two constraints, and the fitter finds the ease. Where the bracket's
curve meets the straight lines on either side, the meeting points became
line/curve corners automatically.

The debug view shows the whole structure — note the on-curve points
clustered at the brackets, and nowhere else:

```chi glyph=stem debug
```

## Across the space

Because `sl`, `sh` and `br` grow with `stem` (but not proportionally —
each has a constant floor), the serif gets sturdier in the black without
becoming a brick, and stays present in the light without vanishing:

```chi glyph=stem set="weight=120"
```

```chi glyph=stem set="weight=880"
```

Ten points at every instance. The one design decision worth stealing is
in the derivations, not the path: a serif proportioned as
`floor + k · stem` interpolates *sensibly*, which no amount of clever
outline can rescue if the numbers underneath are wrong.
