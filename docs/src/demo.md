# Pipeline demo

This page is scaffolding: it exercises every fence option the `chi-docs`
preprocessor supports, and will be replaced by the real chapters. Every
example below is compiled by the book build — if one breaks, the build
fails.

A plain ` ```chi ` fence compiles and renders. A file with no `glyph`
block is a sketch; `draw` renders paths as spines:

```chi
u = 100;
a = (0, 0);
b = (5u, 3u);
p = a {45} .. (3u, 4u) .. {down} b;
draw(p);
```

A ` ```chi hide ` block shows nothing but joins the chapter's hidden
preamble; everything below compiles against it. This one binds the params,
metrics and the stem used by the rest of the page.

```chi hide
params { weight = 400 in 100..900; }
metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; }
stem = 40 + 0.14 * weight;
```

A glyph, leaning on the hidden preamble for `stem` and `asc`:

```chi
glyph l : U+006C {
  advance = stem + 190;
  lsb = (advance - stem) / 2;

  contour main =
      (lsb, 0) -- (lsb + stem, 0)
   -- (lsb + stem, asc) -- (lsb, asc) -- cycle;
}
```

`glyph=` picks one glyph, `set=` picks an instance, and `debug` renders
the construction: metrics lines, guides, skeleton nodes with their derived
edge points, and the fitted handles.

```chi glyph=o set="weight=700" debug
hair = 0.85 * stem;
over = 10 + weight / 200;

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

A ` ```chi norender ` fence is for fragments: highlighted, not compiled,
no output pane.

```chi norender
node m = (x, y) dir theta width (wl, wr);   # asymmetric widths
```

Ordinary fenced blocks in other languages are untouched:

```sh
chi build glyphs.chi --glyph o --set weight=700 --debug -o o-bold.svg
```
