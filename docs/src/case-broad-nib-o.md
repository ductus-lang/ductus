# A broad-nib o

The tool-first study: an `o` whose entire character comes from one pen.
No per-node widths, no contrast expressions — an ellipse held at an
angle decides where the stroke is thick, where it is thin, and how thin
"thin" gets. This is the pens chapter's machinery in production:
a closed skeleton, two witness nodes, two contours.

## Pen and skeleton

```chi norender acc
params {
  # Below 45 the upper witness sits too asymmetrically in its leg and
  # the inner bank folds — see "Working the axes". The range is the fix.
  pen_angle  = 45 in 45..60;
  pen_width  = 105 in 70..150;
  pen_height = 12 in 2..40;
}

metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; }

pen nib {
  width  = pen_width;
  height = pen_height;
  angle  = pen_angle;
}
```

All three axes belong to the *pen*: its angle, its breadth, and how far
it is from a razor. The glyph itself has almost nothing to say —
four nodes on a counterclockwise skeleton, all holding the same pen:

```chi acc glyph=o debug
glyph o : U+006F {
  advance = 560;
  c = (advance / 2, x / 2);
  rx = 190;
  ry = x / 2 + 10;

  node b = c + (0, -ry) dir right pen nib;
  node r = c + (rx, 0)  dir up    pen nib;
  node t = c + (0, ry)  dir left  pen nib;
  node l = c + (-rx, 0) dir down  pen nib;

  guide skel = b.pos .. r.pos .. t.pos .. l.pos .. cycle;

  # The travel sweeps every direction once, so it crosses the pen's
  # axis twice: once at pen_angle, once at pen_angle + 180. Without a
  # node at each crossing the banks would bridge the thin spots fat.
  node w1 = directionpoint(skel, pen_angle) dir pen_angle pen nib;
  node w2 = directionpoint(skel, pen_angle + 180) dir pen_angle + 180 pen nib;

  contour outer =
      b.right .. {w1.dir} w1.right .. r.right .. t.right
   .. {w2.dir} w2.right .. l.right .. cycle;

  contour inner =
      b.left .. {w2.dir + 180} w2.left .. l.left .. t.left
   .. r.left .. {w1.dir + 180} w1.left .. cycle;
}
```

Everything in this study is in those two `directionpoint` lines and the
two contours. The witnesses sit where the skeleton's tangent equals the
pen's axis — the debug view shows them as the two extra nodes between
the compass points — and their banks are exactly `pen_height` apart, so
the ring pinches to the pen's true minimum there and nowhere else. The
contours follow the nodes chapter's winding rule: `right` points in
skeleton order for the counterclockwise outer, `left` points in reverse
order (witnesses included, constraints reversed) for the clockwise
counter.

The result, plain:

```chi glyph=o
```

## Working the axes

Because the witnesses are *queries*, not coordinates, they track every
axis. Steepen the pen and the thin spots migrate around the ring;
plump the pen toward a stub and the pinch relaxes — same source, no
edits:

```chi glyph=o set="pen_angle=60"
```

```chi glyph=o set="pen_height=34"
```

## The range is part of the design

Two constraints hide in that `params` block, and both are the
interpolation chapter's lessons in the field. The first is existence:
a witness lives only while the skeleton's tangent sweep covers the pen
angle — on this closed skeleton the sweep is the full circle, so every
`pen_angle` satisfies both queries. Cut the skeleton open (a `c`, say)
and that stops being free; `latin_set.chi` documents exactly this
negotiation in its header.

The second was found the honest way, by building the instances. Below
`pen_angle = 45`, the upper crossing lands close enough to the top node
that the inner bank's fitted curve folds over itself — a black teardrop
hangs into the counter, with no error anywhere, because nothing is
wrong except the shape. (The debugging chapter's field guide calls this
one out.) More skeleton samples around the crossing would widen the
safe range; narrowing the *declared* range to `45..60` is the cheaper
fix, and it keeps the constraint written down where the axis is
declared. Either way the range ends up meaning something specific:
this is where the construction is known to hold, because someone
looked.
