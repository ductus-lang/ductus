# Debugging and inspecting

Ductus is pretty good about pointing out errors, giving you clear file, line, and column details, and its renderer certainly never stays quiet about problems. But let's be honest, often the real head-scratcher isn't an error message at all. It's when your ink compiles perfectly but just doesn't *look* right. That's where these trusty tools come in: the debug view, the probe bindings you've seen in the language chapters, and the playground's dynamic live loop. This chapter will show you how to make the most of each.

## Reading the debug view

```ductus hide
params { weight = 400 in 100..900; }
metrics {
  upm = 1000; x = 500; cap = 700; asc = 750; desc = -220;
  over = 10 + weight / 200;
}
stem = 40 + 0.14 * weight;
hair = 0.85 * stem;

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

  waist = distance(directionpoint(outer, 45),
                   directionpoint(inner, 225));
}
```

```ductus glyph=o debug
```

Here's everything `ductus build --debug` (or the playground's debug toggle) will draw for you, laid out from back to front:

- **The grid** — Think of it as your trusty graph paper, marked out in font units. You'll see fine lines every 10 units, heavier ones every 50, and the heaviest lines every 100. There's also a major line  pinned to the baseline and to each glyph's origin, making it easy to read coordinates directly.
- **Metrics lines** — These helpful horizontal lines mark out every renderer-meaningful metric, like `x`, `cap`, `asc`, `desc`, and overshoot. Each is labeled at the margin, and the baseline stands out with a heavier line.
- **Guides** — Your `guide` paths appear as graceful gray spines, unfilled, tracing the paths you've laid out.
- **Nodes** — You'll spot a bright pink marker for each skeleton node, complete with an arrow to show its `dir`. Plus, there's a blue dot for every derived point, like `left`, `right`, and `tip`/`tail` when the node has a pen. If a stroke bank looks a bit off, compare it against these dots first. Remember, the dots are exact, while the bank between them is fitted.
- **On-curve points and handles** — Here's where the rubber meets the road. Every point the contours pass through is shown, along with the fitted control handles as thin gray struts. This is where the fitter's decisions truly become visible.
- **Anchors** — Anchors pop up as  labeled mauve markers, just like the `top` you see above the ring.

This careful layering helps you answer that age-old question — *is the problem my skeleton, my derived points, or the fit?* You can check each step in turn: is the guide looking right, are the node dots where they should be, and then, finally, take a close look at those handles.

## Failure modes and their looks

Consider this your little field guide to common mishaps. Everything on it appears somewhere earlier in this book — and more than one entry was met the hard way while the book's own figures were being drawn:

- **A fat crossing where a stroke should pinch** — Oh, that frustrating moment when a stroke should be  pinching, but instead you get a chunky crossing. This often means a thin crossing without a witness node. If the debug view shows no node dot anywhere near the waist, the `directionpoint` witness from the pens chapter is usually the perfect fix.
- **An inner bank that folds over itself** — Sometimes an inner bank gets a little ambitious and folds right over itself. This typically happens when the stroke width pushes past the skeleton's radius of curvature, a classic trap from the pressure chapter. You'll see the outline crossing itself near the tightest turn. The solution is often to add more samples or reduce the weight in that tricky spot.
- **A cap that bulges past its stroke** — Ever notice a cap looking a bit too proud, bulging past its stroke? That's usually a traced terminal on a flat pen. The fix is to simply switch to `tipcap`/`tailcap`.
- **A counter that fills solid** — When a counter decides to go rogue and fill itself solid, it's often because both contours are winding in the same direction. The renderer will actually warn you about this one, saying `nested contours share a winding direction`. The solution, as covered in the nodes chapter, is to reverse the inner traversal.
- **A curve that ignores its constraint** — If a curve seems to be having a mind of its own and ignoring a constraint, it's often a simple matter of a direction being placed on the wrong side of the point. Remember, `{theta} p` constrains *entering* the point, while `p {theta}` constrains *leaving* it.

## Probing numbers

While the debug view helps you spot *where* things are, probe bindings are your go-to for figuring out *how big* they are. Any measurement worth checking is certainly worth a binding. It costs just one line of code, shows up  in `ductus inspect` and the playground inspector, and even tracks along with your sliders:

```ductus norender
glyph o {
  ...
  waist = distance(directionpoint(outer, 45),
                   directionpoint(inner, 225));  # stroke width at 45°
}
```

(A quick heads-up on that `225`: because the inner contour travels the other way, the point opposite the outer's 45° tangent will carry the reversed tangent.)

With `ductus inspect file.ductus --set weight=900`, you can get a precise readout of your design's measurements at a specific instance. Even better, the playground brings these numbers to life, letting you drag `weight` and watch `waist` respond in real-time. This transforms a vague suspicion like "doesn't the stroke get too thin in the black?" into a clear graph you can explore.

## The loop

So, what's the recommended workflow for putting all these tools to good use? It goes something like this: first, run `ductus check` for syntax every time you save your work. Then, you'll spend most of your time shaping in the playground (or using `ductus build --debug` with an SVG viewer that reloads). Finally, before you call a glyph done, you'll use probes alongside `ductus inspect --set …` to thoroughly test the corners of your design space. This is exactly where those existence failures from the previous chapter tend to surface. The case studies that follow will put all these practices into action on real letters.
