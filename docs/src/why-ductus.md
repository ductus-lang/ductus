# Why ductus

**Why "ductus"?**

*Ductus* is a Latin word. It means "lead" or "conduct". In calligraphy, ductus is the direction, the number, and the sequence of the strokes that make a letterform. In linguistics, it is the character of writing as it flows: the direction, the order, and the speed of the strokes.

That is what this language describes. A Ductus program does not draw outlines. It tells how the strokes of a glyph go: where they start, which way they move, and in what order. The name is the design principle.

At its heart, ductus is a language for parametric type design. Imagine a ductus source file as the blueprint for your glyph outlines, crafted directly in code. Every single point on a contour becomes an expression, dynamically responding to named design parameters like weight, contrast, width—or really, whatever your unique design demands. And here's a delightful detail: you'll never have to wrestle with curve handles by hand. Instead, you simply provide the on-curve points, their directions, and their types. A clever spline solver then takes care of placing all the control points for you.

Let's take a peek at a rendered ductus glyph. Here, for example, is the letter `o`. Notice how its stem thickness, hairline thickness, and overshoot are all dynamic expressions, controlled by just two parameters: `weight` and `contrast`:

```ductus hide
params {
  weight   = 400 in 100..900;
  contrast = 0.5 in 0.5..1.0;
}
metrics {
  upm = 1000; x = 500; cap = 700; asc = 750; desc = -220;
  over = 10 + weight / 200;
}

stem = 40 + 0.14 * weight;
hair = contrast * stem;

def bowl(c, rx, ry) -> (n, w, s, e) {
  node n = c + (0,  ry) dir left  width hair;
  node w = c + (-rx, 0) dir down  width stem;
  node s = c + (0, -ry) dir right width hair;
  node e = c + (rx, 0)  dir up    width stem;
}

glyph o : U+006F {
  advance = 2 stem + 380;
  center = (advance / 2, x / 2);
  ry = x / 2 + over - hair / 2;
  rx = advance / 2 - 55 - stem / 2;

  (n, w, s, e) = bowl(center, rx, ry);
  guide spine = n.pos .. w.pos .. s.pos .. e.pos .. cycle;

  contour outer = n.right .. w.right .. s.right .. e.right .. cycle;
  contour inner = n.left  .. e.left  .. s.left  .. w.left  .. cycle;

  anchor top = (advance / 2, x + over);
}
```

```ductus glyph=o
```

Now, without changing a single line of the source code, simply dial up the `weight` to `850`. Watch as every point gracefully shifts, the solver re-sculpts the curves, and that very same file magically produces the heavier, black version of your design:

```ductus glyph=o set="weight=850"
```

This 'o', for all its elegance, clocks in at about twenty-five lines of source code. Don't worry, this book patiently guides you through building it, step by step, in the chapters dedicated to nodes. For now, let's focus on the heart of those twenty-five lines — the core commitments that drive ductus's design. Ductus is built upon five foundational principles, and you'll find that much of what makes the language feel unique stems directly from embracing these ideas wholeheartedly.

## Outline-first

In ductus, we consider the filled contour to be the star of the show, the primary artifact. You won't find any stroke expansion magic baked into the language itself: ductus doesn't take a centerline and a virtual pen to automatically whip up an outline for you. This is a conscious departure from the approach that sometimes made editing METAFONT's output feel like an uphill battle — when outlines are automatically generated, every tiny correction can feel like you're fighting the very system that created it. With ductus, the outline is precisely what you crafted yourself. This means adding a terminal, an ink trap, or a subtle optical correction is just another ordinary, parametric edit that gracefully blends into your design.

## Skeleton as scaffolding, not geometry

While ductus is quite knowledgeable about strokes, it politely declines to draw them for you. Instead, a *node* acts as a smart skeleton point, carrying crucial information like the stroke's local direction and width, and then conveniently exposing its precisely calculated edge points:

```ductus norender
node w = center + (-rx, 0) dir down width stem;
# w.left and w.right are now points, exactly stem/2 from w.pos
```

