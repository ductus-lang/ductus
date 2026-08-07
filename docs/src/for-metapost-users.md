# Ductus for MetaPost users

If you're already comfy with METAFONT and MetaPost, you'll feel right at home with ductus. It's got the same familiar path syntax, mediation, and transform operators, practically unchanged. But wait, there are a few things that didn't make the journey: the equation solver, how pens stroke, and the idea of mutation. That's because ductus embraces a single-assignment, single-pass, outline-first philosophy. This little appendix will serve as your trusty translation guide, explaining what's different and why those delightful changes came to be.

## What transfers directly

Good news: many things you love about MetaPost slide right into ductus without a hitch, making your transition extra smooth.

| MetaPost | ductus | notes |
|---|---|---|
| `z1 .. z2 -- z3 .. cycle` | same | joints, `cycle` — unchanged |
| `z1{up} .. {right}z2` | same | direction specs, degrees or vectors via `angle()` |
| `0.5[a, b]` | same | mediation, ductus's favourite expression too |
| `p shifted (x,y) rotated t scaled k slanted s` | same | postfix, chaining, degrees |
| `reverse p` | `p reversed` | postfix like the rest |
| `subpath`, `point t of p` | `point(p, t)`, `direction(p, t)` | functions, not operators; **fitted** curve, not chords |
| `directionpoint d of p` | `directionpoint(p, d)` | matches orientation, errors if absent |
| `for i = 0 upto 5:` | `for i in 0..5 { }` | inclusive both ends |
| `beginchar("o", ...)` | `glyph o : U+006F { advance = ...; }` | advance is a required binding |

## What changed shape

Some concepts have taken on a slightly different form in ductus, thoughtfully reshaped to align with its unique way of doing things.

| MetaPost | ductus | why |
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

A few things have gracefully stepped aside to make room for ductus's distinct approach.

-   **`whatever` and the equation solver.** First up, say goodbye to `whatever` and the equation solver. Ductus prefers a single-pass, declarative dance, giving you exact derived points (like nodes or `intersection` results) where METAFONT used to wrestle with constraints. Everything else becomes a neat arithmetic calculation.
-   **`tension`.** We've also bid farewell to `tension`. In our experience, designers naturally gravitate towards tweaking points and directions directly, rather than fiddling with tension numbers. Feel free to add a point or a direction constraint if you need that extra control.
-   **`curl`.** `curl` has also sailed off. Open ends in ductus now happily accept direction constraints, with the default behavior gracefully handled by the fitter.
-   **Pens that stroke.** And then there are pens. In ductus, a pen is less of a brush that paints and more of a precision *measuring* device. It's brilliant at placing bank contact points and defining cap geometry (you can dive deeper into [pens](pens.md) and [caps](terminals-and-caps.md)). The whole idea is to cleverly avoid that METAFONT conundrum of having uneditable stroked output.
-   **Implicit `x0`/`y0` coordinate variables and suffix magic.** You'll also find that implicit `x0`/`y0` coordinate variables and suffix magic have gracefully stepped aside. In ductus, a name gets its meaning because you've lovingly given it one.

## The habit to unlearn, the habit to learn

It's time to gently unlearn the habit of 'solving.' You won't be stating relationships and then letting the program magically discover the numbers. Ductus prefers a strict top-to-bottom journey, where every value finds its definition just once, building beautifully upon the values that came before it.

Instead, let's embrace the design-space discipline. This is something rather special that ductus asks for, a little different from METAFONT. Think about maintaining the same number of points and types across every instance (that's our [interpolation compatibility](interpolation-compatibility.md)), and carefully selecting parameter ranges so every query always finds a happy solution. You'll get friendly with `ductus inspect` to peek at the corners of your design space. Where METAFONT programs used to describe a shape with a few knobs, a ductus file paints a picture of the *interpolable region* those knobs elegantly sweep. Once you get comfortable with this way of thinking, you'll find that masters, instances, and even a variable font all live harmoniously in the very same file.