# Why chi

Chi is a language for parametric type design. A chi source file describes
glyph outlines as code: every point on a contour is an expression over named
design parameters — weight, contrast, width, whatever the design calls for —
and the curve handles are never written by hand. You give on-curve points,
directions, and point types; a spline solver places the control points.

This is what a chi glyph looks like rendered. It is an `o` whose stem
thickness, hairline thickness and overshoot are all expressions over two
parameters, `weight` and `contrast`:

```chi hide
params {
  weight   = 400 in 100..900;
  contrast = 0.85 in 0.6..1.0;
}
metrics {
  upm = 1000; x = 500; cap = 700; asc = 750; desc = -220;
  over = 10 + weight / 200;
}

stem = 40 + 0.14 * weight;
hair = contrast * stem;

def bowl(c, rx, ry) -> (n, w, s, e) {
  node n = c + (0,  ry) dir left  width hair;
  node w = c + (-rx, 0) dir down  width stem;
  node s = c + (0, -ry) dir right width hair;
  node e = c + (rx, 0)  dir up    width stem;
}

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
```

```chi glyph=o
```

Change nothing in the source and set `weight = 850`, and every point moves,
the solver refits the curves, and the same file yields the black version of
the same design:

```chi glyph=o set="weight=850"
```

The full source of this `o` is about twenty-five lines; the book builds it
up step by step in the chapters on nodes. What matters here is what those
twenty-five lines commit to — chi's design rests on five principles, and
most of what feels unusual about the language follows from taking them
seriously.

## Outline-first

The filled contour is the primary artifact. There is no stroke expansion in
the language: nothing takes a center line and a pen and produces an outline
for you. This is a deliberate rejection of the approach that made
METAFONT's output so hard to edit — when the outline is generated, every
correction fights the generator. In chi the outline is what you wrote, so a
terminal, an ink trap, or an optical correction is an ordinary edit that
stays parametric.

## Skeleton as scaffolding, not geometry

Chi still knows about strokes — it just refuses to draw them for you.
A *node* is a skeleton point that carries the stroke's local direction and
width, and exposes exactly-computed edge points:

```chi norender
node w = center + (-rx, 0) dir down width stem;
# w.left and w.right are now points, exactly stem/2 from w.pos
```

Outlines are written *through* these derived points. Curves are never
offset — only points are — so there is no offset-curve approximation
anywhere, and the designer can nudge any derived point with plain
arithmetic (`w.right + (3, 0)`) when the design demands it. The debug view
shows the scaffolding of the `o` above: the skeleton spine as a guide, each
node with its direction and its two derived edge points, and the fitted
handles of the final contours.

```chi glyph=o debug
```

## Interpolation compatibility by construction

The number and type of on-curve points in a contour is fixed by the source
text, not by parameter values. Every instance in the design space has
identical topology — the `o` above keeps exactly four points on each
contour whether `weight` is 100 or 900, which is precisely what
interpolation between masters requires. Language constructs are shaped
around this rule; it is why, for example, chi's shape constructors never
choose a point count from a parameter.

## The designer owns the outline

Everything a node or a constructor derives can be replaced or adjusted by
an ordinary expression. Chi has no notion of a point you are not allowed
to touch. The intended workflow is: let the scaffolding put points where
the theory says, then move the ones the eye disagrees with — and keep the
move parametric, because it is an expression too.

## One way to say a thing

Chi descends from METAFONT and MetaPost, and its path syntax will look
familiar to anyone who has used them. But it drops, on purpose, most of
their cleverness:

- **No equation solver.** MetaPost's `whatever` let you say "these lines
  meet somewhere; solve for it". Chi gives you `intersection()` on paths,
  which covers the cases type design actually uses — stem meets diagonal,
  serif meets stem — without a constraint system in the language.
- **No `tension`.** Per-joint tension was considered and dropped: in
  practice designers adjust points and directions, not tension numbers.
- **No path concatenation.** A contour is one path expression. Macros
  share geometry by returning the points and nodes it passes through, not
  by gluing path segments (MetaPost's `&`), so there are no seam-merging
  rules to learn.
- **No implicit variables.** No reserved `x0`/`y0` magic; a name means
  something because you bound it.

Each removal makes source files more literal: what the file says is what
the glyph is.

## What chi is not, yet

The core language evaluates to SVG — one drawing per glyph, or a sheet.
UFO and designspace emission, and sampling masters across the parameter
space, are planned as a layer on top (the interpolation-compatibility rule
exists so that layer can work). Chi is also not an interactive editor:
the [playground](https://gitlab.com/chi-lang/chi-playground) gives you
live rendering and parameter sliders, but the source file is always the
design.

## How to read this book

The next two chapters get you drawing: freehand sketches first, then a
real glyph. Part II covers the language proper, and Part III — nodes,
pens, terminals, pressure — is where chi starts to feel like a design
tool rather than a programming language; if you already draw type for a
living, it is fine to skim ahead to it. Keep the playground open in a
second tab as you read: nearly every example in this book fits in it,
and dragging a slider teaches faster than we can write. Every figure is
compiled from the source shown by the book build itself, so the code
you read is code that runs. The normative language definition lives in the
[specification](https://gitlab.com/chi-lang/tree-sitter-chi/-/blob/main/Spec.md);
this book cites it but does not replace it.
