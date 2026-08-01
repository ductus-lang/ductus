# Interpolation compatibility

The first chapter stated the rule; this one makes it operational. For
any two instances in the parameter space, **every contour must yield
the same number of Bézier segments with the same point types**. That is
what interpolation between masters requires — point 3 of the light `o`
must correspond to point 3 of the black `o` — and it is the promise the
future designspace sampler will rely on when it cuts masters from one
chi source.

## Why you mostly can't break it

Chi's core design makes the rule hold *by construction*: the number and
type of on-curve points in a contour is fixed by the source text, not by
parameter values. Moving a param moves coordinates, never topology.
Every construct that could threaten the rule was shaped around it:

- Shape constructors have fixed point counts — `ellipse()` is always
  eight points, which is why `ellipse(c, r, r)` is not `circle(c, r)`.
- There is no boolean geometry, no "remove this point if", no
  conditional statement — only conditional *expressions*, which select
  values, not structure.
- A witness node from `directionpoint` is one point at every instance,
  wherever the crossing moves to.

Check it the same way the tooling does — point counts are in
`chi inspect`:

```text
glyph o
  advance                  636
  contour outer       4 points
  contour inner       4 points
```

Same counts at `--set weight=100` and at `--set weight=900`: compatible.

## The trapdoors

Three constructs can still express topology change, and all three were
flagged when the language chapters introduced them:

- **Conditionals over geometry choices.** `r if weight < 500 else 0` is
  fine — a number. Using a conditional to swap which *construction* a
  contour follows is fine too, so long as both arms pass through the
  same number of points. What must not happen is arms with different
  point counts; the conditional chapter called this the intended-use
  boundary.
- **`for` bounds that depend on a param.** One decorative element per
  100 units of weight means the light and the black have different
  contours entirely.
- **Count arguments.** `superellipse(c, rx, ry, n, count)` takes its
  point count as an argument precisely because it must not vary; passing
  a param-derived `count` reintroduces the problem the constructors were
  designed to avoid.

The evaluator does not police any of this in v0.1. Until the sampler
exists, keeping the rule is on you — the good news is that
`chi inspect` at the corners of the design space is a thirty-second
audit.

## The other failure mode: existence

Topology can be fixed and an instance can still *fail to evaluate*.
The path queries error when what they ask for is not there — and
whether it is there can depend on params:

- A `directionpoint(skel, pen_angle)` witness exists only while the
  skeleton's tangent actually sweeps through `pen_angle`. Rotate the pen
  axis past the sweep of some leg and that instance errors.
- An `intersection()` of a serif line and a diagonal exists only while
  the diagonal reaches far enough.
- `clamp(x, lo, hi)` errors when a param-derived `lo` overtakes `hi`.

The playground's `latin_set.chi` documents a real case in its header:
its `pen_angle` range is deliberately narrowed so that a witness node on
one leg of the `a` exists across the whole space. That is the right
mitigation today — **choose param ranges so every query stays
satisfiable**, and state the constraint in a comment where the range is
declared. A build at each corner of the design space is the cheap
regression test:

```sh
chi check family.chi
chi build family.chi --set weight=100 -o /dev/null
chi build family.chi --set weight=900 -o /dev/null
```

When the designspace sampler arrives, walking the space and reporting
both incompatibility and unsatisfiable queries is exactly its job.
