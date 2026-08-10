# Paths and the curve fitter

At the heart of ductus, you'll find paths: an ordered list of on-curve points, each with optional direction constraints and a point type — smooth, corner, or line-adjacent. That's the complete picture of what a path keeps. There are no control handles tucked away anywhere; they're freshly computed each time the path is rendered.

## The syntax, completely

```ductus norender
p = a {up} .. b .. {theta} c -- d .. e{corner} .. cycle;
```

- `a .. b` — This creates a lovely smooth curve joint, with the solver  choosing the tangents for you.
- `a -- b` — Just a good old straight line.
- `a {theta} ..` — Here, you're setting a specific direction constraint for the path *leaving* `a`. Similarly, `.. {theta} b` constrains the direction *entering* `b`. Feel free to put any numeric expression in degrees inside those braces — `{up}` does the trick because `up` is defined as 90 degrees, and `{angle(b - a)}` is a clever way to aim one point directly at another.
- `e{corner}` — This marks a free corner, meaning each side of the point gets fitted independently, with no smoothness flowing across it. You can also create a corner by giving a point *different* entering and leaving direction constraints; the fitter will then honor those exact tangents.
- `cycle` — This handy keyword brings your path full circle, closing it back to its very first point using the joint you just defined. Remember, making a path closed is always an explicit choice here.
- When a line graciously meets a curve, that shared point automatically transforms into a special line/curve corner. The line, of course, continues its straight journey, while the curve gently lands tangent to it.

You won't find a path concatenation operator here, and that's by design. In ductus, a contour is always a single, cohesive path expression. Macros contribute their geometry by simply returning the points that path travels through – we'll dive deeper into this fascinating topic in the macros chapter.

## What the fitter does

Whenever you render a path (or query it, as we'll see below), ductus gets to work, fitting beautiful cubic Béziers through its points. It uses a clever two-parameter spline, ensuring G2-smoothness at interior points, faithfully honoring every direction constraint and corner you've set, and closing the path perfectly when it ends with `cycle`. Our solver proudly descends from John Hobby's brilliant METAFONT algorithm, and it boasts one property you'll definitely want to remember: **circular arcs are essentially exact**. Let's see it with four points and no constraints:

```ductus
u = 100;
c = (2u, 2u);
r = 1.5u;

ring = c + (r, 0) .. c + (0, r) .. c + (-r, 0) .. c + (0, -r) .. cycle;
draw(ring);
```

Believe it or not, that's a circle accurate to a remarkable 0.027 % of its radius — truly invisible at any size a glyph might be viewed. And here's a fun secret: add more points, and it gets even *better* (with eight points, you're looking at a mere 0.0004 % error). This incredible precision is precisely why ductus doesn't bother with a dedicated arc primitive; simply placing points on the circle gives you the arc you need.

## The one gotcha: squashed circles

Now, this same  property comes with a small but important caveat. While the fitter's handle lengths are spot-on for circles at every angle, this precision comes at a cost: it means the fit isn't affine-equivariant. What does that mean? Well, transforming a curve *after* it's fitted will give you a different result than fitting the points *after* they've been transformed. In practical terms, this little quirk usually shows up in just one scenario: creating an ellipse by simply scaling a circle.

| aspect ratio | error, as a share of the long radius |
|---|---|
| 1.0 | 0.03 % |
| 0.72 | 1.0 % |
| 0.5 | 3.8 % |
| 0.1 | 17 % |

Imagine a four-point "circle" squashed to half its height, and you'll find it misses a true ellipse by almost 4 %. So, here's a friendly tip: avoid scaling circles directly into ellipses. Instead, reach for the `ellipse()` constructor, which handily places eight points on the true curve for you (check out the reference chapter for more details). For exceptionally flat shapes, you might even want to add a few more points to achieve perfection. But don't fret about other transformations — things like shifting, rotating, or slanting are perfectly safe, because your *points and constraints* transform with complete accuracy, and the fitting magic happens afterward.

## Paths are values

Think of a path as a first-class citizen in ductus: you can bind it to a variable, pass it to a macro, or transform it in any way you like. And here's the best part: because the actual fitting happens only when you render it, none of these operations cause you to lose a single shred of information. Even contour and guide names within a glyph are just ordinary path bindings, which means your later code can easily measure them and interact with them.

ductus gives you four wonderful built-in functions to query a path. Crucially, these functions always 'see' the **fitted** curve — that's the very same beautiful geometry that gets rendered, not just the straight chords between your defined points. When we talk about 'time `t`,' we're using segment time, much like in METAFONT. This means `t` runs from `0` to `n` for a path with `n` segments, with integer values landing precisely on your on-curve points.

```ductus
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

`direction(p, t)` acts as the inverse of `directionpoint`, revealing the tangent angle at a specific moment in time. It's important to remember that `directionpoint` looks for a match in *orientation*, not just parallelism — so 0° means moving rightward, while 180° indicates a leftward journey. If no such point can be found, or if you try to query past the path's end, it will  let you know with an error.

For a wonderfully wavy path, you might find several points sharing the same tangent. No worries, an optional 0-based index is there to help you pick out the later ones. Looking at our example path above, `directionpoint(p, 0)` will happily find the crest, and `directionpoint(p, 0, 1)` will pinpoint the trough that comes right after it. Just be mindful not to ask for more matches than actually exist, as that would result in an error.

## intersection

Say hello to `intersection(p, q)`, which helpfully returns the very first point where two paths decide to cross each other — with "first" always referring to the earliest point along path `p`. When it comes to two line segments, this function is perfectly exact, and it  handles all those workhorse scenarios that METAFONT's `whatever` solver made famous: for instance, where exactly does a diagonal line cross a stem?

```ductus
u = 100;
stem = (2u, 0) -- (2u, 4u);
diag = (0, 3.5u) -- (4u, u);

draw(stem);
draw(diag);

hit = intersection(stem, diag);
draw(hit + (-15, -15) -- hit + (15, 15));
draw(hit + (-15, 15) -- hit + (15, -15));
```

Because these clever queries employ the very same fitter as the renderer, you can trust that a curved path is always 'cut' precisely where it's *drawn*. It's a wonderful principle: what you measure is truly what you see. You'll find Part III relies on this constantly — `directionpoint` becomes your go-to for finding perfect pen waists, and `intersection` is invaluable for building elegant joints. For now, just tuck away this  thought: a path in ductus isn't just a line on a page; it's a rich source of information you can always inquire about.
