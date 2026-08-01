# Nodes and derived points

Everything so far drew outlines point by point. But type is made of
*strokes* — an `o` is a round stroke, an `l` a straight one — and a
stroke has a direction and a thickness before it has an outline. A
**node** is how chi says that:

```chi norender
node n = (x, y) dir theta width w;
```

A node is a skeleton point that knows the imaginary stroke's local
direction of travel (`dir`, degrees) and total thickness (`width`). It
never renders by itself. What it does is *derive*, exactly:

```chi norender
n.pos      # the point itself
n.dir      # the direction, a number
n.left     # n.pos + (w/2) * unitvec(n.dir + 90)
n.right    # n.pos - (w/2) * unitvec(n.dir + 90)
```

*Left* and *right* are relative to the direction of travel, like a
river's banks: stand on the skeleton facing `dir`, and `n.left` is half
the width to your left. There is no offset-curve machinery behind this —
the derived points are plain arithmetic, and the formulas above are the
whole story.

## A first stroke

Outlines are written *through* the derived points. Two nodes make a
tapered stroke: skeleton down the middle, one bank out, one bank back.

```chi
u = 100;
node a = (u, u)      dir 55 width 90;
node b = (5u, 2.5u)  dir 10 width 30;

draw(a.pos {a.dir} .. {b.dir} b.pos);       # the skeleton
draw(a.left {a.dir} .. {b.dir} b.left);     # each bank echoes it
draw(a.right {a.dir} .. {b.dir} b.right);
```

The banks are ordinary curves through ordinary points — they reuse the
node's direction as their own constraint, which is what makes them run
parallel to the skeleton. Close the ends and it is a contour:

```chi
u = 100;
node a = (u, u)      dir 55 width 90;
node b = (5u, 2.5u)  dir 10 width 30;

fill(a.right {a.dir} .. {b.dir} b.right
  -- b.left {b.dir + 180} .. {a.dir + 180} a.left -- cycle);
```

Out along the right bank, straight across the far end, back along the
left bank with every direction reversed (`+ 180`), straight across the
near end via `cycle`. Those two straight crossings are *cut terminals* —
the simplest ends a stroke can have, and a chapter of their own comes
later.

Note what chi did **not** do: it never offset a curve. It offset four
*points* and fitted curves through them. Where a true parallel curve and
the fitted bank would disagree, the cure is the same as everywhere else
in chi — add a witness point — and the pens chapter makes that precise.

## Asymmetric width

`width w` splits the stroke evenly: `w/2` to each bank. `width (wl, wr)`
gives the banks separately — `width w` is just `width (w/2, w/2)` — and
that asymmetry is how off-axis stress is written. The same C-shaped
stroke, weight centered and weight pushed to the outer bank:

```chi
u = 100;

def bowl(cx, wl, wr) -> outline {
  node n = (cx, 2.4u)        dir left  width (wl, wr);
  node w = (cx - 1.2u, 1.2u) dir down  width (wl, wr);
  node s = (cx, 0)           dir right width (wl, wr);

  outline = n.right .. w.right .. s.right
         -- s.left .. w.left .. n.left -- cycle;
}

fill(bowl(1.5u, 30, 30));
fill(bowl(4.5u, 5, 55));
```

Both strokes are 60 units thick, and their skeletons are identical. On
the right, nearly all of it lies outside the skeleton — the skeleton is
no longer the middle of the stroke, and nothing requires it to be.

## The nib axis

One more clause. By default width is applied perpendicular to travel —
`unitvec(dir + 90)` in the formulas above. `nib phi` overrides that
axis:

```chi norender
node k = (x, y) dir theta width w nib phi;
# k.left = k.pos + wl * unitvec(phi)  — phi, not theta + 90
```

A *fixed* nib axis, independent of travel, is a broad pen: the stroke is
thick where travel runs across the axis and thin where travel runs along
it. No pen machinery — the same bowl with every node's nib pinned to 30°:

```chi
u = 100;

def bowl(cx) -> outline {
  node n = (cx, 2.4u)        dir left  width 70 nib 30;
  node w = (cx - 1.2u, 1.2u) dir down  width 70 nib 30;
  node s = (cx, 0)           dir right width 70 nib 30;

  outline = n.right .. w.right .. s.right
         -- s.left .. w.left .. n.left -- cycle;
}

fill(bowl(1.5u));

node a = (4u, 0)    dir up width 70 nib 30;
node b = (4u, 2.4u) dir up width 70 nib 30;
fill(a.right -- b.right -- b.left -- a.left -- cycle);
```

