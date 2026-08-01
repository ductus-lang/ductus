# Shape constructors

Four builtins return the closed shapes that would otherwise be written
out point by point. What they return are **ordinary path values** — the
on-curve points and their tangent constraints, handles still left to the
fitter — so they transform, they answer the path queries, and anything
they build the designer could also write by hand:

```chi
u = 100;

fill(circle((u, u), 0.8u));
fill(ellipse((3.2u, u), 1.1u, 0.65u));
fill(roundrect((4.8u, 0.3u), (6.4u, 1.7u), 30));
fill(superellipse((8u, u), 0.8u, 0.8u, 4, 16));
```

Two properties are shared by all four, and both are design decisions:

- **Counterclockwise**, so the result is directly usable as an outer
  contour. A counter is the same call `reversed`.
- **The point count never depends on a value** — only on which
  constructor was called — so a shape keeps its topology across the
  whole design space, per the
  [interpolation rule](interpolation-compatibility.md).

## circle(c, r) — 4 points

A point on each axis extreme, tangents pinned. The fitter is essentially
exact on circular arcs, so four points describe a circle to 0.027 % of
`r` — invisible at any size type is seen.

## ellipse(c, rx, ry) — 8 points

Eight, not four, and a separate constructor rather than
`circle(...) yscaled k` — both for the same reason. Four points describe
an ellipse only to about 1 % (the fitter's handle rule is exact for
circles, not ellipses — the [fitter
chapter](paths-and-the-fitter.md#the-one-gotcha-squashed-circles)'s
gotcha), so `ellipse` spends eight points and reaches 0.06 %. And
`ellipse(c, r, r)` is deliberately *not* `circle(c, r)`: branching on
`rx == ry` would change the point count as an axis crossed 1.0.

For very flat ellipses, spend more points still —
`superellipse(c, rx, ry, 2, count)` is the same curve with a chosen
count: at a 1:5 aspect it goes from 3.9 % at eight points to 0.6 % at
sixteen and 0.05 % at thirty-two.

## roundrect(p, q, r) — 8 points

Two opposite corners, in any order, and a corner radius: four lines and
four quarter-arcs. The radius is clamped to half the shorter side,
above which neighbouring arcs would overlap. At `r = 0` it is a
rectangle that still *carries* eight points — four of its segments are
zero-length — rather than a four-point path, so the count stays fixed
across a design space where a param drives the radius to zero:

```chi
u = 100;
fill(roundrect((0, 0), (1.6u, 1.6u), 0));
fill(roundrect((2.2u, 0), (3.8u, 1.6u), 24));
fill(roundrect((4.4u, 0), (6u, 1.6u), 60));
fill(roundrect((6.6u, 0), (8.2u, 1.6u), 500));   # clamped to 80
```

One consequence of the zero-length segments: they have no tangent, so
the path queries cannot see them. A `directionpoint` on a radius-0
roundrect finds the sides, never the degenerate corners.

## superellipse(c, rx, ry, n, count)

The family `|x/rx|^n + |y/ry|^n = 1`: `n = 2` is the ellipse, larger
`n` squares the shape off toward the bounding box. The exponent may be
any expression ≥ 2 (an error below — the family is not defined toward
the diamond side here), and since `n` only moves *positions*, it can be
a param: a squareness axis is one binding.

```chi
u = 100;
fill(superellipse((u, u),    0.8u, 0.8u, 2, 16));
fill(superellipse((3.2u, u), 0.8u, 0.8u, 4, 16));
fill(superellipse((5.4u, u), 0.8u, 0.8u, 10, 16));
```

`count` is the sampled point count: at least 4, at most 1024, and a
multiple of 4 so a point lands on each axis extreme. It is an argument
precisely so that it can *not* silently vary — passing a param-derived
count is the interpolation chapter's trapdoor, spelled out. Sampling,
not fitting, limits accuracy: sixteen points hold `n = 4` to 0.15 % of
the side, but only 0.69 % at `n = 10`; flatter and squarer shapes both
reward more points.

## Constructors in glyphs

Inside a glyph the constructors are contour factories, and `reversed`
makes counters — here a washer whose outer edge squareness and hole are
both parametric:

```chi
metrics { upm = 1000; asc = 750; desc = -220; }

glyph washer {
  advance = 500;
  c = (250, 250);

  contour rim  = superellipse(c, 210, 210, 3, 16);
  contour hole = circle(c, 90) reversed;
}
```

The [debugging chapter](debugging.md)'s winding rules apply unchanged:
the constructor hands you counterclockwise, and `reversed` is the whole
story of making it a hole.
