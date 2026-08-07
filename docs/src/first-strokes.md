# First strokes

Before you dive into glyphs, metrics, and advance widths, there's a wonderfully straightforward way to get ductus up and running: we call it a sketch. If your file doesn't include a `glyph` block, it's considered a sketch file. In these files, a top-level `draw()` call will render your paths as thin spines, and `fill()` will fill in any closed shapes you've made. Sketches are perfect for when you want to doodle around, quickly test out an idea, or simply follow along with the examples in this chapter. Feel free to paste any example into the [playground](https://github.com/ductus-lang/ductus-playground), or you can run it directly from a file:

```sh
ductus build sketch.ductus -o sketch.svg
```

## Numbers, names, comments

Each statement you write in ductus should end with a semicolon. If you want to add a helpful note, just use `#` to start a comment. And when you need to give something a name, the `=` symbol is your friend. It's worth noting that bindings are single-assignment here. This means if you try to rebind a name within the same scope, ductus will gently let you know it's an error. This design choice ensures that every value in your file can be traced back to a single, clear line.

Here's a neat trick: placing a number directly in front of an expression automatically multiplies it. This means once you've set up a unit, your coordinates will start to feel delightfully intuitive, almost like working on graph paper. For instance, `3u` simply means three units, and `2(a + b)` gives you twice the sum.

```ductus
u = 100;                   # our unit
draw((0, 0) -- (4u, 2u));  # a line segment
```

The coordinate system is Cartesian and y-up. This means x grows to the right and y grows upwards, just like the mathematics you're familiar with, and quite unlike most screen APIs. By the way, `--` is how you join points with a straight line.

## Points are values

In ductus, `(x, y)` creates a point. These points are quite versatile: you can add them, subtract them, and scale them with ease. What's more, a point also happily doubles as a 2-D vector, which makes calculating displacements a delightfully straightforward arithmetic task:

```ductus
u = 100;
a = (0, 0);
b = (6u, 2u);
m = 0.5[a, b];             # halfway from a to b

draw(a -- b);
draw(m + (-u/4, 0) -- m + (u/4, 0));   # a cross marking m
draw(m + (0, -u/4) -- m + (0, u/4));
```

Meet `t[a, b]`, an operation we call *mediation*. Chances are, this will quickly become one of your most frequently used expressions in ductus. It elegantly finds a point (or number) `t` of the way from `a` to `b`. So, if you're looking for the exact midpoint, `0.5[a, b]` will do the trick. The best part is, `t` can be any expression you dream up.

## Curves

Want to create a beautiful curve? Just connect your points with `..` instead of `--`, and ductus will gracefully fit a smooth path right through them. This is truly the heart of the language's approach to curves: you simply place your on-curve points, and a sophisticated two-parameter spline solver — a clever descendant of the one John Hobby crafted for METAFONT — intelligently selects every control handle for you, ensuring the curve flows smoothly through each and every point.

```ductus
u = 100;
p = (0, 0) .. (2u, 2u) .. (4u, 0) .. (6u, 2u);
draw(p);
```

You'll notice that no handles ever appear in your source code, and they never will. If a curve isn't quite shaping up how you envisioned, your instinct won't be to fiddle with handles. Instead, you'll simply add another point exactly where you feel the geometry needs a bit more definition, or perhaps you'll constrain its direction.

> **Try it** — Feel encouraged to paste this wave example into the playground and have some fun pushing those numbers around. Try raising a crest, adding a fifth point, or even deleting one. Don't worry, you really can't break anything. The fitter is wonderfully responsive, re-solving the curve with every keystroke, and watching it gracefully adapt is truly the quickest way to build a rapport with it.

## Directions

Sometimes you want a curve to head in a specific direction. You can constrain its tangent at a point by enclosing a direction in braces. For example, `{45}` tells the curve to leave (or enter) at a 45-degree angle. We use degrees for angles, measured counterclockwise, with 0° pointing to the right. Handy named bindings like `right`, `up`, `left`, and `down` are also available, representing 0, 90, 180, and 270 degrees respectively. If you place a direction specifier *before* a point, it constrains the curve *entering* that point; *after* a point, it guides the curve *leaving* it.

```ductus
u = 100;
a = (0, 0);
b = (4u, 0);

p = a .. (2u, u) .. b;              # the fitter's choice
q = a {up} .. (2u, u) .. {down} b;  # forced vertical ends

draw(p);
draw(q shifted (5u, 0));            # beside it, to compare
```

Notice how the wave on the left follows the fitter's gentle, natural route. The wave on the right, however, is beautifully constrained, forced to gracefully leave point `a` straight up and then elegantly dive into `b` straight down. (By the way, `shifted` is just one of ductus's handy transform operators – we'll dedicate an entire chapter to them later.)

## Mixing lines and curves

You'll be pleased to know that `--` for lines and `..` for curves can be mixed together quite freely within a single path. When a line gracefully meets a curve, that intersection point effortlessly transforms into a natural line/curve corner. The curve will arrive perfectly tangent to whatever direction it needs, while the line, of course, remains wonderfully straight.

```ductus
u = 100;
fill((0, 0) -- (3u, 0) .. (4u, u) .. (3u, 2u) -- (0, 2u) -- cycle);
```

The `cycle` keyword thoughtfully closes your path, bringing it right back to its starting point. It's important to remember that closedness is always explicitly written, never just inferred. And speaking of closed paths, `fill()` naturally requires one to work its magic.

## Corners

What if you want a point to be distinctly sharp *between two curves*? You can achieve this by marking it with `{corner}`. When you do, the fitter intelligently treats each side of that point independently, which means you won't get any automatic smoothness across it.

```ductus
u = 100;
wave    = (0, 0) .. (u, u) .. (2u, 0) .. (3u, u) .. (4u, 0);
scallop = (0, 0) .. (u, u) .. (2u, 0){corner} .. (3u, u) .. (4u, 0);

draw(wave);
draw(scallop shifted (5u, 0));
```

We're using the very same five points in both examples. On the left, you can see how the fitter keeps the curve beautifully smooth right through the middle point, allowing it to gracefully swing through a rounded valley. On the right, however, that middle point becomes a deliberate corner. Here, each side is fitted entirely independently, causing the two arches to end quite steeply, meeting in a distinct cusp. Corners like these — strategically placed where two curves should visibly break apart — are truly the seeds for crafting every teardrop terminal and ink trap we'll explore in Part III.

Here's a little confession, with a valuable lesson tucked inside: in two earlier drafts of this very example, I tried marking the tip of a teardrop as a corner, and absolutely nothing changed. The shape stubbornly remained a smooth egg both times. The trick is, at a perfectly symmetric point, the tangents that the `{corner}` specifier frees up can simply choose to align perfectly with what the smooth ones would have done anyway. Think of `{corner}` as granting the fitter the *freedom* to break smoothness; it's the surrounding geometry that ultimately decides whether that freedom actually makes a visible difference.

## Reading the sheet

Everything you draw in ductus gracefully appears together on a single sheet. If you ever want a peek behind the curtain, simply pass `--debug` to `ductus build` (or toggle the debug option in the playground), and ductus will thoughtfully draw a coordinate grid right underneath your work. Our debug figures throughout this book also take advantage of this, just like this rendering of our earlier wave:

```ductus hide
u = 100;
draw((0, 0) .. (2u, 2u) .. (4u, 0) .. (6u, 2u));
```

```ductus debug
```

This grid is essentially graph paper, measured in font units. You'll see fine lines every 10 units, heavier ones every 50, and the heaviest lines marking every 100 units. So, if you look at our wave, its crests at `y = 200` are comfortably sitting two heavy squares above the baseline. For sketches, the sheet simply and conveniently sizes itself to whatever you've drawn. A real glyph, however, as we'll discover in the next chapter, derives its vertical extent from font metrics — which, along with baselines, heights, and all those other helpful indicators, is truly what the debug view is designed for.