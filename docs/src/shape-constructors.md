# Shape constructors

Instead of meticulously plotting points one by one, ductus offers four  built-in functions to conjure up common closed shapes. What you get back are **ordinary path values** — complete with on-curve points and tangent constraints, with the fine-tuning of handles  left to the fitter. This means these shapes are incredibly versatile: you can transform them, query their paths, and ultimately, anything they craft, you could also painstakingly draw by hand if you truly wished:

```ductus
u = 100;

fill(circle((u, u), 0.8u));
fill(ellipse((3.2u, u), 1.1u, 0.65u));
fill(roundrect((4.8u, 0.3u), (6.4u, 1.7u), 30));
fill(superellipse((8u, u), 0.8u, 0.8u, 4, 16));
```

These four handy constructors share a couple of key traits, both carefully considered design choices:

- **Counterclockwise**: We've set them up this way so the shapes are immediately ready to serve as an outer contour. If you need a counter (an inner hole), simply pop `reversed` on the call.
- **Consistent Point Count**: Here's a neat trick: the number of points defining your shape depends *only* on the constructor you choose, never on any specific value you feed it. This ensures your shape  maintains its topology throughout the entire design space, perfectly aligning with ductus's clever [interpolation rule](interpolation-compatibility.md).

## circle(c, r) — 4 points

Imagine placing a point right at each axis extreme, with tangents  pinned. That's how `circle` works its magic. Because ductus's fitter is wonderfully precise with circular arcs, these four points are more than enough to describe a circle with astonishing accuracy — we're talking a mere 0.027 % of `r`, an error you'd never spot at any typical type size.

## ellipse(c, rx, ry) — 8 points

You might notice `ellipse` uses eight points, not four, and it's a distinct constructor rather than simply scaling a `circle(...)`. There's a good reason for both. While four points are plenty for a circle, they only capture an ellipse to about 1% accuracy. That's because the fitter's handle rule is absolutely spot-on for circles, but not quite for ellipses — it's a little [gotcha](paths-and-the-fitter.md#the-one-gotcha-squashed-circles) you'll find in the fitter chapter. So, to give you a  precise ellipse, `ellipse` thoughtfully allocates eight points, achieving a much tighter 0.06%. And here's another key detail: using `ellipse(c, r, r)` is intentionally *not* the same as `circle(c, r)`. If ductus were to cleverly switch between them when `rx == ry`, it would unfortunately alter the point count as an axis crossed that 1.0 threshold, messing with your design space consistency.

For those really flat ellipses, you might want to consider spending a few more points. Happily, `superellipse(c, rx, ry, 2, count)` offers the very same elliptical curve, but lets you pick your desired point count. For instance, with a dramatic 1:5 aspect ratio, you'll see a precision jump from 3.9% with eight points, to a much nicer 0.6% with sixteen, and an impressive 0.05% with thirty-two.

## roundrect(p, q, r) — 8 points

`roundrect` takes two opposite corners (you can give them in any order) and a corner radius to create its charming shape: four straight lines meeting four gentle quarter-arcs. Just a heads-up: the radius is cleverly clamped to half the shorter side of the rectangle. Go beyond that, and the neighboring arcs would start bumping into each other. Even if you set `r = 0`, it's still a rectangle that *carries* eight points — though four of its segments magically become zero-length. This might seem a bit curious, but it's crucial: by maintaining eight points instead of shrinking to a four-point path, your roundrect keeps its consistent point count, even if a parameter smoothly drives the radius all the way down to zero across your design space:

```ductus
u = 100;
fill(roundrect((0, 0), (1.6u, 1.6u), 0));
fill(roundrect((2.2u, 0), (3.8u, 1.6u), 24));
fill(roundrect((4.4u, 0), (6u, 1.6u), 60));
fill(roundrect((6.6u, 0), (8.2u, 1.6u), 500));   # clamped to 80
```

There's a little consequence of those zero-length segments: since they don't have a tangent, path queries won't be able to "see" them. For example, if you ask for a `directionpoint` on a radius-0 roundrect, you'll find points along the main sides, but never on those perfectly sharp, degenerate corners.

## superellipse(c, rx, ry, n, count)

Here we have the wonderfully versatile superellipse, defined by the family `|x/rx|^n + |y/ry|^n = 1`. Think of it as a spectrum of shapes: when `n = 2`, you get a classic ellipse, but as you increase `n`, the shape  squares off, pushing outward toward its bounding box. You can use any expression for `n` as long as it's ≥ 2 (values below that would lead to an error here, as this family isn't defined for the diamond-like shapes). And since `n` primarily shifts *positions* rather than changing the fundamental structure, it makes a perfect candidate for a parameter — imagine defining a whole 'squareness' axis for your type design.

```ductus
u = 100;
fill(superellipse((u, u),    0.8u, 0.8u, 2, 16));
fill(superellipse((3.2u, u), 0.8u, 0.8u, 4, 16));
fill(superellipse((5.4u, u), 0.8u, 0.8u, 10, 16));
```

The `count` argument determines how many points are sampled along the superellipse. You'll need at least 4 points, and you can go up to 1024; it's also a multiple of 4, ensuring a point always lands perfectly on each axis extreme. We've made `count` an explicit argument for a very good reason: it ensures it can *never* silently change on you. As we detail in the interpolation chapter, passing a count derived from a parameter is a classic 'trapdoor' for unexpected behavior. Here, it's the sampling process, not the fitter, that truly limits accuracy. For instance, sixteen points keep `n = 4` accurate to a lovely 0.15% of the side, but only achieve 0.69% at `n = 10`. It's a good rule of thumb that both flatter and squarer shapes will generally sing with a few more points.

## Constructors in glyphs

When you bring these constructors inside a glyph, they truly shine as contour factories. And remember our friend `reversed`? That's your go-to for creating counters, or inner holes. Take a peek at this washer example, where both the outer edge's squareness and the inner hole are  parametric:

```ductus
metrics { upm = 1000; asc = 750; desc = -220; }

glyph washer {
  advance = 500;
  c = (250, 250);

  contour rim  = superellipse(c, 210, 210, 3, 16);
  contour hole = circle(c, 90) reversed;
}
```

Just a quick note: all those handy [winding rules](debugging.md) from the debugging chapter still apply here, precisely as you'd expect. These constructors kindly give you a counterclockwise path, and if you need to turn it into a lovely hole, `reversed` is truly all you need.
