# Lists, loops, and imports

Chi's repetition and reuse machinery is small and, like conditionals, it
comes with an interpolation-shaped warning label.

## for over a range

`for i in a..b` iterates from `a` to `b` **inclusive on both ends**,
stepping by 1 — the same range form the `params` block uses. Each
iteration runs in a fresh scope with the loop variable bound first, so
nothing carries over between iterations; the loop exists for its side
effects (`draw`, `fill`) and for statements parameterized by the
variable:

```chi
u = 100;
for i in 0..6 {
  ray = (0, 0) -- (4u, 0);
  draw(ray rotated (15 i));
}
```

Seven rays, 0° through 90°. `ray` is rebound each time around — legally,
because each iteration is a new scope.

## for over a list

`[a, b, c]` is a list literal, and `for` iterates any values — here a
list of points seeding a row of domes:

```chi
u = 100;
for c in [(0, 0), (2.5u, 0), (5u, 0)] {
  dome = c .. c + (u, u) .. c + (2u, 0) -- cycle;
  fill(dome);
}
```

Lists are iterated, not indexed — chi has no subscripting (the postfix
bracket means mediation), and no way to ask a list its length. If you
find yourself wanting either, you usually want a macro taking the values
as parameters instead.

## Loops versus the design space

The warning label: a `for` bound that varies with a param — say, one
decorative diamond per 100 units of weight — would give different
instances different geometry, breaking interpolation compatibility
exactly the way a structural conditional does. Bounds should be constants
of the design. (The evaluator does not police this yet; the future
designspace sampler will.)

## import

`import "file.chi";` evaluates another file's items — params, metrics,
pens, macros, bindings, glyphs — into the current scope, at the import's
position, as if written inline. This chapter's sibling file
`examples/units.chi` holds a unit and a macro:

```chi norender
# examples/units.chi
u = 100;

def diamond(c, r) -> p {
  p = c + (0, r) -- c + (r, 0) -- c + (0, -r) -- c + (-r, 0) -- cycle;
}
```

and importing it makes both available:

```chi
import "examples/units.chi";

fill(diamond((u, u), 80));
fill(diamond((0, 0), 80) yscaled 0.5 shifted (3u, u));
```

The rules, quickly:

- **Top level only** — an import inside a glyph or macro body is an
  error.
- **Include-once** per canonical file: importing the same file along two
  routes evaluates it once. Import cycles are errors.
- **Relative to the importing file**, so a library can import its own
  helpers no matter where the root file lives.
- **Errors carry the file name** — a broken binding in an imported file
  is reported against that file.
- **The playground rejects imports** for now: file resolution belongs to
  the host, and the browser has no filesystem. The CLI resolves them
  relative to your source file.

Between `def` and `import` you have chi's whole reuse story: macros share
construction within a file, imports share files within a family. What
they share, from Part III on, is mostly nodes — which is where this book
goes next.