Your outlines are then elegantly woven *through* these handy derived points. We never offset curves; instead, we offset individual points. This means you won't encounter any fuzzy offset-curve approximations. And if your design calls for a little tweak, you can easily nudge any derived point with simple arithmetic, like `w.right + (3, 0)`. Take a look at the debug view, and you'll see the elegant scaffolding behind our 'o'. It reveals the skeleton spine serving as a guide, each node proudly displaying its direction and its two derived edge points, along with the gracefully fitted handles of the final contours.

```ductus glyph=o debug
```

## Interpolation compatibility by construction

Here's a key insight: the number and type of on-curve points in a contour are determined directly by your source text, not by the shifting sands of parameter values. This means every single instance across your entire design space shares an identical topology. Our 'o' glyph, for example, steadfastly maintains exactly four points on each contour, whether its `weight` is a slender 100 or a hefty 900. This structural consistency is exactly what you need for smooth interpolation between masters. You'll notice that ductus's language constructs are carefully shaped around this important rule. This is precisely why, for instance, ductus's shape constructors will never let a parameter dictate the number of points in a shape.

## The designer owns the outline

Here's the beautiful truth: anything a node or constructor calculates or derives is completely yours to command. You can replace or adjust it with a perfectly ordinary expression. Ductus simply doesn't believe in points you're not allowed to touch. Our intended workflow encourages you to let the scaffolding thoughtfully place points according to design theory. Then, with a designer's keen eye, you can gently nudge or reposition any points that just don't quite feel right. And here's the clever part: even those adjustments remain parametric, because they're just another expression.

## One way to say a thing

Ductus proudly traces its lineage back to METAFONT and MetaPost, so if you've ever dabbled with those, ductus's path syntax will likely feel comfortably familiar. However, ductus consciously, and quite deliberately, streamlines things by letting go of some of their more elaborate cleverness.

- **No equation solver.** You might remember MetaPost's `whatever`, which allowed you to declare "these lines meet somewhere; now figure it out". Ductus offers you a focused `intersection()` function for paths instead. This covers all the practical cases you'll genuinely use in type design — think a stem meeting a diagonal, or a serif gracefully joining a stem — all without needing a full-blown constraint system in the language.
- **No `tension`.** We considered per-joint tension but ultimately decided against it. In the real world of type design, it turns out designers tend to adjust points and directions directly, rather than fine-tuning abstract tension numbers.
- **No path concatenation.** Each contour in ductus is a single, unified path expression. Macros elegantly share geometry by simply returning the points and nodes they define, rather than by gluing together path segments (which you might remember from MetaPost's `&`). This means you won't have any tricky seam-merging rules to puzzle over.
- **No implicit variables.** You won't find any mysterious reserved `x0`/`y0` magic here. In ductus, a name holds meaning because you thoughtfully gave it that meaning.

Each of these intentional removals contributes to source files that are wonderfully literal: what your file declares is precisely what your glyph becomes.

## What ductus is not, yet

Currently, the core ductus language gracefully evaluates to SVG, producing either one drawing per glyph or a convenient sheet of glyphs. We have exciting plans for UFO and designspace emission, along with smart sampling of masters across the parameter space, all envisioned as a powerful layer built on top. (The interpolation-compatibility rule we discussed earlier is specifically designed to make this future layer sing.) It's worth noting that ductus isn't an interactive editor in the traditional sense. While the delightful [playground](https://github.com/ductus-lang/ductus-playground) offers live rendering and handy parameter sliders, always remember that your source file remains the definitive blueprint of your design.

## How to read this book

The next couple of chapters are designed to get your hands dirty, starting with some freehand sketches, and then moving on to crafting a proper glyph. Part II delves into the language itself, while Part III — where we explore nodes, pens, terminals, and pressure — is really where ductus begins to feel less like a programming language and more like an intuitive design tool. If you're already a seasoned type designer, feel free to peek ahead to Part III. Here's a friendly tip: keep the playground open in a second browser tab while you read. Almost every example in this book will happily run there, and you'll find that simply dragging a slider can teach you more quickly than we could ever explain in words. Every single figure you see is compiled directly from the source code embedded in the book build itself, giving you the satisfying assurance that the code you read is indeed code that truly runs. For the definitive word on the language, the normative definition resides in the [specification](https://github.com/ductus-lang/tree-sitter-ductus/blob/master/Spec.md). This book lovingly cites it, but doesn't aim to replace it.
