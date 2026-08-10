# Pens

Remember how the nodes chapter wrapped up, showing off a broad pen that was actually just a fixed `nib` axis? We also noted something interesting: if your travel path ran right along that axis, the stroke would just… disappear. Well, a proper pen, the kind you hold, has a distinct body. You can turn it on its own axis, and it will still leave a mark as wide as its nib is *thick*. Ductus's `pen` captures that idea  — it's essentially an ellipse:

```ductus norender
pen italic {
  width  = 120;   # full major axis; required
  height = 12;    # full minor axis; optional, default 0
  angle  = 30;    # major-axis orientation, degrees; default 0
}

node m = (x, y) dir theta pen italic;
```

You'll find pen blocks hanging out at the top level, much like `params`. The values you set for a pen's fields are just regular expressions. This means they can even reference `params`, allowing your pen to  interpolate and adapt across your entire design space. And here's a neat trick: the pen's name itself is a first-class value, ready to be passed around to macros. Just a quick heads-up: when you're defining a node, you can specify **either** its `width` (perhaps with an optional `nib`) **or** a `pen` — but never both at the same time.

Now, it's important to understand that a pen isn't some magical stroke expansion tool. Instead, think of it as a clever rule for figuring out a node's precise edge points. Specifically, `n.left` and `n.right` transform into the exact contact points where our ellipse touches the stroke's two banks. This means you're still drawing contours through those very points, outline-first, just like you do with everything else in ductus.

## The family

While the ellipse is quite versatile, three special, or "degenerate," cases actually cover most of what you'll want to do. It's helpful to think of the ellipse as part of a whole family, with these key members:

- **`height = 0`** — This gives you a *razor* pen. If you define `pen x { width = w; angle = phi; }` and use it on a node, it acts precisely like specifying `width w nib phi`. It's that familiar fixed-axis nib we met in the last chapter, but now it has its very own name.
- **small `height`** — Think of this as a *realistic razor*. When your travel direction aligns perfectly with the pen's axis, the stroke thickness won't vanish entirely. Instead, it  bottoms out at that small `height` value, rather than collapsing to nothing.
- **`height = width`** — Hello, *monoline* pen. This creates a perfect circle, giving you a stroke that's wonderfully consistent and equally thick no matter which way you travel.

Let's see how the same C-stroke looks when we apply all three of these:

```ductus
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

Take a look at the razor stroke on the left: you'll see it  pinches down to nothing partway through. This happens because its banks are essentially just the skeleton shifted by a constant vector, meaning even a few sparse samples are enough to perfectly capture that collapse. Now, move to the monoline stroke on the right, and you'll notice it's wonderfully smooth and even all the way around. But then there's that middle stroke, with our elliptical pen designed to bottom out at 14 units… and yet, it stubbornly stays quite chunky through the crossing. Don't worry, the pen isn't misbehaving at all. This is actually a classic sampling problem, and it's perhaps the most crucial insight to grasp when working with pens.

## Thin crossings need a witness

Here's what happens: when your travel direction sweeps across the pen's major axis, those true edge points actually swing rather dramatically to the ellipse's opposite end. This swing happens within just a few degrees of travel, defining what we call the "swing zone," which is roughly `atan(height / width)` wide. The issue is that the fitter, bless its heart, only sees the on-curve points that you provide in your source. So, if your banks are being fitted between two nodes that simply *bracket* this crucial crossing point, they'll just cut straight across at full width. The subtle pinch is happening right there, in between your samples, completely unwitnessed.

The good news is the cure follows a familiar ductus pattern: any feature that's narrower than your sampling needs a point to witness it. And happily, the pens machinery makes placing that witness remarkably straightforward. The crossing itself occurs precisely where the skeleton's tangent matches the pen's angle. Conveniently, `directionpoint` is designed to find this exact spot, tracking it faithfully across your entire design space.

```ductus
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

As we travel from top to left to bottom, the tangent passes through 205 degrees. This angle aligns with the pen's axis, pointing down-left, and it's precisely where our `m` node finds its perfect home, right between `n` and `w`. At this point, the pen's banks are exactly `height` apart, and those clever direction constraints ensure the waist becomes perfectly tangent to the path of travel. Go back and compare this with the middle stroke in our family figure: same pen, same skeleton, but with just one extra node, and what a difference it makes.

This whole process elegantly mirrors how you place points at extrema when you're working with ordinary outline design. Remember, the razor pen never required a witness because its banks are always just constant translations of the skeleton. That's a unique and special property of that particular degenerate pen. It's easy to get used to this behavior, only to feel a bit baffled the first time a "real" pen leaves a crossing looking a bit thick. If that ever happens to you, consider this your friendly reminder to revisit this section.

## The edge points, precisely

For those of you who love to dive into the nitty-gritty details — just a heads-up, everything we've covered so far works perfectly even if you never look at this part. But for the truly curious, here's how the magic happens: given semi-axes `a = width/2` and `b = height/2`, and a node traveling at a direction `dir`, the edge points are actually the ellipse's *support points* positioned along the left normal. We calculate this using `ψ = dir + 90 − angle`:

```text
h       = sqrt(a²·cos²ψ + b²·sin²ψ)        # half thickness across travel
x*      = rotate(angle) · (a²·cos ψ, b²·sin ψ) / h
n.left  = n.pos + x*
n.right = n.pos − x*
```

When `height` is greater than zero, `n.left` consistently represents the true left bank of travel. This is the precise contact point of our rolling ellipse, which means the banks you draw through these points are authentic envelope edges, not mere approximations. You can find the full derivation in the ductus specification, should you wish to explore it further.

## Joints

When two strokes come together at an angle, they form a joint, and it's quite neat how both sides of this joint are handled by tools you're already familiar with. The *outer* shoulder is  formed by the pen itself, tracing a path between your two travel directions. You achieve this by placing extra nodes at the same position, but giving them intermediate directions, and then simply writing through them. Meanwhile, the *inner* corner is elegantly found right where the two inner banks intersect — a perfect job for `intersection()`.

```ductus
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

Notice how three nodes gather at the corner position: `k1` continues its journey right, `k2` is already heading up, and `kj` elegantly sits halfway between them at 45 degrees. Out on the exterior, their `right` points  fan out, showing the pen smoothly rolling around that corner. But on the inside, a single, precisely computed point takes care of the entire corner for you. If you need a broader, more sweeping turn, simply add more intermediate directions. A handy trick for naming them is using `0.5[t1, t2]`, a concept we explored in the expressions chapter.

You might observe that both ends of this particular stroke are still quite flat. Don't worry,  rounding them off — essentially tracing the pen around a terminal — is exactly what we'll dive into in the next chapter. We'll also explore what happens when your pen gets a little too flat for that tracing to hold up.
