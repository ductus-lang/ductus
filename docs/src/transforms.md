# Transforms

Transforms are like little helpers that come *after* the thing they're transforming. They're postfix operators, meaning they read left to right, and you can string them together to create a whole sequence of changes:

```ductus norender
p shifted (dx, dy)
p scaled k            # uniform, about the origin
p xscaled k
p yscaled k
p rotated theta       # degrees, counterclockwise, about the origin
p slanted k           # (x, y) -> (x + k*y, y)
p reversed            # paths only: reverse the direction of travel
p rotated 10 shifted (50, 0)    # first rotate, then shift
```

These handy operations work their magic on points, paths, and even nodes – we'll get to those in Part III. To give you a taste, let's watch one shape go through a little transformation journey:

```ductus
u = 100;
f = (0, 0) -- (1.5u, 0) .. (2u, u) .. (1.5u, 2u) -- (0, 2u) -- cycle;

fill(f);
fill(f scaled 0.5 shifted (3u, 0));
fill(f slanted 0.25 shifted (4.5u, 0));
fill(f rotated 90 shifted (9.5u, 0));
```

## About the origin

`scaled`, `rotated`, `xscaled`, `yscaled`, and `slanted` all have a special relationship with the **origin**. They don't offer a direct "rotate about a specific point" option. Instead, the trick is to build your shape right at the origin, apply your transformations, and then use `shifted` to move it exactly where you want it. (Thinking about it in reverse – shifting to the origin, transforming, then shifting back – is just the same  chain in reverse.) You can see this principle in action with our example above: `f` starts its life at the origin, which sets it up perfectly for all those cool transformations.

`slanted k` gives us that elegant italic effect. It gently nudges every point to the right by `k` times its height. This means vertical lines will  lean into slopes, while horizontal lines stay comfortably in place. For instance, a `slanted` value of 0.25 gives you a tilt of about 14 degrees.

## Transforms preserve the fit's input, not its output

When you transform a path, you're actually transforming its underlying *points and constraints*. The curve then gets a fresh fitting to these new positions. This approach is usually exactly what you'd hope for, with one quirky exception: non-uniform scaling of circles (a little gotcha we met in the previous chapter). For just about everything else, it's perfect: a `rotated` `o` is a truly `o`-shaped curve nestled in its rotated spot, not just a wobbly, rotated copy. Even your direction constraints come along for the ride; a `{up}` constraint on a path that's `rotated 90` will happily become a `{left}` constraint.

## reversed

`reversed` offers a neat trick: it simply flips a path's direction of travel. This means the points are reordered in reverse, and any entering and leaving constraints get swapped around. If you just draw a spine, you won't see any immediate difference – the direction really comes into play when the path is *filled*. Ductus uses the nonzero winding rule for filling, which requires a 'counter' path to run in the opposite direction to the contour surrounding it. Later on, when we explore nodes and derive both sides of a stroke from a single skeleton, `reversed` will be the key to ensuring the inner bank forms a perfectly proper counter. For now, you can tuck this little tidbit away: it's the same beautiful ink, just heading in the opposite direction.

## What transforms cost

Curious about the cost of all these wonderful transformations? The answer is a  'nothing at all.' A path value simply holds onto its points, and a transform just maps them once. On the sheet level, everything remains  consistent as well. These transformations take place within glyph space, well before any rendering happens, so they don't concern themselves with sheet layout, metrics, or other glyphs.
