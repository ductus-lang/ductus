# Interpolation compatibility

We talked about the rule before, and now it's time to put it to work. For any two instances within your parameter space, it's essential that **every contour yields the same number of Bézier segments with the same point types**. This consistent structure is precisely what interpolation between masters requires—think of point 3 of a light `o` needing to correspond perfectly to point 3 of a black `o`. It's also the fundamental promise that our future designspace sampler will rely on when it's ready to cut masters from a single chi source.

## Why you mostly can't break it

Good news: Chi is designed to prevent you from breaking this rule almost entirely. It's baked right into the core design of the language itself. The number and type of on-curve points in a contour are determined by your source text, not by the values of your parameters. When you adjust a parameter, you're moving coordinates, never changing the underlying topology. We've thoughtfully shaped every construct that could potentially threaten this rule to keep it safe:

- Shape constructors inherently have fixed point counts. For instance, an `ellipse()` will always result in eight points. That's why you'll find `ellipse(c, r, r)` rather than a separate `circle(c, r)` function.
- You won't find boolean geometry or "remove this point if" conditions here. We also don't use conditional *statements* that alter structure. Instead, Chi offers conditional *expressions*, which simply select between different values.
- A witness node, like one created by `directionpoint`, will consistently be a single point at every instance, no matter where its crossing point happens to move.

Want to double-check yourself, or see how the tooling thinks about it? Point counts are readily available in `chi inspect`:

```text
glyph o
  advance                  636
  contour outer       4 points
  contour inner       4 points
```

If you see the same counts when you run `chi inspect` with `--set weight=100` and again with `--set weight=900`, you know your masters are compatible.

## The trapdoors

Even with Chi's protective design, there are still three specific constructs where you might accidentally introduce a topology change. We made sure to flag all of these when we introduced them in earlier language chapters:

-   **Conditionals over geometry choices.** Something like `r if weight < 500 else 0` is perfectly fine, as it simply chooses a number. It's also fine to use a conditional to swap between different *constructions* for a contour, provided that both options result in the exact same number of points. What you absolutely must avoid is creating arms with different point counts. We refer to this specific limitation as the intended-use boundary in the conditional chapter.
-   **`for` bounds that depend on a parameter.** Imagine wanting one decorative element per 100 units of weight. This approach would mean your light master and your black master end up with entirely different contours, which is a no-go for interpolation.
-   **Count arguments.** When `superellipse(c, rx, ry, n, count)` asks for its point count as an argument, it's precisely because that count needs to stay constant. If you pass in a `count` that's derived from a parameter, you're reintroducing the very problem that these constructors were designed to avoid.

The evaluator doesn't currently police any of these situations in v0.1. Until the sampler arrives, keeping the rule solid is on you. The good news is that running `chi inspect` at the corners of your design space offers a super quick audit.

## The other failure mode: existence

Even if your topology is perfectly stable, there's another kind of hiccup that can occur: an instance might still *fail to evaluate*. Think of it this way: sometimes your design asks for something that simply isn't there in a particular instance. Whether something exists can, in turn, depend on your parameters:

-   A `directionpoint(skel, pen_angle)` witness only comes into existence while the skeleton's tangent actually sweeps through that `pen_angle`. If you rotate the pen axis past the sweep of a leg, that specific instance will lead to an error.
-   An `intersection()` between a serif line and a diagonal will only exist if the diagonal reaches far enough to actually cross the serif.
-   `clamp(x, lo, hi)` will signal an error if a parameter-derived `lo` value ever manages to overtake `hi`.

A great example of this in action lives in `latin_set.chi` from the playground. Its header carefully documents a real-world case: the `pen_angle` range is intentionally narrowed to ensure that a specific witness node on one leg of the `a` is present across the entire design space. For now, the best strategy is to **choose parameter ranges so every query stays satisfiable**, and then make sure to note this constraint in a comment where you declare the range. Want a quick way to check this yourself? Building at each corner of your design space offers a straightforward regression test:

```sh
chi check family.chi
chi build family.chi --set weight=100 -o /dev/null
chi build family.chi --set weight=900 -o /dev/null
```

When our designspace sampler eventually joins the party, it will take on the important task of exploring the entire space and reporting both incompatibilities and unsatisfiable queries.