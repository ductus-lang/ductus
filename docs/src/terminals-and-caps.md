# Terminals and caps

Ductus doesn't include automatic caps, and that's a conscious design choice, not something we've missed. We believe a terminal should be treated like any other part of an outline. Whether you envision a stroke's end as a clean flat cut, a gracefully traced round cap, a soft teardrop, or even a subtle ink trap, you'll write it directly into the contour. This approach ensures it remains editable point by point and fully parametric. This chapter provides a detailed look at how you can achieve these.

```ductus hide
u = 100;
pen main { width = 90; height = 36; angle = 20; }
pen flat { width = 130; height = 10; angle = 110; }
```

## Cuts

Meet the terminal you've likely been creating all along: where the two banks connect straight across, using either `-- ` or the closing `cycle`. Because you're in complete control of this cut line, its angle is also entirely up to you. Simply extending one corner, for instance, makes the cut turn oblique. And with a broad-pen node, the cut naturally follows the direction of the nib. Here, we'll demonstrate three uprights, each with a different kind of end:

```ductus
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

The first example demonstrates a plain, straightforward cut. In the second, we've raised one corner by 55 units, giving us an oblique cut—all accomplished within a single expression. The third example introduces a powerful new capability: it *traces the pen*'s distinctive shape.

## Tracing the pen: .tip and .tail

Pen nodes provide you with two more handy points: `n.tip` and `n.tail`. `n.tip` represents the ellipse's support point right along the direction of travel—essentially, the front of the pen. Conversely, `n.tail` is its mirror image, found at the back. (A quick note: attempting to use these on razor or `width`/`nib` nodes will result in errors, as a zero-height pen doesn't have a distinct front or back.) The ellipse's tangent at these points is always perpendicular to the direction of travel. This crucial detail means that when you're crafting a terminal that traces the pen, you know the direction constraints for its three key points before you even begin to draw.

```ductus norender
... -- b.right {b.dir} .. {b.dir + 90} b.tip .. {b.dir + 180} b.left -- ...
```

To visualize this: let the right bank continue its journey with the stroke, then gently round the front as it turns left, and finally arrive on the left bank traveling backwards. A `.tail` cap applied at the *start* of a stroke works as a perfect mirror image of this sequence. You'll see this precise idiom in action in the `va_pen.ductus` example, where it's used for every stroke end.

```ductus norender
... a.left {a.dir + 180} .. {a.dir - 90} a.tail .. cycle;
```

## Where tracing breaks: flat pens

When you use a traced cap, you're essentially providing the fitter with three specific points and asking it to construct half an ellipse. For a nicely plump pen, this task is usually quite straightforward. But things get a little tricky when you're working with more eccentric pens. Imagine, for example, a realistic razor pen where the `height` is only a tenth of its `width`, and it's held with its long axis perpendicular to the direction of travel. This creates a very broad stroke with ends that are nearly flat. In these situations, the three support points tend to cluster tightly at the pen's tips, forcing the fitter to attempt a ninety-degree turn across an almost-zero chord. The visual result often isn't what you'd hope for: you might see a sharp corner where the cap should curve, and an unexpected bulge where it ought to lie flat:

```ductus
node a = (u, u)              dir 20 pen flat;
node b = a.pos + 1.8u * unitvec(20) dir 20 pen flat;

fill(a.right -- b.right {b.dir} .. {b.dir + 90} b.tip
  .. {b.dir + 180} b.left -- a.left {a.dir + 180}
  .. {a.dir - 90} a.tail .. cycle);

node c = a.pos + (0, 1.4u)   dir 20 pen flat;
node d = c.pos + 1.8u * unitvec(20) dir 20 pen flat;

