# Built-in functions

Let's explore each built-in function, understanding its exact behavior and any conditions that might lead to an error. As you dive in, keep a few handy conventions in mind: all angles are measured in **degrees** (counting counterclockwise, with 0° pointing to +x); if you provide the wrong number of arguments or arguments of the wrong *type*, you'll encounter an evaluation error that conveniently names the function; and finally, built-in names are reserved—so you can't use a `def` to redefine one.

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

The way `clamp` throws an error is quite intentional, not just a defensive measure. We see a `lo` value that goes beyond `hi` as a strong signal that something has likely gone awry with your parameter range. The [interpolation chapter](interpolation-compatibility.md) even relies on these issues being brought to your attention, rather than quietly absorbed.

## Points and vectors

A single point in chi is wonderfully versatile, effectively doubling as a 2-D vector. These next four functions help you smoothly convert between seeing things as vectors and understanding their angular representations (you can find more details in the [Expressions chapter](expressions.md#angles-and-vectors)):

| function | returns |
|---|---|
| `angle(v)` | the angle of vector `v`, degrees (atan2) |
| `unitvec(theta)` | the unit vector `(cos theta, sin theta)` |
| `length(v)` | `|v|` |
| `distance(a, b)` | `length(b - a)` |

## Path queries

These four functions all take a close look at the **fitted** curve. This is the very same geometry that gets rendered, ensuring you're never dealing with just the chords between on-curve points. When we talk about time `t`, we're referring to segment time. It runs from `0` all the way to `n` for a path made of `n` segments. Integer values of `t` will land you precisely on the on-curve points, while fractional values will place you somewhere inside the corresponding segment. If you'd like a deeper dive, the [paths chapter](paths-and-the-fitter.md#paths-are-values) covers all of this in detail.

| function | returns | errors when |
|---|---|---|
| `point(p, t)` | the position on `p` at time `t` | `t` outside `0..n` |
| `direction(p, t)` | tangent angle at `t`, degrees | `t` outside `0..n` |
| `directionpoint(p, d)` | first point of `p` whose tangent is `d` | no such point |
| `directionpoint(p, d, i)` | the `i`-th such point, 0-based | fewer than `i + 1` such points |
| `intersection(p, q)` | first intersection of the two paths | they don't meet |

Here's some fine print worth knowing:

- `directionpoint` cares about the **orientation** of the tangent, not just whether it's parallel. For instance, 0° means the path is traveling rightward. This is super handy for common tasks like finding where a pen's thin stroke crosses a certain direction.
- When we say "First" for both `directionpoint` and `intersection`, we mean the point with the least position along the first path. This involves checking its segments in path order, and then the earliest time within a segment. `directionpoint` also offers an optional index, allowing you to count matches in that same order, starting from 0. So, `directionpoint(p, d, 0)` is simply another way to write `directionpoint(p, d)`. If a match happens to land exactly on a joint (or on a closed path's start point), it counts as a single match.
- `intersection` delivers exact results when applied to two line segments. This is where those classic METAFONT "whatever" idioms—like a stem meeting a diagonal or a serif gracefully touching a stem—really shine.
- Keep in mind that zero-length segments (like what you'd find with `roundrect` when its radius is 0) don't have a tangent and won't show up in these queries.

## Shape constructors

Meet `circle`, `ellipse`, `roundrect`, and `superellipse`—these delightful functions return closed, counterclockwise path values. They're so important, they even get their very own [next chapter](shape-constructors.md) to shine in.

## Sketch commands

| function | effect |
|---|---|
| `draw(path, ...)` | render each path as a thin spine |
| `fill(path, ...)` | fill each closed path |

These commands are special; you'll only find them meaningful at the top level of sketch files. If your file happens to include any `glyph` block, chi will happily render those glyphs instead. A quick note: `fill` is designed to work with closed paths. Each `fill` operation works independently. This means if you're trying to create a counter (that's a fancy word for a hole), you'll need to define it using a glyph's contours, rather than trying to use two separate `fill` commands.