# Built-in functions

Every builtin, with its exact behavior and error conditions. Three
conventions hold throughout: all angles are **degrees**
(counterclockwise, 0° = +x); wrong argument *counts* and wrong argument
*types* are evaluation errors naming the function; and builtin names are
reserved — a `def` may not redefine one.

## Numbers

| function | returns | notes |
|---|---|---|
| `min(a, b)` | the smaller | |
| `max(a, b)` | the larger | |
| `clamp(x, lo, hi)` | `x` limited to `[lo, hi]` | **error** when `lo > hi` |
| `abs(a)` | absolute value | |
| `sign(a)` | −1, 0, or 1 | `sign(0)` is 0, exactly |
| `sqrt(a)` | square root | |
| `floor(a)` `ceil(a)` `round(a)` | integers as numbers | |
| `sin(a)` `cos(a)` `tan(a)` | trigonometry | argument in degrees |

The `clamp` error is deliberate rather than defensive: a `lo` that
overtakes `hi` is almost always a param range gone wrong, and the
[interpolation chapter](interpolation-compatibility.md) wants that
surfaced, not absorbed.

## Points and vectors

A point doubles as a 2-D vector; these four convert between the vector
and angle views (see [Expressions](expressions.md#angles-and-vectors)):

| function | returns |
|---|---|
| `angle(v)` | the angle of vector `v`, degrees (atan2) |
| `unitvec(theta)` | the unit vector `(cos theta, sin theta)` |
| `length(v)` | `|v|` |
| `distance(a, b)` | `length(b - a)` |

## Path queries

All four observe the **fitted** curve — the same geometry that renders,
never the chords between on-curve points. Time `t` is segment time:
`0..n` for a path of `n` segments, integer values landing exactly on the
on-curve points, fractional values inside the corresponding segment.
The [paths chapter](paths-and-the-fitter.md#paths-are-values) develops
all of this.

| function | returns | errors when |
|---|---|---|
| `point(p, t)` | the position on `p` at time `t` | `t` outside `0..n` |
| `direction(p, t)` | tangent angle at `t`, degrees | `t` outside `0..n` |
| `directionpoint(p, d)` | first point of `p` whose tangent is `d` | no such point |
| `intersection(p, q)` | first intersection of the two paths | they don't meet |

Fine print worth knowing:

- `directionpoint` matches **orientation**, not mere parallelism: 0°
  means traveling rightward. The typical use — finding a pen's thin
  crossing — depends on this.
- "First", in both `directionpoint` and `intersection`, means least
  position along the first path: its segments in path order, then the
  least time within a segment.
- `intersection` on two line segments is exact — the `whatever` idioms
  of METAFONT (stem meets diagonal, serif meets stem) land here.
- Zero-length segments (see `roundrect` at radius 0) have no tangent
  and are invisible to the queries.

## Shape constructors

`circle`, `ellipse`, `roundrect` and `superellipse` return closed,
counterclockwise path values; they have the [next
chapter](shape-constructors.md) to themselves.

## Sketch commands

| function | effect |
|---|---|
| `draw(path, ...)` | render each path as a thin spine |
| `fill(path, ...)` | fill each closed path |

Top level only, and only meaningful in sketch files — a file with any
`glyph` block renders its glyphs instead. `fill` requires closed paths;
each `fill` is filled independently, so a counter (a hole) needs a
glyph's contours, not two fills.
