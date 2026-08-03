# Paths and the curve fitter

A path is chi's central value: an ordered list of on-curve points with,
per point, optional direction constraints and a point type — smooth,
corner, or line-adjacent. That is *all* a path stores. No control handles
anywhere: they are computed, fresh, every time the path is rendered.

## The syntax, completely

```chi norender
p = a {up} .. b .. {theta} c -- d .. e{corner} .. cycle;
```

- `a .. b` — a smooth curve joint. The solver chooses the tangents.
- `a -- b` — a straight line.
- `a {theta} ..` — a constraint on the direction *leaving* `a`;
  `.. {theta} b` constrains the direction *entering* `b`. The braces hold
  any numeric expression in degrees — `{up}` works because `up` is 90,
  and `{angle(b - a)}` aims one point at another.
- `e{corner}` — a free corner: each side fitted independently, no
  smoothness across the point. Giving a point *different* entering and
  leaving direction constraints also makes it a corner, with those exact
  tangents.
- `cycle` — closes the path back to its first point with the preceding
  joint. Closedness is always explicit.
- Where a line meets a curve, the shared point automatically becomes a
  line/curve corner: the line stays straight, the curve lands tangent.

There is no path concatenation operator, deliberately: a contour is one
path expression, and macros contribute geometry by returning the points
it passes through — the macros chapter returns to this.

## What the fitter does

When a path is rendered (or queried — below), chi fits cubic Béziers
through its points with a two-parameter spline: G2-smooth at interior
points, honoring every direction constraint and corner, closed when the
path ends in `cycle`. The solver is a descendant of John Hobby's METAFONT
algorithm, and it has one property worth knowing by heart: **circular
arcs are essentially exact**. Four points, no constraints:

```chi
u = 100;
c = (2u, 2u);
r = 1.5u;

ring = c + (r, 0) .. c + (0, r) .. c + (-r, 0) .. c + (0, -r) .. cycle;
draw(ring);
```

That is a circle to 0.027 % of its radius — invisible at any size a glyph
is seen. Add points and it gets *better* (eight points: 0.0004 %). This
is why chi has no arc primitive: points on the circle are the arc.

## The one gotcha: squashed circles

The same property has a sharp edge. The fitter's handle lengths are exact
for circles at every angle, and that exactness is bought by *not* being
affine-equivariant: transforming the fitted curve and fitting the
transformed points give different results. In practice this bites in
exactly one place — an ellipse made by scaling a circle:

| aspect ratio | error, as a share of the long radius |
|---|---|
| 1.0 | 0.03 % |
| 0.72 | 1.0 % |
| 0.5 | 3.8 % |
| 0.1 | 17 % |

A four-point "circle" squashed to half its height misses a true ellipse
by almost 4 %. So don't scale circles into ellipses: the `ellipse()`
constructor places eight points on the true curve (reference chapter),
and for very flat shapes, spend more points still. Everything else —
shifting, rotating, slanting — is safe, because the *points and
constraints* transform exactly and the fit runs after.

## Paths are values

A path can be bound, passed to a macro, transformed, and — because the
fit happens at render time — none of that loses information. Contour and
guide names inside a glyph are ordinary path bindings too, so later code
can measure them.

Four builtins query a path, and they observe the **fitted** curve — the
same geometry that renders, not the straight chords between your points.
Time `t` is segment time, METAFONT-style: `0..n` for a path of `n`
segments, integers landing exactly on your on-curve points.

```chi
u = 100;
p = (0, 0) .. (2u, 2u) .. (4u, 0) .. (6u, 2u);
draw(p);

mid = point(p, 1.5);            # halfway along the middle segment
crest = directionpoint(p, 0);   # first point whose tangent is 0 degrees

draw(mid + (-15, -15) -- mid + (15, 15));
draw(mid + (-15, 15) -- mid + (15, -15));
draw(crest + (-15, -15) -- crest + (15, 15));
draw(crest + (-15, 15) -- crest + (15, -15));
```

`direction(p, t)` is the inverse of `directionpoint`: the tangent angle
at a given time. `directionpoint` matches *orientation*, not mere
parallelism — 0° means traveling rightward, 180° leftward — and it is an
error if no such point exists, as is querying past the end of the path.

A wavy path can have several points with the same tangent; an optional
0-based index picks the later ones. In the path above, `directionpoint(p,
0)` finds the crest and `directionpoint(p, 0, 1)` the trough that follows
it; asking for more matches than exist is an error.

## intersection

`intersection(p, q)` returns the first point where two paths cross —
"first" meaning earliest along `p`. On two line segments it is exact, and
it covers the workhorse uses of METAFONT's `whatever` solver: where does
the diagonal cross the stem?

```chi
u = 100;
stem = (2u, 0) -- (2u, 4u);
diag = (0, 3.5u) -- (4u, u);

draw(stem);
draw(diag);

hit = intersection(stem, diag);
draw(hit + (-15, -15) -- hit + (15, 15));
draw(hit + (-15, 15) -- hit + (15, -15));
```

Because the queries run the same fitter as the renderer, a curved path
is cut where it is *drawn*: what you measure is what you see. Part III
leans on this constantly — `directionpoint` finds pen waists,
`intersection` builds joints. For now, just file away that a path is
something you can ask questions of.
