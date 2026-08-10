# Parameters, metrics, and bindings

A ductus file finds its structure through three distinct forms for blocks and bindings. You'll use `params` to declare all the variables in your design, `metrics` to name those essential font-level measurements, and plain bindings to derive everything else. This chapter also dives into how you can read values back *out* of your design. It's ductus's elegant answer to questions like, "how thick did that stem actually turn out?"

## params

```ductus norender acc
params { size = 300 in 100..460; }

metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; }
```

When you define an entry in `params`, you're essentially creating a design axis. Each one needs a name, a default value, and a range specified with `in`. Within the ductus language itself, a parameter acts just like a regular number binding. What makes it special, though, is its ability to be overridden *from outside* your source file, allowing for unique instances:

```sh
ductus build dot.ductus --set size=460
```

The ductus playground automatically generates handy sliders from this block, providing one for each parameter and respecting its declared range. When ductus evaluates your file, it follows a clear, single-pass process. First, it handles your parameters — applying defaults, then any `--set` overrides. Next up are the metrics, and after that, the rest of your file is evaluated from top to bottom.

## metrics

The renderer pays special attention to five particular metric keys. These are `upm`, `x`, `cap`, `asc`, and `desc`, which together define the overall sheet size and help draw those useful debug guidelines, just as we explored in the first-glyph chapter. Any other key you include here is simply a named number that comfortably lives alongside these essential ones. Metric values behave just like ordinary expressions, and they can even reference your parameters. This means defining something like a weight-dependent overshoot can be as simple as a single line:

```ductus norender
metrics {
  upm = 1000; x = 500;
  over = 10 + weight / 200;   # heavier weights overshoot more
}
```

## Bindings are the measuring instrument

A plain binding is how you derive a value; there's no separate "measure" keyword to worry about. But here's where numeric bindings serve a dual purpose: ductus is designed to **report them directly to tooling**. Top-level bindings are reported in source order, and a glyph's own bindings appear right alongside its advance. Since ductus doesn't include a traditional print statement, these bindings become your only way to retrieve a computed number from your source file. This powerful feature means that a binding applied to geometry acts as a *probe*:

```ductus acc
glyph dot {
  advance = 500;
  center = (advance / 2, x / 2);
  r = size / 2;

  contour main = circle(center, r);
  bulge = distance(point(main, 0.2), center) - r;
}
```

Here, `bulge` gets right into measuring the fitted contour itself. It tells us precisely how far a point, one fifth of the way into the first segment, deviates from a perfect circle. (By the way, `circle()` is a shape constructor that provides a simple path value with four points; you can explore its relatives in the reference chapter.) Notice that nothing actually uses `bulge` within the file; its sole purpose is to be observed and understood.

## ductus inspect

When you run `ductus inspect`, it thoroughly evaluates your file and then thoughtfully reports every number contained within. This includes your parameters and their ranges, all your metrics, every binding, and for each glyph, you'll see its advance, any probes you've defined, and the point counts for each contour:

```text
$ ductus inspect dot.ductus
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

So, that `bulge` value of 0.0407 font units, when compared to a radius of 150, precisely indicates the fitter's 0.027 % circle error. And yes, all of this is measured right from within the ductus language itself. You'll only find *numeric* bindings appearing in this list. For instance, `center` is a point, so it's reported as part of your geometry (like anchors and contours) rather than showing up in this number table. Happily, `--set` works here too, providing a great way to quickly check a derived value across your design space:

```ductus set="size=460"
```

The figure you see above displays the very same file, but this time with `--set size=460` applied. If you were to run `ductus inspect dot.ductus --set size=460` yourself, you'd observe `r` at 230 and `bulge`  scaled up right alongside it, still maintaining that consistent 0.027 %.

> **Try it** — load this file in the playground and drag `size` while
> watching the inspector. `bulge` moves, but its share of `r` never
> does. Before probes existed, checking this number meant parsing the
> output SVG with a Python script; the probe exists so that ductus can
> measure ductus.

## Immutability, and why

Here's an important principle: every single binding in ductus — whether it's a parameter, a metric, a top-level declaration, or glyph-local — is designed for single-assignment. Trying to rebind a name within the same scope will actually result in an error. Of course, inner scopes, like those within glyph bodies or macro bodies, are allowed to shadow names from outer scopes. When you combine this immutability with ductus's single-pass evaluation, it makes reasoning about your ductus file  straightforward. Any value you see can be traced back to one specific line, evaluated just once, using values that were clearly defined above it. So, if something ever seems amiss, you know there's exactly one place you need to investigate.
