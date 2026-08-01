# Parameters, metrics, and bindings

Three block-and-binding forms give a chi file its structure: `params`
declares what varies, `metrics` names the font-level measures, and plain
bindings derive everything else. This chapter is also about reading
values back *out* — chi's answer to "how thick did that stem actually
come out?".

## params

```chi norender acc
params { size = 300 in 100..460; }

metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; }
```

Each entry in `params` is a design axis: a name, a default, and a range
after `in`. Inside the language a param is simply a number binding —
what makes it an axis is that it can be overridden *from outside* the
source, per instance:

```sh
chi build dot.chi --set size=460
```

The playground generates its sliders from this block, one per param, with
the declared range. Evaluation order is fixed and single-pass: params
first (defaults, then any `--set` overrides), then metrics, then the rest
of the file top to bottom.

## metrics

Five metric keys are meaningful to the renderer: `upm`, `x`, `cap`,
`asc`, `desc` size the sheet and draw the debug guidelines, as the
first-glyph chapter showed. Every other key is just a named number that
happens to live with them. Metric values are ordinary expressions and may
reference params — a weight-dependent overshoot is one line:

```chi norender
metrics {
  upm = 1000; x = 500;
  over = 10 + weight / 200;   # heavier weights overshoot more
}
```

## Bindings are the measuring instrument

A plain binding derives a value; there is no separate "measure" keyword.
But numeric bindings do double duty: chi **reports them to tooling** —
top-level ones in source order, and a glyph's own bindings alongside its
advance. Since chi has no print statement, a binding is the only way to
read a computed number back out of a source file. That makes a binding
over geometry a *probe*:

```chi acc
glyph dot {
  advance = 500;
  center = (advance / 2, x / 2);
  r = size / 2;

  contour main = circle(center, r);
  bulge = distance(point(main, 0.2), center) - r;
}
```

`bulge` measures the fitted contour itself: how far a point one fifth
into the first segment strays from a perfect circle. (`circle()` is a
shape constructor — a plain path value with four points; the reference
chapter covers its siblings.) Nothing uses `bulge`; it exists to be read.

## chi inspect

`chi inspect` evaluates a file and prints every number in it — params
with their ranges, metrics, bindings, and each glyph's advance, probes,
and per-contour point counts:

```text
$ chi inspect dot.chi
params
  size                     300  in 100..460
metrics
  asc                      750
  cap                      700
  desc                    -220
  upm                     1000
  x                        500

glyph dot
  advance                  500
  bulge                 0.0407
  r                        150
  contour main        4 points
```

That `bulge` of 0.0407 font units on a radius of 150 is the fitter's
0.027 % circle error, measured from inside the language. Only *numeric*
bindings appear — `center` is a point, so it is reported by geometry
(anchors, contours), not in the number table. `--set` works here too,
which is how you check a derived value across the design space:

```chi set="size=460"
```

The figure above is the same file at `--set size=460`; running
`chi inspect dot.chi --set size=460` would show `r` at 230 and `bulge`
scaled up with it, still 0.027 %. The playground's inspector panel shows
the same numbers live, following the sliders.

## Immutability, and why

Every binding — param, metric, top-level, glyph-local — is
single-assignment; rebinding a name in the same scope is an error. Inner
scopes (glyph bodies, macro bodies) may shadow outer names. Combined with
single-pass evaluation this keeps a file boring to reason about, in the
best sense: any value traces to one line, evaluated once, from values
defined above it.
