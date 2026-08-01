# Debugging and inspecting

Chi's error messages carry a file, line and column, and its renderer
refuses nothing silently — but most type-design debugging is not about
errors. It is about ink that compiled fine and looks wrong. The tools
for that are the debug view, the probe bindings from the language
chapters, and the playground's live loop; this chapter is how to read
them.

## Reading the debug view

```chi hide
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

```chi glyph=o debug
```

Everything `chi build --debug` (or the playground's debug toggle) draws,
from back to front:

- **The grid** — graph paper in font units: fine lines every 10, heavier
  every 50, heaviest every 100, with a major line pinned to the baseline
  and to each glyph's origin. Read coordinates straight off it.
- **Metrics lines** — one horizontal per renderer-meaningful metric
  (`x`, `cap`, `asc`, `desc`, overshoot), labeled at the margin; the
  baseline is drawn heavier.
- **Guides** — gray spines: every `guide` path, unfilled.
- **Nodes** — a pink marker per skeleton node with an arrow showing
  `dir`, plus a blue dot for each derived point (`left`, `right`, and
  `tip`/`tail` where the node has a pen). If a stroke bank looks wrong,
  compare it against these dots first: the dots are exact, the bank
  between them is fitted.
- **On-curve points and handles** — every point the contours pass
  through, with the fitted control handles as thin gray struts. This is
  where the fitter's decisions become visible.
- **Anchors** — labeled mauve markers, like the `top` above the ring.

The layering answers the standard question — *is the problem my
skeleton, my derived points, or the fit?* — by letting you check each
in turn: guide right? node dots right? then look at the handles.

## Failure modes and their looks

A short field guide, all seen earlier in this book:

- **A fat crossing where a stroke should pinch** — a thin crossing with
  no witness node. The debug view shows no node dot anywhere near the
  waist; the pens chapter's `directionpoint` witness is the fix.
- **An inner bank that folds over itself** — stroke width exceeding the
  skeleton's radius of curvature, the pressure chapter's trap. The
  outline crosses itself near the tightest turn; add samples or shed
  weight there.
- **A cap that bulges past its stroke** — a traced terminal on a flat
  pen; switch to `tipcap`/`tailcap`.
- **A counter that fills solid** — both contours winding the same way.
  The renderer warns about this one (`nested contours share a winding
  direction`); the fix is the nodes chapter's rule, reverse the inner
  traversal.
- **A curve that ignores its constraint** — usually a direction written
  on the wrong side of the point: `{theta} p` constrains *entering*,
  `p {theta}` *leaving*.

## Probing numbers

The debug view answers "where is it"; probe bindings answer "how big is
it". Any measurement worth checking is worth a binding — it costs one
line, it shows up in `chi inspect` and the playground inspector, and it
tracks the sliders:

```chi norender
glyph o {
  ...
  waist = distance(directionpoint(outer, 45),
                   directionpoint(inner, 225));  # stroke width at 45°
}
```

(Note the `225`: the inner contour travels the other way, so the point
opposite the outer's 45° tangent carries the reversed tangent.)

`chi inspect file.chi --set weight=900` then reads the design measuring
itself at that instance. The playground shows the same numbers live —
drag `weight` and watch `waist` move — which turns a suspicion
("doesn't the stroke get too thin in the black?") into a graph you can
scrub.

## The loop

The workflow the tools want, in order: `chi check` for syntax on save;
the playground (or `chi build --debug` and an SVG viewer that reloads)
while shaping; probes plus `chi inspect --set …` at the corners of the
design space before calling a glyph done — the previous chapter's
existence failures surface exactly there. The case studies that follow
practice all of it on real letters.
