# First strokes

Before glyphs, metrics, and advance widths, there is a simpler way to run
chi: the sketch. A file with no `glyph` block is a sketch file — top-level
`draw()` renders paths as thin spines, `fill()` fills closed ones. Sketches
are how you doodle, test an idea, or follow along with this chapter. Paste
any example into the [playground](https://gitlab.com/chi-lang/chi-playground)
or run it from a file:

```sh
chi build sketch.chi -o sketch.svg
```

## Numbers, names, comments

A chi statement ends with a semicolon; `#` starts a comment; `=` binds a
name. Bindings are single-assignment — rebinding a name in the same scope
is an error, which keeps any value in a file traceable to one line.

A number literal directly in front of an expression multiplies it, so once
you bind a unit, coordinates read like graph paper: `3u` is three units,
`2(a + b)` twice a sum.

```chi
u = 100;                   # our unit
draw((0, 0) -- (4u, 2u));  # a line segment
```

The coordinate system is Cartesian and y-up: x grows right, y grows up,
like the maths you learned and unlike most screen APIs. `--` joins points
with a straight line.

## Points are values

`(x, y)` is a point. Points add, subtract, and scale, and a point does
double duty as a 2-D vector, so displacement is just arithmetic:

```chi
u = 100;
a = (0, 0);
b = (6u, 2u);
m = 0.5[a, b];             # halfway from a to b

draw(a -- b);
draw(m + (-u/4, 0) -- m + (u/4, 0));   # a cross marking m
draw(m + (0, -u/4) -- m + (0, u/4));
```

`t[a, b]` is *mediation*, the expression you will use most in chi: the
point (or number) `t` of the way from `a` to `b`. `0.5[a, b]` is the
midpoint; `t` can be any expression.

## Curves

Join points with `..` instead of `--` and chi fits a smooth curve through
them. This is the heart of the language: you place on-curve points, and a
two-parameter spline solver — a descendant of the one John Hobby built for
METAFONT — chooses every control handle, keeping the curve smooth through
each point.

```chi
u = 100;
p = (0, 0) .. (2u, 2u) .. (4u, 0) .. (6u, 2u);
draw(p);
```

No handles appear in the source, and none ever will. When a curve isn't
what you want, you don't reach for handles — you add a point where the
geometry needs witnessing, or you constrain a direction.

## Directions

A direction in braces constrains the curve's tangent at a point: `{45}`
means "leave (or enter) at 45 degrees". Angles are in degrees,
counterclockwise, with 0° pointing right; `right`, `up`, `left`, `down`
are ordinary bindings for 0, 90, 180, 270. A spec before a point
constrains the curve *entering* it, after a point the curve *leaving* it.

```chi
u = 100;
a = (0, 0);
b = (4u, 0);

p = a .. (2u, u) .. b;              # the fitter's choice
q = a {up} .. (2u, u) .. {down} b;  # forced vertical ends

draw(p);
draw(q shifted (5u, 0));            # beside it, to compare
```

The left wave takes the gentle route; the right one is forced to leave `a`
straight up and dive into `b` straight down. (`shifted` is one of chi's
transform operators — they get a chapter of their own.)

## Mixing lines and curves

`--` and `..` mix freely in one path. Where a line meets a curve, the
meeting point automatically becomes a line/curve corner: the curve arrives
tangent to whatever direction it needs, the line stays perfectly straight.

```chi
u = 100;
fill((0, 0) -- (3u, 0) .. (4u, u) .. (3u, 2u) -- (0, 2u) -- cycle);
```

`cycle` closes the path back to its first point — closedness is always
written, never inferred — and `fill()` needs a closed path.

## Corners

Sometimes a point should be sharp *between two curves*. Mark it with
`{corner}` and the fitter treats each side independently, no smoothness
across the point:

```chi
u = 100;
wave    = (0, 0) .. (u, u) .. (2u, 0) .. (3u, u) .. (4u, 0);
scallop = (0, 0) .. (u, u) .. (2u, 0){corner} .. (3u, u) .. (4u, 0);

draw(wave);
draw(scallop shifted (5u, 0));
```

Same five points. On the left the fitter keeps the curve smooth through
the middle point, so it swings through a rounded valley. On the right the
middle point is a corner: each side is fitted independently, the two
arches end steeply, and they meet in a cusp. Corners like this — placed
where two curves should visibly break — are the germ of every teardrop
terminal and ink trap in Part III.

## Reading the sheet

Everything renders onto one sheet. Passing `--debug` to `chi build` (or
toggling debug in the playground) draws a coordinate grid under it — the
book's debug figures use it too, like this rendering of the wave from
earlier:

```chi hide
u = 100;
draw((0, 0) .. (2u, 2u) .. (4u, 0) .. (6u, 2u));
```

```chi debug
```

The grid is graph paper in font units: fine lines every 10, heavier every
50, heaviest every 100 — so the wave's crests at `y = 200` sit two heavy
squares above the baseline. For sketches
the sheet simply sizes itself to what you drew; a real glyph, as the next
chapter shows, gets its vertical extent from font metrics instead — along
with baselines, heights, and everything else the debug view is really for.
