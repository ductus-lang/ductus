# Chi for MetaPost users

Chi descends from METAFONT and MetaPost, and readers fluent in them can
move fast: path syntax, mediation and the transform operators carry
over nearly verbatim. What does *not* carry over is the equation
solver, the pen stroking, and the mutation — chi is single-assignment,
single-pass, outline-first. This appendix is the translation table and
the reasoning behind its gaps.

## What transfers directly

| MetaPost | chi | notes |
|---|---|---|
| `z1 .. z2 -- z3 .. cycle` | same | joints, `cycle` — unchanged |
| `z1{up} .. {right}z2` | same | direction specs, degrees or vectors via `angle()` |
| `0.5[a, b]` | same | mediation, chi's favourite expression too |
| `p shifted (x,y) rotated t scaled k slanted s` | same | postfix, chaining, degrees |
| `reverse p` | `p reversed` | postfix like the rest |
| `subpath`, `point t of p` | `point(p, t)`, `direction(p, t)` | functions, not operators; **fitted** curve, not chords |
| `directionpoint d of p` | `directionpoint(p, d)` | matches orientation, errors if absent |
| `for i = 0 upto 5:` | `for i in 0..5 { }` | inclusive both ends |
| `beginchar("o", ...)` | `glyph o : U+006F { advance = ...; }` | advance is a required binding |

## What changed shape

| MetaPost | chi | why |
|---|---|---|
| `x1 = whatever[z2, z3]; ...` equations | `intersection(p, q)` | no linear solver; the type-design uses of `whatever` are path intersections, which are explicit and exact on lines |
| `penpos1(w, phi)` | `node n = ... dir theta width w nib phi;` | the node also carries travel direction, and derives `n.left`/`n.right` exactly |
| `pickup pencircle xscaled w yscaled h rotated phi` | `pen name { width = w; height = h; angle = phi; }` | a pen derives edge points; it never strokes anything |
| `penstroke` / `draw` with a pen | write both banks through derived points | outline-first: the contour is the artifact, and [terminals](terminals-and-caps.md) are outline too |
| `fill` / `unfill` | contours with opposite winding | nonzero rule; `unfill` is a `reversed` counter |
| `p & q` concatenation | macros return points; one path expression per contour | no seam-merging rules to learn — the [macros chapter](macros.md) argues this out |
| `vardef` returning a path | `def name(...) -> value { }` | tuple exports, destructured at the call site |
| numeric equations, `:=` | `name = expr;` once | single-assignment; any value traces to one line |

## What is gone on purpose

- **`whatever` and the equation solver.** Chi keeps evaluation
  single-pass and declarative; where METAFONT solved constraints, chi
  gives exact derived points (nodes, `intersection`) and expects the
  rest to be arithmetic.
- **`tension`.** Considered and dropped: in practice designers adjust
  points and directions, not tension numbers. Add a point or a
  direction constraint instead.
- **`curl`.** Open ends take direction constraints; the default end
  behavior is the fitter's.
- **Pens that stroke.** A chi pen is a *measuring* device — it places
  bank contact points and cap geometry ([pens](pens.md),
  [caps](terminals-and-caps.md)) — never a brush that paints. The
  METAFONT problem of uneditable stroked output is the problem chi was
  shaped to avoid.
- **Implicit `x0`/`y0` coordinate variables and suffix magic.** A name
  means something because you bound it.

## The habit to unlearn, the habit to learn

Unlearn solving: there is no "state relationships and let the program
find the numbers". Chi runs strictly top to bottom, and every value is
defined once, from values above it.

Learn the design-space discipline instead — the one thing chi asks that
METAFONT never did. Same point count and types at every instance
([interpolation compatibility](interpolation-compatibility.md)), param
ranges chosen so every query stays satisfiable, `chi inspect` at the
corners of the space. METAFONT programs described a shape with knobs;
a chi file describes the *interpolable region* those knobs sweep. Get
used to thinking that way, and masters, instances and a variable font
are all the same file.
