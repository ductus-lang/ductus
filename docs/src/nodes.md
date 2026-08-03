# Nodes and derived points

Up until now, we've been crafting outlines point by point. But here's the thing: type is fundamentally about *strokes*. Think of an `o` as a lovely round stroke, or an `l` as a crisp straight one. Each stroke carries its own direction and thickness, long before it takes on its final outline. And in chi, a **node** is how we elegantly capture all of that information:

```chi norender
node n = (x, y) dir theta width w;
```

Essentially, a node is a special skeleton point that holds vital information about an imaginary stroke. It knows the local direction of travel (`dir`, measured in degrees) and the stroke's total thickness (`width`). It's a foundational concept, but it never actually appears on its own. Instead, its true power comes from its ability to *derive* other points, like so:

```chi norender
n.pos      # the point itself
n.dir      # the direction, a number
n.left     # n.pos + (w/2) * unitvec(n.dir + 90)
n.right    # n.pos - (w/2) * unitvec(n.dir + 90)
```

Think of *left* and *right* like the banks of a river, always relative to your direction of travel. If you're standing on the skeleton facing `dir`, `n.left` will be half the stroke's width to your left. No fancy offset-curve magic happening here; these derived points are simply born from straightforward arithmetic. What you see in those formulas above is exactly how it works.

## A first stroke

So, how do we actually create outlines with these clever points? We write them *through* the derived points. Just two nodes are enough to define a beautiful tapered stroke. Imagine a skeleton running down the middle, with one bank stretching out, and the other coming back.

```chi
u = 100;
node a = (u, u)      dir 55 width 90;
node b = (5u, 2.5u)  dir 10 width 30;

draw(a.pos {a.dir} .. {b.dir} b.pos);       # the skeleton
draw(a.left {a.dir} .. {b.dir} b.left);     # each bank echoes it
draw(a.right {a.dir} .. {b.dir} b.right);
```

These banks are simply ordinary curves, passing through ordinary points. The neat trick is that they intelligently reuse the node's direction as their own constraint, which is precisely what makes them glide so gracefully parallel to the skeleton. Now, let's close off those ends, and voilà, we have a complete contour:

```chi
u = 100;
node a = (u, u)      dir 55 width 90;
node b = (5u, 2.5u)  dir 10 width 30;

fill(a.right {a.dir} .. {b.dir} b.right
  -- b.left {b.dir + 180} .. {a.dir + 180} a.left -- cycle);
```

Picture this path: we journey out along the right bank, then take a straight shot across the far end, cruise back along the left bank (notice how every direction is cleverly reversed with `+ 180`), and finally, complete the loop by crossing the near end with `cycle`. Those two neat, straight crossings are what we call *cut terminals*. They're the most straightforward way a stroke can end, and we'll dive much deeper into them in a dedicated chapter later on.

It's important to notice what chi *didn't* do here. It didn't perform any complex curve offsetting. Instead, it precisely offset four *points* and then skillfully fitted curves right through them. If a truly parallel curve and our fitted bank ever have a minor disagreement, the solution is delightfully simple, just like many other things in chi: we simply add a witness point. The pens chapter will explain this in fine detail.

## Asymmetric width

By default, `width w` beautifully distributes the stroke evenly, assigning `w/2` to each bank. But what if you need more control? `width (wl, wr)` lets you specify the banks separately. In fact, `width w` is just a neat shorthand for `width (w/2, w/2)`. This lovely asymmetry is precisely how we articulate off-axis stress in our designs. Let's observe the same elegant C-shaped stroke, first with its weight perfectly centered, and then with that weight playfully pushed towards the outer bank:

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

Notice that both of these strokes maintain a thickness of 60 units, and their underlying skeletons are absolutely identical. However, on the right, you'll see almost the entire stroke extends beyond its skeleton. This clearly illustrates that the skeleton doesn't always have to sit perfectly in the middle of the stroke, and there's no rule saying it must.

## The nib axis

Now, let's explore one more intriguing clause. Normally, width spreads out perpendicular to the direction of travel, as shown by `unitvec(dir + 90)` in our earlier formulas. But with `nib phi`, you gain the delightful ability to override that default axis:

```chi norender
node k = (x, y) dir theta width w nib phi;
# k.left = k.pos + wl * unitvec(phi)  — phi, not theta + 90
```

Imagine a *fixed* nib axis, one that remains steadfast regardless of the stroke's travel direction. This effectively simulates a broad pen: the stroke becomes wonderfully thick when it travels across the axis, and elegantly thin when it glides along it. No complex pen machinery needed here, just the same familiar bowl, but this time with every node's nib axis gracefully pinned to 30°:

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