fill(c.tailcap .. d.tipcap .. cycle);
```

Take a close look at the lower stroke; it demonstrates the standard traced idiom. If you inspect its ends, you'll see how the cap slightly overshoots the pen's true footprint. Now, for the fix, observe the upper stroke. It introduces **`.tipcap` and `.tailcap`**, which provide cap geometry constructed *analytically* instead of being fitted. Each of these returns a small path—the exact half-ellipse from one bank contact, around the front (or back), to the other. They carry their own fixed curve segments, making them wonderfully immune to eccentricity. It means that no matter how flat the pen, the cap will perfectly reflect its true shape.

## Splicing

Using `c.tailcap` in that contour revealed one more clever path feature: splicing. When a path value is used as an *element* within a larger path expression, it essentially splices itself in. Its points and segments seamlessly merge into the host path, and any joints you define on either side will connect smoothly to the splice's endpoints. The great thing is that the cap's boundary points inherently carry their exact tangents along with them. This means there's no need to explicitly write anything at the seam, allowing the entire outline of a stroke to be wonderfully concise, like this:

```ductus norender
fill(a.tailcap .. b.tipcap .. cycle);
```

— a straightforward combination of two caps and the two bank curves that the fitter smoothly connects between them. Just a quick note: you can only splice open paths, not closed ones. While caps are a prime example and a common use case, any open path value will splice in the very same fashion.

## Joints

A joint is a terminal's close relative. Where two strokes meet at a corner, the pen pivots: it stands on the shared point while the direction of travel rotates from the first stroke's to the second's. On the outer side of the turn, its boundary sweeps an arc between the two bank contacts — the same silhouette a cap traces, only spanning the turn of the joint instead of a half turn. You could trace that arc with direction constraints, and for a plump pen it works just as well as it did for caps. It also breaks in exactly the same place: a flat pen turns almost all of its corner within a fraction of a degree of pivot, and the fitted curve bulges far off the metal. So joints get the analytic treatment too. **`jointcap(a, b)`** takes two nodes declared at the same position with the same pen — `a` arriving at the corner, `b` leaving it — and returns the exact arc as fixed curve segments, ready to splice.

The arc comes oriented for the traversal we've used throughout: out along the right bank, back along the left. On a left turn the outer corner lies on the right bank, and the arc runs from `a`'s right contact to `b`'s. On a right turn it lies on the left bank, and the arc runs from `b`'s left contact back to `a`'s. Either way, it drops in right where the outer banks end. The inner side of a joint needs no arc at all: the two banks simply cross there, and `intersection()` pins the corner. Here is a peaked stroke pair written with the same flat pen that defeated the traced cap earlier — note how the joint carries the pen's true silhouette, in tune with both terminals:

```ductus
d1 = 72; d2 = -55;                       # a 127-degree turn at the apex
apex = (2.6u, 2.35u);

node s1 = apex - 2.3u * unitvec(d1) dir d1 pen flat;
node e1 = apex dir d1 pen flat;
node s2 = apex dir d2 pen flat;
node e2 = apex + 2.3u * unitvec(d2) dir d2 pen flat;

inner = intersection(s1.right -- e1.right, s2.right -- e2.right);

fill(s1.tailcap -- inner -- e2.tipcap -- jointcap(e1, s2) -- cycle);
```

Two caps, one joint arc, one inner corner, four straight banks: the entire two-stroke figure is a single expression. The `N-caps-joints.ductus` playground example scales this up to an English N — two terminals and two joints, neither of them perpendicular. And to be clear about the boundaries: `jointcap` wants a genuine corner. Equal directions have no turn to round, and a full 180-degree reversal has no outer side — that one is simply a cap, which you already know how to write.

## Teardrops, torn ends, traps

Anything else you can imagine a terminal to be will simply be a creative combination of moves we've already explored in this book. And that's exactly the beauty of not having rigid cap machinery. For example, a sharp break between two curves comes to life with `{corner}` from the first-strokes chapter. If you're envisioning a teardrop terminal, that's just a swell traced gracefully around a corner point. An ink trap can be fashioned by pulling an inner corner open—perhaps by nudging an `intersection()` point into the joint. These are all straightforward edits that remain completely parametric. Our catalogue comes to a close here, and we're intentionally stopping short. Past this point, terminals evolve into exciting design decisions that are entirely yours to make, rather than being dictated by fixed language features.