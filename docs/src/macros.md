# Macros

When you use `def`, you're essentially giving a name to a building block of your design. Just like everything else in ductus, a macro's body is declarative—we're talking about bindings here, no `return` statements in sight. And that neat little arrow, well, it's there to tell us exactly which of those bindings the caller will get back:

```ductus
u = 100;

def diamond(c, r) -> p {
  p = c + (0, r) -- c + (r, 0) -- c + (0, -r) -- c + (-r, 0) -- cycle;
}

fill(diamond((u, u), 80));
fill(diamond((3u, u), 55));
fill(diamond((4.2u, u), 30));
```

Parameters are simply names without a specific type, and arguments are always passed by value. What's really cool is how versatile ductus is: almost any value can travel through your macros. Think numbers, points, paths, and pens—even the nodes that form the backbone of the constructions we'll explore in Part III.

## Returning several things

If you need to send back more than one thing, simply use `-> (a, b, c)` to return a tuple. Ductus handles the destructuring right at the spot where you call your macro.

Now, this tuple isn't a general-purpose value you can pass around freely; its existence is quite special, spanning just that bridge between the macro and its caller. Because of this, the destructuring pattern you define becomes the entire handshake protocol for those returned values.

```ductus
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

Just like that, you have two arches, born from a single definition but sporting different proportions. And notice this: the caller takes the reins, crafting the path itself using those handy returned points.

## Why macros return geometry, not path pieces

Let's take a moment to really look at that last example, shall we? It elegantly answers a question that every MetaPost user eventually bumps into: where exactly is the `&` path concatenation operator in ductus? The short answer is: you won't find one in ductus.

In ductus, a contour is fundamentally defined as **one path expression**. This means that instead of giving you little path fragments to glue together, a macro helps out by returning the precise points (or nodes) that your contour will journey through.

This design choice isn't accidental; it's quite deliberate. Think about it: path concatenation often requires a whole set of rules for merging the 'seam.' You'd need to decide whose direction constraint wins, whether a junction should be smooth or a sharp corner, and all those rules would silently operate, invisible at every single joint. Ductus takes a different route. Here, the caller explicitly defines each junction because, after all, they are crafting the entire path themselves. This keeps what a macro computes wonderfully separate from how the contour ultimately uses those values.

You'll see this principle again in Part III, where macros return *nodes* for precisely the same reason. Take the `bowl()` macro, for instance: it hands you four key 'skeleton' points, and then the glyph itself gets to decide exactly how its contours gracefully travel through them.

Macros can absolutely call other macros. Imagine building a fancy serif from smaller corner pieces, or crafting an entire letter part out of those serifs. However, there are a couple of ground rules: recursion is not allowed, and just like `params` and `pen` blocks, macro definitions must live at the top level of your code.

## Scope

A macro body operates within its own ordinary scope. This means it comfortably sees all top-level bindings—like your `params`, `metrics`, and anything else defined earlier on. Its own parameters will gracefully shadow any outer names, and any local variables within—that's everything you don't explicitly export—will simply vanish once you hit that closing brace. It's a tidy system: nothing leaks out. What you don't choose to export simply ceases to exist once you step beyond those protective braces.