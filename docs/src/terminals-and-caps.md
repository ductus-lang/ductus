# Terminals and caps

Chi has no automatic caps, and that is a principle, not a gap: a
terminal is outline like any other. Whatever a stroke's end should be —
a flat cut, a traced round cap, a teardrop, an ink trap — it is written
into the contour, stays editable point by point, and stays parametric.
This chapter is the catalogue.

```chi hide
u = 100;
pen main { width = 90; height = 36; angle = 20; }
pen flat { width = 130; height = 10; angle = 110; }
```

## Cuts

The terminal you have been writing all along: the two banks joined
straight across, `-- ` or the closing `cycle`. And because the cut line
is yours, so is its angle — extend one corner and the cut turns oblique;
a broad-pen node's cut already runs along the nib. Three uprights, three
ends:

```chi
def bar(x) -> (a, b) {
  node a = (x, 0)    dir up pen main;
  node b = (x, 2.6u) dir up pen main;
}

(a1, b1) = bar(u);
fill(a1.right -- b1.right -- b1.left -- a1.left -- cycle);

(a2, b2) = bar(2.6u);
fill(a2.right -- b2.right -- b2.left + (0, 55) -- a2.left -- cycle);

(a3, b3) = bar(4.2u);
fill(a3.right -- b3.right {b3.dir} .. {b3.dir + 90} b3.tip
  .. {b3.dir + 180} b3.left -- a3.left -- cycle);
```

The first is the plain cut. The second raises one corner by 55 units —
an oblique cut, one expression. The third is new: it *traces the pen*.

## Tracing the pen: .tip and .tail

Pen nodes derive two more points: `n.tip`, the ellipse's support point
along the direction of travel — the front of the pen — and `n.tail`,
the same thing backwards. (Both are errors on razor and `width`/`nib`
nodes: a zero-height pen has no front.) The ellipse's tangent at those
points is perpendicular to travel, so a terminal that traces the pen is
three points whose direction constraints are known before you draw
anything:

```chi norender
... -- b.right {b.dir} .. {b.dir + 90} b.tip .. {b.dir + 180} b.left -- ...
```

Leave the right bank still traveling with the stroke, round the front
turning left, arrive on the left bank traveling backwards. A `.tail` cap
at the *start* of a stroke is the mirror image — this is the idiom the
`va_pen.chi` example uses at every stroke end:

```chi norender
... a.left {a.dir + 180} .. {a.dir - 90} a.tail .. cycle;
```

## Where tracing breaks: flat pens

The traced cap hands the fitter three points and asks it for half an
ellipse. For a plump pen that is easy. For an eccentric one — here a
realistic razor with `height` a tenth of `width`, held with its long
axis across the travel so the stroke is broad and its ends nearly flat —
the three support points cluster at the pen's tips, and the fit must
turn ninety degrees across a nearly-zero chord. The result renders as a
corner where the cap should turn, and a bulge where it should lie flat:

```chi
node a = (u, u)              dir 20 pen flat;
node b = a.pos + 1.8u * unitvec(20) dir 20 pen flat;

fill(a.right -- b.right {b.dir} .. {b.dir + 90} b.tip
  .. {b.dir + 180} b.left -- a.left {a.dir + 180}
  .. {a.dir - 90} a.tail .. cycle);

node c = a.pos + (0, 1.4u)   dir 20 pen flat;
node d = c.pos + 1.8u * unitvec(20) dir 20 pen flat;

fill(c.tailcap .. d.tipcap .. cycle);
```

The lower stroke is the traced idiom; look closely at its ends — the cap
overshoots the pen's true footprint. The upper stroke uses the fix:
**`.tipcap` and `.tailcap`**, cap geometry constructed *analytically*
instead of fitted. Each returns a small path — the exact half-ellipse
from one bank contact around the front (or back) to the other — carrying
its own fixed curve segments, immune to eccentricity. However flat the
pen, the cap is the pen.

## Splicing

`c.tailcap` in that contour introduced one last path feature: a path
value used as an *element* of a path expression splices in — its points
and segments merge into the host path, and the joints written on either
side connect to the splice's endpoints. The cap's boundary points carry
their exact tangents with them, so nothing needs to be written at the
seam: the whole outline of a stroke can be as short as

```chi norender
fill(a.tailcap .. b.tipcap .. cycle);
```

— two caps and the two bank curves the fitter runs between them. A
closed path cannot be spliced, only open ones; caps are the motivating
case, but any open path value splices the same way.

## Teardrops, torn ends, traps

Everything else a terminal can be is a combination of moves this book
has already made — which is the point of having no cap machinery. A
sharp break between two curves is `{corner}` from the first-strokes
chapter; a teardrop terminal is a swell traced around a corner point; an
ink trap is an inner corner pulled open — an `intersection()` point
nudged into the joint, an ordinary edit that stays parametric. The case
studies in the next part build several of these on real letters; there
is deliberately nothing more to teach here.
