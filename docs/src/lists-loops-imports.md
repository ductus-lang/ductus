# Lists, loops, and imports

Ductus offers some neat tools for repetition and reuse. They're pretty lean, much like conditionals, and they come with a special heads-up about how they play with interpolation.

## for over a range

`for i in a..b` is your go-to for stepping through a sequence of numbers. It'll count from `a` all the way up to `b`, making sure to include both ends of that range, and it increments by 1 each time. This is the very same range style you'll find in the `params` block.

Each time the loop runs, it gets a brand new, clean scope, with your loop variable ready and waiting. This means absolutely nothing carries over from one iteration to the next. So, what's it good for? Primarily, it's there to produce side effects, like `draw` and `fill`, or to help you parameterize statements using that handy loop variable:

```ductus
u = 100;
for i in 0..6 {
  ray = (0, 0) -- (4u, 0);
  draw(ray rotated (15 i));
}
```

And just like that, you get seven rays, spanning from 0° right up to 90°. Notice how `ray` gets a fresh assignment each time the loop goes around, which is perfectly fine because, as we saw, every iteration sets up its own clean scope.

## for over a list

You can create a list right in your code using `[a, b, c]`. And guess what? `for` can happily loop over any values you toss into a list. Take this example, where a list of points lays the groundwork for a charming row of domes:

```ductus
u = 100;
for c in [(0, 0), (2.5u, 0), (5u, 0)] {
  dome = c .. c + (u, u) .. c + (2u, 0) -- cycle;
  fill(dome);
}
```

Now, here's a key point about lists in ductus: you iterate over them, but you can't index them directly. Ductus doesn't offer subscripting (that postfix bracket actually means mediation, which is a different beast entirely), and there's no built-in way to check a list's length either. If you ever catch yourself wishing for those features, you'll generally find that a macro, designed to take those values as parameters, is a much better fit for what you're trying to achieve.

## Loops versus the design space

Alright, let's talk about that warning label we mentioned earlier. Imagine you tie a `for` loop's bounds to a parameter – perhaps you want one snazzy decorative diamond for every 100 units of weight. What happens then? Well, different instances of your design would end up with wildly different geometries. This completely breaks interpolation compatibility, much in the same way a structural conditional would. The takeaway here is clear: your loop bounds really ought to be constants inherent to your design.

By the way, the current evaluator isn't strict about this yet, but the upcoming designspace sampler will certainly be keeping an eye on it.

## import

`import "file.ductus";` is your way of bringing in goodies from another file. It takes all sorts of items – parameters, metrics, pens, macros, bindings, even entire glyphs – and evaluates them right into your current scope, exactly where you placed the import statement. Think of it as if you had typed all that content directly into your file.

To see it in action, let's look at `examples/units.ductus`, a companion file for this chapter. It thoughtfully provides us with a handy unit and a macro:

```ductus norender
# examples/units.ductus
u = 100;

def diamond(c, r) -> p {
  p = c + (0, r) -- c + (r, 0) -- c + (0, -r) -- c + (-r, 0) -- cycle;
}
```

Once we import it, both become instantly available for us to use:

```ductus
import "examples/units.ductus";

fill(diamond((u, u), 80));
fill(diamond((0, 0), 80) yscaled 0.5 shifted (3u, u));
```

A few quick rules to keep in mind about importing:

- **Top level only** — You'll want to place your imports at the top level of your file; tucking an import inside a glyph or macro body will result in an error.
- **Include-once** per canonical file: No need to worry about duplicate evaluations. Even if you import the same file via different paths, ductus will only evaluate it once. Also, do keep an eye out for import cycles; those will trigger an error.
- **Relative to the importing file**, which is super handy. This setup means a library can confidently import its own internal helpers, no matter where your main project file decides to call home.
- **Errors carry the file name** — If something goes wrong, say a binding is broken in an imported file, ductus will let you know by reporting the error against that specific file. It helps keep debugging clear.
- **The playground rejects imports** for now: Just a heads-up that the playground isn't quite ready for imports yet. That's because file resolution is handled by the host environment, and, well, your browser doesn't have a filesystem to browse. The CLI, on the other hand, handles them beautifully, resolving paths relative to your source file.

So, there you have it. Between `def` for defining things and `import` for bringing in outside resources, you've got ductus's entire story for reusing your work. Think of it this way: macros are perfect for sharing clever constructions right within a single file, while imports are designed for sharing entire files across a whole family of designs.

And what exactly do they share? Well, from Part III onward, you'll discover that it's mostly about nodes – and that's precisely where we're headed next in this book.