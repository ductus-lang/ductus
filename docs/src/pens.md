# Pens

The nodes chapter ended with a broad pen faked by a fixed `nib` axis —
and with the observation that where travel runs along that axis, the
stroke collapses to nothing. A real pen has a body: turn it along its
own axis and it still makes a mark as wide as the nib is *thick*. Chi's
`pen` is exactly that — an ellipse:

```chi norender
pen italic {
  width  = 120;   # full major axis; required
  height = 12;    # full minor axis; optional, default 0
  angle  = 30;    # major-axis orientation, degrees; default 0
}

node m = (x, y) dir theta pen italic;
```

Pen blocks are top-level, like `params`; the field values are ordinary
expressions (they may reference params, so a pen interpolates across the
design space); and the name is a first-class value that can be passed to
macros. A node takes **either** `width` (optionally with `nib`) **or**
`pen`, never both.

A pen is *not* stroke expansion. It is a rule for deriving a node's edge
points — `n.left` and `n.right` become the ellipse's contact points with
the stroke's two banks — and contours are still written through those
points, outline-first, like everything in chi.

## The family

Three degenerate cases cover most uses, so it pays to see the ellipse as
a family:

- **`height = 0`** — a *razor*: `pen x { width = w; angle = phi; }` on a
  node behaves exactly like `width w nib phi`. This is the fixed-axis
  nib from last chapter, now with a name.
- **small `height`** — a *realistic razor*: where travel becomes
  parallel to the pen's axis, stroke thickness bottoms out at `height`
  instead of collapsing to zero.
- **`height = width`** — a *monoline* pen: a circle, equally thick in
  every direction of travel.

The same C-stroke under all three:

```chi
u = 100;

pen razor { width = 80; angle = 25; }
pen soft  { width = 80; height = 14; angle = 25; }
pen round { width = 46; height = 46; }

def arc(cx, p) -> outline {
  node n = (cx, 2.4u)        dir left  pen p;
  node w = (cx - 1.2u, 1.2u) dir down  pen p;
  node s = (cx, 0)           dir right pen p;

  outline = n.right .. w.right .. s.right
         -- s.left .. w.left .. n.left -- cycle;
}

fill(arc(1.4u, razor));
fill(arc(4.2u, soft));
fill(arc(7u, round));
```

The razor stroke (left) pinches toward nothing partway down — its banks
are the skeleton translated by a constant vector, so even three sparse
samples reproduce the collapse exactly. The monoline stroke (right) is
even everywhere. And the middle stroke, with the elliptical pen that
should bottom out at 14 units… stays fat through the crossing. That is
not the pen misbehaving. It is a sampling problem, and it is the most
important thing to understand about pens.

## Thin crossings need a witness

Where the travel direction crosses the pen's major axis, the true edge
points swing to the other end of the ellipse within a few degrees of
travel — the swing zone is roughly `atan(height / width)` wide. The
fitter, as always, only knows the on-curve points the source writes.
Banks fitted between two nodes that merely *bracket* the crossing cut
straight across at full width; the pinch happens between the samples,
so no sample witnesses it.

The cure is the standard chi cure — a feature narrower than the sampling
must be witnessed by a point — and the pens machinery makes the witness
easy to place. The crossing is where the skeleton's tangent equals the
pen angle, and `directionpoint` finds exactly that, tracking it across
the design space:

```chi
u = 100;

pen soft { width = 80; height = 14; angle = 25; }

node n = (u, 2.4u)  dir left  pen soft;
node w = (-0.2u, 1.2u) dir down pen soft;
node s = (u, 0)     dir right pen soft;

skel = n.pos .. w.pos .. s.pos;
node m = directionpoint(skel, 205) dir 205 pen soft;   # 205 = 25 + 180

fill(n.right .. {m.dir} m.right .. w.right .. s.right
  -- s.left .. w.left .. {m.dir + 180} m.left .. n.left -- cycle);
```

Traveling top → left → bottom, the tangent passes 205° — the pen's axis,
pointed down-left — between `n` and `w`, and that is where `m` lands.
Its banks are exactly `height` apart, and the direction constraints make
the waist tangent to travel. Compare the middle stroke of the family
figure: same pen, same skeleton, one more node.

This mirrors placing points at extrema in ordinary outline design. The
razor never needed it because its banks are constant translates — a
special property of the degenerate pen, and the reason faked broad pens
feel simpler than they are.

## The edge points, precisely

For the curious — everything above works without this. For semi-axes
`a = width/2`, `b = height/2` and a node traveling at `dir`, the edge
points are the ellipse's *support points* along the left normal: with
`ψ = dir + 90 − angle`,

```text
h       = sqrt(a²·cos²ψ + b²·sin²ψ)        # half thickness across travel
x*      = rotate(angle) · (a²·cos ψ, b²·sin ψ) / h
n.left  = n.pos + x*
n.right = n.pos − x*
```

With `height > 0`, `left` is always the true left bank of travel — the
contact point of the rolling ellipse, so the banks written through these
points are envelope edges, not approximations. The spec carries the
derivation.

## Joints

Two strokes meeting at an angle make a joint, and both of its sides come
from machinery you already have. The *outer* shoulder is the pen itself,
traced between the two travel directions: place extra nodes at the same
position with intermediate directions and write through them. The
*inner* corner is where the two inner banks cross — `intersection()`:

```chi
u = 100;

pen main { width = 80; height = 16; angle = 30; }

node a  = (0, 0)    dir right pen main;
node k1 = (2.2u, 0) dir right pen main;
node kj = (2.2u, 0) dir 45    pen main;
node k2 = (2.2u, 0) dir up    pen main;
node b  = (2.2u, 3u) dir up   pen main;

inner = intersection(a.left -- k1.left, k2.left -- b.left);

fill(a.right -- k1.right {right} .. kj.right .. {up} k2.right
  -- b.right -- b.left -- inner -- a.left -- cycle);
```

Three nodes share the corner position: `k1` still traveling right, `k2`
already traveling up, and `kj` halfway between at 45°. Their `right`
points fan around the outside — the pen rolling around the corner — while
on the inside, one computed point does the whole job. For a wider turn,
add more intermediate directions; `0.5[t1, t2]` from the expressions
chapter is the natural way to name them.

Both ends of that stroke are still cut flat. Rounding them off — tracing
the pen around a terminal — is the next chapter, along with what happens
when the pen is too flat for tracing to survive.
