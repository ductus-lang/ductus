# A first glyph

Imagine a sketch, freely drawing anything you wish, anywhere you please. Now, meet the glyph — it's a bit more committed. A glyph isn't just a drawing; it's a promise, complete with a name, a defined width, and neatly closed contours ready for a font renderer to bring to life. In this chapter, we're going to craft the simplest glyph imaginable: a straightforward sans-serif `l`, essentially just a single rectangle. But we won't stop there; we'll transform it into something parametric, which is, after all, the true magic of using ductus.

## A complete file

```ductus
metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; }

glyph l : U+006C {
  advance = 250;
  stem = 60;
  lsb = (advance - stem) / 2;

  contour main =
      (lsb, 0) -- (lsb + stem, 0)
   -- (lsb + stem, asc) -- (lsb, asc) -- cycle;
}
```

Let's break this down, piece by piece:

- **`metrics`** The `metrics` block is where you define font-level measurements. Five specific keys hold special significance for the renderer: `upm` (units per em), `x` (x-height), `cap` (cap height), `asc` (ascender), and `desc` (descender, which is a negative value). These are crucial for setting the overall size of your canvas and for displaying helpful guidelines in the debug view. Any other key you include here is simply a handy named number, ready to be used by any subsequent code.
- **`glyph l : U+006C { ... }`** This line introduces our glyph, giving it the name `l` and assigning it the Unicode codepoint U+006C. You might notice that the codepoint isn't always strictly necessary; for ligatures or other unencoded glyphs, you can simply leave it out. And if your glyph needs a name that isn't a valid identifier, no problem — just write it as a string, like `glyph "ka.mlym" : U+0D15`.
- **`advance`** The `advance` measurement is absolutely essential — it's the one binding every glyph *must* have, defining its horizontal advance width in font units. Beyond that, everything else inside a glyph's body functions just like ordinary code. Here, `stem` and `lsb` are local bindings. It's helpful to remember that the origin for a glyph is positioned at its left sidebearing point on the baseline, which means `lsb` is literally where your ink begins to flow.
- **`contour`** The **`contour`** keyword is how you define a filled shape. It's crucial that its path **must** always end with `cycle`. If you leave a contour open, ductus won't just issue a warning; it will flag an error. That's because, quite simply, there's no logical way to fill a shape that isn't completely closed.

Now, here's an important convention to start keeping in mind: this contour is drawn **counterclockwise**. Picture it: the bottom edge moves rightward, then up the right side, and finally back along the top. Ductus uses the nonzero winding rule for filling, which means outer contours should run counterclockwise, while any 'counters' — like the hole inside an `o` — should go clockwise. With our simple rectangle here, it doesn't make a practical difference yet. However, once we delve into the chapter on nodes, this winding direction becomes absolutely vital.

## Building it

To bring your designs to life, the `ductus` binary is your go-to tool for checking and rendering your source files:

```sh
ductus check l.ductus                 # parse errors only, no rendering
ductus build l.ductus -o l.svg        # every glyph, one sheet
ductus build l.ductus --glyph l --debug -o l-debug.svg
```

The `--debug` flag activates the construction view, which is the very same rendering style you'll see used throughout this book. It's a fantastic way to visualize everything: your metrics guidelines, the underlying grid, all your on-curve points, and those neatly fitted handles.

## Making it parametric

Right now, our `l` is essentially just a drawing, albeit with a few extra steps. But the real fun begins when we make it parametric. We start by clearly stating what parts of our design can change. This is where the `params` block comes in: it lets you define each design axis, assigning it both a default value and an allowed range. From there, we can elegantly derive properties like our `stem`.

```ductus norender acc
params { weight = 400 in 100..900; }

metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; }

stem = 40 + 0.14 * weight;
```

So, `weight` has now become a vibrant design axis. What that means is this single file is capable of describing every possible `weight` value, all the way from 100 to 900, simultaneously. You then pick a specific *instance* from outside the source file, perhaps by running `ductus build l.ductus --set weight=700`, or by simply adjusting a slider in the playground. Notice that `stem` is still just a regular binding, but now it gracefully adapts based on that chosen `weight`.

The glyph's definition itself hardly needs touching. The main difference is that `stem` moves to the file level, and we adjust the `advance` so it elegantly grows alongside the stem, keeping those sidebearings perfectly balanced.

```ductus acc
glyph l : U+006C {
  advance = stem + 190;
  lsb = (advance - stem) / 2;

  contour main =
      (lsb, 0) -- (lsb + stem, 0)
   -- (lsb + stem, asc) -- (lsb, asc) -- cycle;
}
```

That render used the default, `weight = 400`. The same source at `--set weight=150`:

```ductus set="weight=150"
```

and at `--set weight=900`:

```ductus set="weight=900"
```

Same file, same four points, same topology — just imagine, only the numbers have shifted. This beautiful invariance is a testament to ductus's interpolation-compatibility rule doing its job, and it's precisely what allows you to effortlessly sample an entire designspace from a single, unified source.

> **Try it** — head over to the playground and observe the sidebearings as you adjust the `weight`. You'll notice how the advance gracefully increases with the stem, ensuring the white space on each side remains perfectly consistent. That magic comes from the `advance = stem + 190` line, marking our very first deliberate design decision in this book.

Finally, the debug view of the default instance, now with something to show: the metrics lines from the `metrics` block, the baseline, and the four on-curve points of the contour:

```ductus debug
```

Our simple rectangle, charming as it is, really doesn't get to flex any of ductus's impressive curve-drawing muscles — you won't find any `..` joints, explicit directions, or clever nodes here. But don't worry, the next part of our journey will dive deep into the language features that *do* bring those to life: exploring expressions and mediation, crafting intricate paths with the curve fitter, and building upon the powerful parameter system we've only just begun to uncover in this chapter.