The upright stroke beside the bowl shows the other broad-pen signature:
its cut ends run along the nib axis, giving the entry angle a broad pen
leaves. And the bowl hints at a problem — somewhere between two nodes
the travel direction crosses the nib axis, and there the true stroke
should thin to *nothing*. The pens chapter is about saying that
properly.

## Building the o

Now the glyph this book opened with, from nothing. The scaffolding first
— params, metrics, and the two stroke weights:

```chi norender acc
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
```

`stem` is the thick stroke, `hair` the thin one, `over` the overshoot —
how far a round shape must poke past the flat metrics to *look* aligned
with them.

The skeleton is four nodes on an ellipse, traveling counterclockwise:
across the top leftward, down the left side, along the bottom rightward,
up the right side. Horizontal travel gets `hair`, vertical travel gets
`stem` — that is all the contrast in the design:

```chi glyph=o debug
glyph o : U+006F {
  advance = 2 stem + 380;
  center = (advance / 2, x / 2);

  # mid-stroke radii: the outline's extremes land on x + over and -over
  ry = x / 2 + over - hair / 2;
  rx = advance / 2 - 55 - stem / 2;

  node n = center + (0,  ry) dir left  width hair;
  node w = center + (-rx, 0) dir down  width stem;
  node s = center + (0, -ry) dir right width hair;
  node e = center + (rx, 0)  dir up    width stem;

  guide spine = n.pos .. w.pos .. s.pos .. e.pos .. cycle;
}
```

The debug view shows exactly what exists so far: four nodes with their
direction arrows, each with its two derived edge points, and the guide
running through them. No contours yet — nothing would render in a normal
build. The radii deserve a second look: they are *mid-stroke* radii,
pulled in by half a stroke so that the outline's extremes — half a
stroke further out — land exactly on the overshoot line.

Two path expressions turn the scaffolding into ink:

```chi acc glyph=o debug
glyph o : U+006F {
  advance = 2 stem + 380;
  center = (advance / 2, x / 2);
  ry = x / 2 + over - hair / 2;
  rx = advance / 2 - 55 - stem / 2;

  node n = center + (0,  ry) dir left  width hair;
  node w = center + (-rx, 0) dir down  width stem;
  node s = center + (0, -ry) dir right width hair;
  node e = center + (rx, 0)  dir up    width stem;

  guide spine = n.pos .. w.pos .. s.pos .. e.pos .. cycle;

  contour outer = n.right .. w.right .. s.right .. e.right .. cycle;
  contour inner = n.left  .. e.left  .. s.left  .. w.left  .. cycle;

  anchor top = (advance / 2, x + over);
}
```

Without the debug overlay:

```chi glyph=o
```

And because every number traces back to the params, the whole design
space came along for free — `weight = 850`:

```chi glyph=o set="weight=850"
```

and low contrast at `contrast = 1.0`, where `hair` equals `stem`:

```chi glyph=o set="weight=850 contrast=1.0"
```

Eight points, two contours, at every instance. This is the
skeleton-as-scaffolding principle from the first chapter made concrete:
the nodes place sixteen numbers' worth of geometry, and the designer
wrote four widths and four positions.

## The winding convention

Look again at the two contours:

```chi norender
contour outer = n.right .. w.right .. s.right .. e.right .. cycle;
contour inner = n.left  .. e.left  .. s.left  .. w.left  .. cycle;
```

The skeleton travels counterclockwise, so the `right` bank is the
outside of the ring and the `left` bank is the inside. The outer contour
follows skeleton order — `n, w, s, e` — and is counterclockwise. The
inner contour walks the *left* points in **reverse** order — `n, e, s,
w` — which makes it clockwise: exactly what the nonzero fill rule needs
a counter to be. This is the rule to memorize:

> Traverse a closed skeleton counterclockwise: the `right` points in
> order form the outer contour, and the `left` points in reverse order
> form the counter.

(Equivalently: build both banks in skeleton order and apply the
transforms chapter's `reversed` to the inner one. Written out in reverse
with each direction implicit, as here, there is nothing to reverse.)

## The designer owns every point

Derived points are ordinary points. `w.right + (3, 0)` nudges the left
flank of the `o` three units without touching the node; replacing
`s.right` entirely with a hand-placed point is an ordinary edit; and
because a nudge is an expression, it can be parametric like everything
else:

```chi norender
contour outer = n.right .. w.right + (3, 0) .. s.right .. e.right .. cycle;
```

Nodes also transform — `shifted`, `rotated` and friends move the
position and turn the direction (and the nib axis) together, and
`scaled` additionally scales the width. A macro that returns nodes,
like `bowl()` above, therefore hands the caller everything: position,
direction, thickness, and both banks, ready to be written through — or
overruled.
