# Transforms

Transforms are postfix operators — they read left to right, after the
thing they transform, and they chain:

```chi norender
p shifted (dx, dy)
p scaled k            # uniform, about the origin
p xscaled k
p yscaled k
p rotated theta       # degrees, counterclockwise, about the origin
p slanted k           # (x, y) -> (x + k*y, y)
p reversed            # paths only: reverse the direction of travel
p rotated 10 shifted (50, 0)    # first rotate, then shift
```

They apply to points, paths — and nodes, which Part III introduces. One
shape, put through its paces:

```chi
u = 100;
f = (0, 0) -- (1.5u, 0) .. (2u, u) .. (1.5u, 2u) -- (0, 2u) -- cycle;

fill(f);
fill(f scaled 0.5 shifted (3u, 0));
fill(f slanted 0.25 shifted (4.5u, 0));
fill(f rotated 90 shifted (9.5u, 0));
```

## About the origin

`scaled`, `rotated`, `xscaled`, `yscaled` and `slanted` all work about
the **origin** — there is no "rotate about a point" variant. The idiom
is the chain: build the shape at the origin, transform, then `shifted`
into place. (Going the other way — shift to the origin, transform, shift
back — is the same chain read backwards.) The figure above leans on this:
`f` sits at the origin precisely so the later transforms behave.

`slanted k` is the italic transform: every point moves right by `k`
times its height, verticals become slopes, horizontals stay put. A slant
of 0.25 is about 14 degrees.

## Transforms preserve the fit's input, not its output

A transformed path is the transformed *points and constraints* — the
curve is refitted afterward. For everything except non-uniform scaling of
circles (the previous chapter's gotcha) this is exactly what you want: a
rotated `o` is the `o` fitted in its rotated position, not a rotated
approximation of it. Direction constraints rotate along; a `{up}`
constraint on a path `rotated 90` comes out as `{left}`.

## reversed

`reversed` flips a path's direction of travel: points in reverse order,
entering and leaving constraints swapped. On a drawn spine nothing
visible changes — direction matters when the path is *filled*. Chi fills
with the nonzero winding rule, so a counter must run opposite to the
contour around it; when the chapter on nodes derives both banks of a
stroke from one skeleton, `reversed` is how the inner bank becomes a
proper counter. Until then, file it away: same ink, opposite orientation.

## What transforms cost

Nothing. A path value stores points; a transform maps them once. The
sheet-level meaning is unchanged too: transforms happen in glyph space,
before rendering — they know nothing about the sheet layout, metrics, or
other glyphs.
