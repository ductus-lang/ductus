# Malayalam tha

Malayalam ത (tha) is a double bowl with inward-curling tips — a shape
with no straight lines, four counters' worth of white space, and a
left/right symmetry begging to be written once. It is the space-first
study: the advance is *derived from the counters*, so the width axis
squeezes white, not strokes. This walkthrough tracks the real
`tha.chi` from the example set.

## Foundation

```chi norender acc
params {
  weight   = 400 in 100..900;
  contrast = 0.85 in 0.4..1.0;
  width    = 100 in 60..120;
}
metrics {
  upm = 1000; x = 500; cap = 700; asc = 750; desc = -220;
  over = 10 + weight / 200;
}

stem = 40 + 0.14 * weight;
hair = contrast * stem;
term = 0.75 * hair;          # the tips taper before their round caps
```

Note the third axis: `width` is not the stroke weight — that is
`weight` — but the letter's *openness*.

## Symmetry as a macro

The letter's right half mirrors its left. A macro mirrors a node across
a vertical axis — and the one subtlety is that the direction is
*negated*, because a mirrored node is read by a traversal running the
other way:

```chi norender acc
def mirror(n, axis) -> m {
  node m = (2 * axis - n.pos.x, n.pos.y) dir -n.dir width n.width;
}
```

This is the macros chapter's promise cashed in: a macro takes a node,
returns a node, and the caller decides what to do with its banks.

## Counters first, then the glyph

```chi acc glyph=tha
glyph tha : U+0D24 {
  bowl = 1.6 * width;             # bowl counter width
  gap  = 0.55 * width;            # counter between the central strokes

  wx  = 45 + stem / 2;            # left outer stroke, centre
  lcx = wx + stem + bowl;         # left central stroke, centre
  rcx = lcx + stem + gap;         # right central stroke, centre
  ex  = rcx + stem + bowl;        # right outer stroke, centre
  advance = ex + stem / 2 + 45;
  mid = 0.5[lcx, rcx];

  ax = 0.5[wx, lcx] + 14;         # tip hovers just right of bowl centre

  # Left half of the skeleton, travelling from the tip into the letter:
  # curl up over the left shoulder, down around the left bowl, in to
  # the centre.
  node a = (ax, 0)                            dir 150   width term;
  node b = (wx, 0.48 * x)                     dir up    width term;
  node c = (0.5[wx, lcx], x + hair / 2 - over) dir right width hair;
  node d = (mid + 2gap, b.pos.y)              dir down  width hair;
  node e = (mid, over)                        dir left  width hair;

  # Right half by symmetry.
  f = mirror(d, mid);
  g = mirror(c, mid);
  h = mirror(b, mid);
  i = mirror(a, mid);

  guide skeleton = a.pos .. b.pos .. c.pos .. d.pos .. e.pos
                .. f.pos .. g.pos .. h.pos .. i.pos;

  # Round caps by hand: one extra point past each tip, half a width
  # out along the travel direction, tangents pinned to a semicircle.
  acap = a.pos - (a.width / 2) * unitvec(a.dir);
  icap = i.pos + (i.width / 2) * unitvec(i.dir);

  contour main =
       {a.dir} a.right {a.dir}
    .. b.right .. c.right .. d.right .. e.right .. f.right .. g.right
    .. h.right
    .. {i.dir} i.right {i.dir}
    .. icap {i.dir + 90}
    .. {i.dir + 180} i.left {i.dir + 180}
    .. h.left .. g.left .. f.left .. e.left .. d.left .. c.left
    .. b.left
    .. a.left {a.dir + 180}
    .. acap {a.dir + 270}
    .. cycle;
}
```

Three things to unpack, in order of importance.

**The advance is a sum of counters and strokes.** Read the `wx → lcx →
rcx → ex` chain: each stroke centre is the previous one plus a stroke
plus a *counter* (`bowl` or `gap`). The letter is literally laid out as
alternating black and white, so when the `width` axis moves, every
counter breathes and every stroke keeps its weight — the opposite of
naive scaling.

**One contour, eighteen-plus points, still just two banks.** The
skeleton runs tip to tip through nine nodes; the contour goes out along
the right bank, around the right cap, back along the left bank, around
the left cap. The same shape as the pressure chapter's strokes, at
letter scale.

**The caps are ordinary points.** These are `width` nodes — no pen, so
no `tip` or `tipcap`. A round cap is built from what a cap *is*: a point
half a width past the tip (`acap`, `icap`), entered and left along the
pinned semicircle tangents. Chi's no-cap-machinery principle means this
costs two bindings and four constraints — and means the taper into the
cap (`term`, thinner than `hair`) was free to design.

The construction view shows the skeleton snaking through both bowls and
the node dots that place every bank point:

```chi glyph=tha debug
```

## Across the space

Weight moves ink; the counters hold their proportions:

```chi glyph=tha set="weight=150"
```

```chi glyph=tha set="weight=850"
```

And the width axis moves *white*: at `width=60` the bowls tighten and
the letter condenses without a single stroke thinning; the black version
of the same instance stays credible because black and white are
independent:

```chi glyph=tha set="width=60"
```

```chi glyph=tha set="width=120 weight=700"
```

Of the three studies this is the one closest to real practice: a
complex-script glyph is mostly a negotiation between counters, and a
source format where the advance is *written as* that negotiation keeps
it on the page, where the next designer can read it.
