# Pressure

Chi has no pressure primitive. It doesn't need one: a node's width is an
ordinary expression, so *pressure is just that expression*, sampled
along a stroke the same way an outline is sampled at points. This
chapter is two idioms — the pointed pen and the pressure-scaled pen —
and the shared pattern underneath them.

## The pattern: press()

Both idioms use the path queries from the language chapters to build
nodes *on* a spine. Given a path, a time along it, and a pressure, a
macro returns the sampled node — position and direction both come from
the path, so the banks are true offsets of the curve that renders:

```chi norender
def press(path, t, pressure) -> n { ... }

e = press(spine, 2.65, 1);    # "at 2.65 along, full weight"
```

The spine is drawn once, as a gesture of three or four points; pressure
is then placed *along* it. Sampling density is free, which matters — an
arch shoulder can turn 125° in one segment, and without nodes inside the
turn, the inner bank has nothing to trace.

## The pointed pen

A pointed pen's tines spread under pressure, *across* the direction of
travel — the slit runs along the stroke. That is exactly a node's
default nib axis, so the pointed pen needs no pen at all: width alone is
the whole model.

```chi
u = 100;
shade = 60;              # full-pressure thickness
hair  = 5;               # the unloaded mark

def press(path, t, pressure) -> n {
  node n = point(path, t) dir direction(path, t)
           width hair + pressure * (shade - hair);
}

spine = (2u, 4u) {265} .. (1.85u, 2u) .. {275} (2u, 0);

a = press(spine, 0,   0);
b = press(spine, 0.6, 0.9);
c = press(spine, 1.4, 0.9);
d = press(spine, 2,   0);

fill(a.right .. b.right .. c.right .. d.right
  -- d.left .. c.left .. b.left .. a.left -- cycle);
```

A copperplate full-pressure pull: hairline into the stroke, swell
through the middle, release to hairline out. The pressure profile *is*
the four `press` lines. Where a stroke reaches full weight, and how
quickly it lets go, is most of what distinguishes one hand from another
— and here it sits in four numbers you can argue with.

Terminals come for free and honestly: a pointed pen leaves either a flat
cut — the banks joined straight across — or a sharp end, which is
pressure 0, where the banks all but meet. What it never leaves is a
round cap, which is why this idiom deliberately uses no `pen`: `tip` and
`tail` would offer caps the tool being modeled cannot make. The
`copperplate.chi` example in the playground builds the full seven
strokes of the script this way, and its header comment is a design
document worth reading whole.

## The pressure-scaled pen

The other idiom keeps the pen — an ellipse with its aspect and angle —
and scales the *whole nib* with pressure. Since node transforms scale a
pen's dimensions along with everything else, pressure becomes a `scaled`:

```chi
u = 100;
shade = 46;
hair  = 5;

pen brush {
  width  = shade;
  height = 0.35 shade;
  angle  = 20;
}

def press(path, t, pressure) -> n {
  squeeze = (hair + pressure * (shade - hair)) / shade;
  node m = (0, 0) dir direction(path, t) pen brush;
  n = m scaled squeeze shifted point(path, t);
}

spine = (0, 0) .. (2u, 2u) .. (4u, 0) .. (6u, 2u);

a = press(spine, 0, 0);
b = press(spine, 1, 0.22);
c = press(spine, 2, 0.73);
d = press(spine, 3, 1);

fill(a.tailcap .. b.right{b.dir} .. c.right{c.dir} .. d.tipcap
  .. c.left{c.dir + 180} .. b.left{b.dir + 180} .. cycle);
```

Three details carry this example. The node is built at the **origin** and
shifted out afterward, because `scaled` works about the origin — the
transforms chapter's discipline paying off. Width and height shrink
*together*, so the pen's aspect and angle never change: a shut pen still
prints a hairline-sized ellipse, not a point. And the terminals are last
chapter's `tailcap`/`tipcap` splices, which stay exact at every
pressure.

## Choosing between them

Model the tool. Tines that spread: `width`, no pen — thickness lives
across travel, ends are cuts or points. A nib with a body — a brush, a
stub, anything that leaves round ends: a pen scaled by pressure, with
real caps. The two idioms differ by a handful of lines; what they share
is that pressure is *authored*, per sample, as numbers you can read back
with `chi inspect`.

## Where it runs out

Honest limits, all inherited from sampling:

- **Between samples, pressure's shape is the fitter's choice.** The
  banks swell however the spline swells. The only control is more
  samples — which is also the answer's cost.
- **Pressure and curvature interact, silently.** Half the stroke width
  must stay under the spine's radius of curvature, or the inner bank
  folds over itself and the stroke looks gnawed. Real hands obey this
  for free — press on the straight pull, release into the turn — but chi
  has no diagnostic for it yet; the symptom is the shape.
- **The pen's parameters cannot vary per node.** A pen block is
  top-level; `scaled` shrinks it uniformly. A nib whose *aspect* changes
  under pressure — a true pointed pen with a fixed slit axis that also
  bottoms out — is currently not sayable. The copperplate example's
  header discusses exactly where that boundary bites.

That's the whole strokes toolkit. The case studies ahead use all of it,
on real letters.
