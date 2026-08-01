# Macros

`def` names a piece of construction. A macro's body is declarative like
everything else in chi — bindings, no `return` statement — and an arrow
names which of those bindings the caller receives:

```chi
u = 100;

def diamond(c, r) -> p {
  p = c + (0, r) -- c + (r, 0) -- c + (0, -r) -- c + (-r, 0) -- cycle;
}

fill(diamond((u, u), 80));
fill(diamond((3u, u), 55));
fill(diamond((4.2u, u), 30));
```

Parameters are untyped names, arguments pass by value, and any value
can travel: numbers, points, paths, pens — and nodes, which Part III's
constructions are built on.

## Returning several things

`-> (a, b, c)` exports a tuple, destructured at the call site. The tuple
is not a general value — it exists only across the call boundary — so
the destructuring pattern is the whole protocol:

```chi
u = 100;

def arch(c, w, h) -> (l, top, r) {
  l = c + (-w/2, 0);
  top = c + (0, h);
  r = c + (w/2, 0);
}

(l1, t1, r1) = arch((2u, 0), 3u, 2u);
draw(l1 {up} .. t1 .. {down} r1);

(l2, t2, r2) = arch((5.5u, 0), 2u, 3u);
draw(l2 {up} .. t2 .. {down} r2);
```

Two arches, one definition, different proportions — and the caller wrote
the path itself, through the returned points.

## Why macros return geometry, not path pieces

That last example is worth staring at for a moment, because it answers
the question every MetaPost user asks sooner or later: where is `&`,
the path concatenation operator? Chi doesn't have one. A contour is **one path expression**, and
a macro contributes by returning the points (or nodes) the contour passes
through — not by returning path fragments to be glued.

The trade is deliberate. Concatenation needs rules for merging the seam —
whose direction constraint wins, is the junction smooth or a corner —
and those rules would live invisibly at every joint. In chi the caller
writes the junction explicitly, because the caller is writing the whole
path; what a macro computed and what the contour does with it stay
separable. The macros in Part III return *nodes* for the same reason: the
`bowl()` macro hands back four skeleton points, and the glyph decides how
its contours travel through them.

Macros may call macros — build a serif from corner pieces, a letter part
from serifs. Recursion is not allowed, and macro definitions are top-level
only, like `params` and `pen` blocks.

## Scope

A macro body is an ordinary scope: it sees every top-level binding
(params, metrics, and anything defined above), its parameters shadow
outer names, and its locals — everything not exported — vanish at the
closing brace. Nothing leaks; what you don't export doesn't exist
outside the braces.
