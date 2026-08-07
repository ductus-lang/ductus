# Malayalam tha

Meet Malayalam ത (tha), a delightful double bowl character with tips that gently curl inwards. It's a shape utterly devoid of straight lines, generously featuring four distinct counters of white space, and possessing a left/right symmetry that practically begs to be designed just once. This character is a fantastic example of a 'space-first' design approach. Here, the advance is cleverly *derived directly from the counters*, which means the `width` axis gracefully adjusts the white space, rather than simply squashing the strokes. Come along as we explore the actual `tha.ductus` file, pulled straight from our collection of examples.

## Foundation

```ductus norender acc
params {
  weight   = 400 in 100..900;
  contrast = 0.85 in 0.4..1.0;
  width    = 100 in 60..120;
}
metrics {
  upm = 1000; x = 500; cap = 700; asc = 750; desc = -220;
  over = 10 + weight / 200;
}

stem = 40 + 0.14 * weight;
hair = contrast * stem;
term = 0.75 * hair;          # the tips taper before their round caps
```

Take a peek at the third axis: `width`. It's important to remember this isn't about the *stroke weight* (that's what `weight` is for), but rather about how *open* the letter feels.

## Symmetry as a macro

You'll notice right away that the letter's right half is a perfect mirror image of its left. Naturally, we use a macro to mirror a node across a vertical axis. There's just one clever trick to remember: the direction needs to be *negated*. This is because a mirrored node is read by a traversal moving in the opposite direction.

```ductus norender acc
def mirror(n, axis) -> m {
  node m = (2 * axis - n.pos.x, n.pos.y) dir -n.dir width n.width;
}
```

Here, we see the promise from the macros chapter beautifully delivered. A macro gracefully accepts a node, hands back a mirrored node, and then it's entirely up to the caller to decide how to handle its banks.

## Counters first, then the glyph

```ductus acc glyph=tha
glyph tha : U+0D24 {
  bowl = 1.6 * width;             # bowl counter width
  gap  = 0.55 * width;            # counter between the central strokes

  wx  = 45 + stem / 2;            # left outer stroke, centre
  lcx = wx + stem + bowl;         # left central stroke, centre
  rcx = lcx + stem + gap;         # right central stroke, centre
  ex  = rcx + stem + bowl;        # right outer stroke, centre
  advance = ex + stem / 2 + 45;
  mid = 0.5[lcx, rcx];

  ax = 0.5[wx, lcx] + 14;         # tip hovers just right of bowl centre

  # Left half of the skeleton, travelling from the tip into the letter:
  # curl up over the left shoulder, down around the left bowl, in to
  # the centre.
  node a = (ax, 0)                            dir 150   width term;
  node b = (wx, 0.48 * x)                     dir up    width term;
  node c = (0.5[wx, lcx], x + hair / 2 - over) dir right width hair;
  node d = (mid + 2gap, b.pos.y)              dir down  width hair;
  node e = (mid, over)                        dir left  width hair;

  # Right half by symmetry.
  f = mirror(d, mid);
  g = mirror(c, mid);
  h = mirror(b, mid);
  i = mirror(a, mid);

  guide skeleton = a.pos .. b.pos .. c.pos .. d.pos .. e.pos
                .. f.pos .. g.pos .. h.pos .. i.pos;

  # Round caps by hand: one extra point past each tip, half a width
  # out along the travel direction, tangents pinned to a semicircle.
  acap = a.pos - (a.width / 2) * unitvec(a.dir);
  icap = i.pos + (i.width / 2) * unitvec(i.dir);

  contour main =
       {a.dir} a.right {a.dir}
    .. b.right .. c.right .. d.right .. e.right .. f.right .. g.right
    .. h.right
    .. {i.dir} i.right {i.dir}
    .. icap {i.dir + 90}
    .. {i.dir + 180} i.left {i.dir + 180}
    .. h.left .. g.left .. f.left .. e.left .. d.left .. c.left
    .. b.left
    .. a.left {a.dir + 180}
    .. acap {a.dir + 270}
    .. cycle;
}
```

Let's unpack three key insights, presented in order of their significance.

**The advance is a sum of counters and strokes.** Take a close look at the `wx → lcx → rcx → ex` chain. You'll observe that each stroke center is carefully positioned by adding the previous center, a stroke's width, and then a *counter* (`bowl` or `gap`). This means the letter is quite literally constructed as alternating black and white spaces. So, when the `width` axis shifts, every counter expands or contracts gracefully, while each stroke holds its intended weight. It's a wonderfully precise approach, completely unlike naive scaling.

**One contour, eighteen-plus points, still just two banks.** Our skeleton elegantly snakes from tip to tip, passing through nine distinct nodes. The contour then traces its path outward along the right bank, gracefully curves around the right cap, and returns along the left bank, before completing its journey around the left cap. It’s the very same shape we explored with the pressure chapter's strokes, now brought to life at letter scale.

**The caps are ordinary points.** It's interesting to note that these are simply `width` nodes. You won't find a dedicated pen, or any special `tip` or `tipcap` features here. Instead, a round cap is thoughtfully constructed from its fundamental essence: a point positioned half a width beyond the tip (look for `acap` and `icap`), with its entry and exit precisely guided along pinned semicircle tangents. This adherence to Ductus's 'no-cap-machinery' principle means we achieve this with just two bindings and four constraints. And, as a delightful bonus, the taper into the cap (using `term`, which is thinner than `hair`) was completely free for us to design exactly as we wished.

Our construction view offers a clear glimpse of the skeleton, gracefully snaking its way through both bowls, alongside the helpful node dots that precisely place every single bank point.

```ductus glyph=tha debug
```

## Across the space

When we adjust the `weight`, we're directly influencing the ink; meanwhile, the counters faithfully maintain their elegant proportions:

```ductus glyph=tha set="weight=150"
```

```ductus glyph=tha set="weight=850"
```

Now, for the `width` axis, it's all about moving *white space*. At `width=60`, for instance, the bowls gently tighten, and the letter condenses beautifully, all without a single stroke thinning out. This is why the black version of the same instance remains wonderfully credible: black and white are truly independent design elements here.

```ductus glyph=tha set="width=60"
```

```ductus glyph=tha set="width=120 weight=700"
```

Among the three studies we've looked at, this one feels most aligned with real-world practice. In complex script design, a glyph often comes down to a delightful negotiation between its various counters. Having a source format where the advance is *explicitly expressed* as that negotiation ensures this vital information remains clearly on the page, ready for the next designer to easily understand and appreciate.