Take a look at the upright stroke nestled next to our bowl. It beautifully showcases another signature of broad-pen behavior: its cut ends neatly align with the nib axis, perfectly capturing the entry angle a broad pen would naturally leave. The bowl, however, subtly hints at a fascinating challenge. Somewhere along its path, between two nodes, the travel direction crosses the nib axis. At precisely that point, a true stroke would gracefully thin to *nothing*. The good news is, the upcoming pens chapter will teach us how to articulate this behavior perfectly.

## Building the o

Let's finally build the 'o' glyph that greeted us at the very beginning of this book, starting from scratch. First, we lay down the essential scaffolding: our parameters, metrics, and the definitions for the two key stroke weights:

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

`stem` defines our lovely thick stroke, while `hair` handles the delicate thin one. And `over`? That's our clever overshoot value, determining just how much a round shape needs to gently extend beyond the flat metrics to *appear* perfectly aligned.

Our skeleton is beautifully formed by four nodes arranged on an ellipse, gracefully traveling counterclockwise. It moves across the top going leftward, sweeps down the left side, glides along the bottom heading rightward, and finally rises up the right side. Horizontal movements are assigned the `hair` thickness, while vertical movements take on the `stem` thickness. This simple distinction is the secret to all the delightful contrast in our design.

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

The debug view gives us a clear peek at what we've crafted so far: four nodes, each proudly displaying its direction arrow, along with its two cleverly derived edge points, and the graceful guide line connecting them all. There are no contours just yet, so nothing would actually render in a typical build. Those radii are quite special and worth a closer look. They're actually *mid-stroke* radii, thoughtfully pulled in by half a stroke's thickness. This ensures that the very extremes of the outline, which are half a stroke further out, land perfectly on our defined overshoot line.

Now, with just two elegant path expressions, we transform our scaffolding into beautiful ink:

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

And here's the magic: because every single number in our design gracefully traces back to our initial parameters, the entire design space comes along for the ride, absolutely free. For example, setting `weight = 850` gives us:

```chi glyph=o set="weight=850"
```

Or perhaps a low contrast look at `contrast = 1.0`, where `hair` and `stem` charmingly become equal:

```chi glyph=o set="weight=850 contrast=1.0"
```

We've created eight distinct points and two flowing contours, all dynamically at every instance. This truly embodies the 'skeleton-as-scaffolding' principle introduced in our first chapter, bringing it vividly to life. You defined just four positions and four widths, and the nodes, in their wisdom, effortlessly positioned everything else.

> **Try it** — the playground ships this `o` among its examples. Go ahead, drag `weight` and `contrast` to their limits in the playground and watch the debug view. You'll see the edge-point dots gracefully slide, the guide line gently breathe and reshape, and yet, not a single point will ever mysteriously appear or disappear.

## The winding convention

Let's take a closer look at our two contours, shall we:

```chi norender
contour outer = n.right .. w.right .. s.right .. e.right .. cycle;
contour inner = n.left  .. e.left  .. s.left  .. w.left  .. cycle;
```

Our skeleton journeys along in a pleasing counterclockwise direction. This means the `right` bank naturally forms the outside of our ring, while the `left` bank gracefully defines the inside. Notice that the outer contour follows the exact order of our skeleton — `n, w, s, e` — and it too is counterclockwise. Now, for the inner contour, we smartly walk the *left* points in **reverse** order — `n, e, s, w`. This clever reversal makes it clockwise, which is precisely the orientation the nonzero fill rule expects for a perfect counter. So, here's a handy rule to tuck away for future reference:

> Traverse a closed skeleton counterclockwise: the `right` points in
> order form the outer contour, and the `left` points in reverse order
> form the counter.

(If you prefer, you could also build both banks in skeleton order and then simply apply the `reversed` transform from the dedicated transforms chapter to the inner one. But by writing it out in reverse, with each direction implicitly handled as we've done here, there's no need for an explicit reversal.)

## The designer owns every point

Here's a liberating thought: derived points are, at heart, just ordinary points. This means you can do wonderful things: `w.right + (3, 0)` will gently nudge the left flank of the `o` three units to the right, all without disturbing the original node. You could even entirely replace `s.right` with a carefully hand-placed point, which is a perfectly normal edit. And because a nudge is itself an expression, it can be beautifully parametric, just like everything else in chi:

```chi norender
contour outer = n.right .. w.right + (3, 0) .. s.right .. e.right .. cycle;
```

Nodes themselves are also quite transformable. Functions like `shifted`, `rotated`, and their companions will gracefully move the node's position and, at the same time, adjust its direction (and even its nib axis). `scaled`, of course, thoughtfully scales the width as well. So, when a macro like our `bowl()` example returns nodes, it generously hands the caller everything they might need: the position, the direction, the thickness, and both derived banks, all perfectly ready to be either artfully written through or, if inspiration strikes, confidently overruled.