# A slab-serifed stem

Let's kick things off with our simplest exploration – a capital stem boasting a bracketed slab serif, crafted as a single, uninterrupted outline. It's the only one that doesn't even need a skeleton. This example  illustrates just how much you can achieve with simple paths and automatic line/curve corners before you even think about placing a node. Plus, it highlights how elegantly serif proportions remain fully parametric when they're simply derived from the stem itself.

## The design

Think of a slab serif as a neat rectangle nestled right beneath the stem. Now, the *bracket* is that lovely little curve that gently coaxes the serif into the stem, ensuring the joint feels smooth and never abruptly angular. We describe it using just four key numbers. The clever part is that they all spring from `stem`, which means your serif  holds onto its perfect proportions no matter how the weight changes:

```ductus norender acc
params { weight = 400 in 100..900; }

metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; }

stem = 40 + 0.14 * weight;

sl = 30 + 0.45 * stem;    # serif overhang beyond the stem
sh = 24 + 0.20 * stem;    # serif slab height
br = 26 + 0.30 * stem;    # bracket size
```

## The outline

We'll trace this  shape counterclockwise, starting from the serif's bottom-left corner. You'll notice that every corner along the slab is a simple line-to-line transition, so there's no need for any special annotations. The two brackets, though, achieve their lovely smoothness precisely *because* their entry and exit directions are so thoughtfully constrained. For instance, `{left} .. {up}` effortlessly guides the curve from a horizontal path right into a vertical one:

```ductus acc glyph=stem
glyph stem {
  advance = stem + 2 sl + 100;
  xl = (advance - stem) / 2;    # stem left edge
  xr = xl + stem;               # stem right edge

  contour main =
      (xl - sl, 0) -- (xr + sl, 0) -- (xr + sl, sh)
   -- (xr + br, sh) {left} .. {up} (xr, sh + 2br)
   -- (xr, cap) -- (xl, cap)
   -- (xl, sh + 2br) {down} .. {left} (xl - br, sh)
   -- (xl - sl, sh) -- cycle;
}
```

Let's take a closer look at that right bracket. The path makes a graceful entrance along the serif's top, heading `{left}`, then gently curves, and finally departs heading `{up}` right into the stem. It's truly amazing: just one segment and two constraints, and ductus's fitter somehow magically figures out the perfect, smooth ease. What's more, those points where the bracket's curve meets the straight lines on either side? They seamlessly become line/curve corners all on their own.

The debug view gives us a wonderful look at the entire structure – do note how the on-curve points  cluster just at the brackets, and nowhere else:

```ductus glyph=stem debug
```

## Across the space

Here's where the magic really happens: because `sl`, `sh`, and `br` all grow alongside the `stem` (though not purely proportionally, as each has a constant floor), our serif builds a wonderful sturdiness in the heavier weights without ever turning into a clunky brick. And in the lighter weights, it remains  present and refuses to simply disappear:

```ductus glyph=stem set="weight=120"
```

```ductus glyph=stem set="weight=880"
```

Imagine, just ten points at every single instance. Now, if there's one design decision here truly worth borrowing, it's all about these derivations, not just the path itself. Crafting a serif proportioned as `floor + k · stem` ensures that it interpolates *sensibly* across all weights. And believe me, no amount of clever outline trickery can ever save a design if the fundamental numbers underneath aren't quite right.
