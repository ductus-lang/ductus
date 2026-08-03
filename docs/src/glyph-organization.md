# Glyph organization

Up until now, we've thought of a glyph as a name paired with its contours. But as you start building an entire *font* full of them, some important organizational questions pop up. You might wonder: what's a proper name for a glyph, what essential bits go inside its definition, and how exactly do a bunch of files collaborate on a shared design. Luckily, Chi's answers to these questions are nothing new – they're all concepts you've already bumped into, just neatly arranged.

## Names and codepoints

```chi norender
glyph o : U+006F { ... }          # encoded, identifier name
glyph "ka.mlym" : U+0D15 { ... }  # names that aren't identifiers
glyph "t_t" { ... }               # unencoded: ligatures, alternates
```

That codepoint is entirely optional, by the way. If you're defining a ligature or a stylistic alternate, you can simply leave it out. And for those tricky names an identifier just can't handle – think `script`-suffixed conventions often found in complex-script fonts – a string name comes to the rescue. Each glyph gets its own little block, and the sheet kindly renders them exactly as they appear in your source file.

## What lives in a glyph body

When it comes to the glyph's body, only four specific kinds of statements actually tell the renderer what to do. Everything else you write there is just plain old code:

- `advance = expr;` — This one is essential: it sets the horizontal space your glyph takes up.
- `contour name = path;` — These define the actual 'ink' that gets filled, and remember, they always need to end with `cycle` to close the shape.
- `guide name = path;` — Think of guides as your trusty construction geometry: things like skeletons, diagonals, or those helpful alignment lines. They're never filled with ink and you'll only see them in the debug view. Since guide names are just regular path bindings, you can poke around with them using `directionpoint` and other handy tools. Essentially, a guide is a piece of documentation that your compiler thoughtfully checks and your tooling beautifully draws for you.
- `anchor name = point;` — These create named positions, perfect for attaching marks. Imagine where an accent might gracefully perch over this glyph, or where a matra gently clips on. Chi itself simply records and displays these points (you'll spot them in the debug view or with `chi inspect`), but the actual attachment magic happens with the font compiler further down the line.

## Sharing a design: one file, then several

When you keep related glyphs together in a single file, they naturally share everything defined above them – that's just how scoping works its charm. Take a peek at the playground's `latin_set.chi` file, for instance. It crafts an entire broad-nib lowercase alphabet this very way: a single `params` block, one shared pen, and then glyph after delightful glyph.

But what happens when one file grows a bit too big to manage comfortably? That's where `import` (a friend you met in the language chapters) steps in as your go-to organizational superhero. You can neatly tuck away your axes, metrics, stroke weights, and construction macros into a dedicated 'foundation' file, and then simply have each glyph file – or even an entire family file – import it. Here's a glimpse at our example foundation for this chapter:

```chi norender
# examples/family.chi
params { weight = 400 in 100..900; }

metrics {
  upm = 1000; x = 500; cap = 700; asc = 750; desc = -220;
  over = 10 + weight / 200;
}

stem = 40 + 0.14 * weight;
hair = 0.85 * stem;

def bowl(c, rx, ry) -> (n, w, s, e) { ... }
```

Now, let's look at two glyphs built upon this foundation. Notice how much *isn't* explicitly defined within each glyph, because our trusty foundation file is carrying all that weight for us:

```chi acc
import "examples/family.chi";

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

glyph c : U+0063 {
  advance = 2 stem + 330;
  center = (advance / 2 + 30, x / 2);
  ry = x / 2 + over - hair / 2;
  rx = advance / 2 - 40 - stem / 2;

  (n, w, s, e) = bowl(center, rx, ry);
  guide spine = n.pos .. w.pos .. s.pos;

  contour main = n.right .. w.right .. s.right
              -- s.left .. w.left .. n.left -- cycle;
}
```

The sheet will present both glyphs, carefully advanced and nestled side by side, exactly as they appear in your source file. The 'c' cleverly reuses the 'o's bowl, simply choosing not to follow the path down the right side. It's the same set of nodes, just a different contour story. This perfectly illustrates the pattern from the macros chapter – where macros kindly hand back geometry, and callers then get to craft their own paths – now doing some fantastic family-scale heavy lifting.

So, for a real-world font family, here's a layout that tends to work beautifully:

```text
family.chi          # imports everything below; chi build family.chi
foundation.chi      # params, metrics, stroke weights
pens.chi            # pen blocks
constructions.chi   # bowl(), arch(), serif() macros
latin.chi           # glyph blocks
malayalam.chi       # glyph blocks
```

A neat detail: `import` works as an include-once mechanism and resolves paths relative to the importing file. This means your individual glyph files can also be built on their own – each one simply begins with `import "foundation.chi";` and instantly gains access to that consistent design space.

## Anchors and guides in the debug view

In the debug rendering, you'll see everything the glyph body declared beautifully labeled. Take a look at those spine guides, each individual node, and the 'o's `top` anchor gracefully floating above its ring.

```chi glyph=o debug
```

Everything you see on that sheet, apart from the two actual contours, is what we call metadata. It's wonderfully visible scaffolding there just for you, the designer, but it remains completely invisible to the final rendered output. Our next chapter will delve into the single, crucial promise that all this thoughtful organization needs to uphold across the entire design space.