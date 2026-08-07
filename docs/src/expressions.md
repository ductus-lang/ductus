# Expressions

Everything in ductus — be it a stem width, a point on a contour, or an angle — is expressed as, well, an expression. We've kept the expression language wonderfully concise, making it easy to learn and use. In this chapter, we'll dive into arithmetic, mediation, and ductus's handy numeric builtins. If you've already peeked at the first-strokes chapter, some of these ideas might just feel like old friends.

## Arithmetic

You'll find the familiar arithmetic operators here: `+ - * / ^`. They follow the usual precedence rules, with `^` handling exponentiation (and being right-associative), while `-` pulls double duty by also negating values. Here's a clever little rule that simplifies things quite a bit: if you place a **number literal directly in front of an expression, it automatically multiplies it**.

```ductus norender
stem = 40 + 0.14 * weight;
hair = 0.8 stem;            # same as 0.8 * stem
gap  = 2(stem + hair);      # parenthesized expressions too
```

This handy shorthand is why ductus code often looks so neat, peppered with expressions like `2u` and `3u`, giving it a clean, graph-paper feel. Just remember, this special rule applies *only* to number *literals*. Trying `stem hair` for multiplication would result in a syntax error, not a product. This way, you'll never accidentally make two names ambiguous.

## Points

In ductus, a coordinate pair like `(x, y)` represents a point, and it's quite versatile, happily doubling as a 2-D vector. You can easily add and subtract points, and scale them by numbers on either side. It's wonderfully intuitive:

```ductus norender
q = p + (30, 0);        # displace
v = b - a;              # the vector from a to b
m = (a + b) / 2;        # scale by a number; also the midpoint, but see below
```

Now, a quick heads-up: trying to add `point + number` directly will lead to an error. There's no implicit 'broadcasting' here. If you're aiming for something like `(p.x + 3, p.y + 3)`, you'll need to explicitly construct that point. For peeking at a point's individual coordinates, you can use `p.x` and `p.y`. And a neat trick like `(p.x, 0)` will effectively drop your point straight down to the baseline.

## Mediation

Prepare to meet ductus's secret weapon: `t[a, b]`. This wonderful expression calculates a value that's `t` of the way from `a` to `b`. It works beautifully for both numbers and points. Seriously, this is ductus's most-used expression. You'll find yourself reaching for it constantly:

```ductus norender
0.5[a, b]              # the midpoint
0.25[x, cap]           # a quarter of the way up from x-height to cap
(w / (w + v))[a, b]    # t can be any numeric expression
```

To get a feel for it, try reading `t[a, b]` aloud as "*t* between *a* and *b*". Since ductus wisely avoids traditional indexing, this postfix bracket notation is delightfully unambiguous. When you're working with points, mediation truly shines, making sure your construction lines stay tidy. Just imagine it ruling a perfect ladder between two rails, all without you ever needing to name a single intermediate coordinate. It's really quite elegant:

```ductus
u = 100;
a = (0, 0);      b = (6u, u);
c = (u, 3u);     d = (5u, 2u);

draw(a -- b);
draw(c -- d);
draw(0.25[a, c] -- 0.25[b, d]);
draw(0.5[a, c]  -- 0.5[b, d]);
draw(0.75[a, c] -- 0.75[b, d]);
```

## Angles and vectors

In ductus, angles always follow the same trusty conventions: they're measured in **degrees, counterclockwise, with 0° pointing right**. You'll find this consistent behavior throughout, from direction specifications to `rotated` transformations and all the trigonometric builtins. For your convenience, we've even provided `right`, `up`, `left`, and `down` as helpful prelude bindings for 0, 90, 180, and 270 degrees, respectively.

We've got two lovely builtins ready to help you switch between angles and vectors: `unitvec(theta)` gives you a neat unit vector at a given `theta` angle, while `angle(v)` kindly tells you the angle of any vector (using atan2, naturally, in degrees). When paired together, they make expressing "perpendicular to" wonderfully straightforward, saving you from wrestling with trigonometry right there in your code:

```ductus
u = 100;
a = (0, 0);
b = (6u, 2u);
n = unitvec(angle(b - a) + 90);    # unit normal to the line ab

draw(a -- b);
draw(0.3[a, b] -- 0.3[a, b] + 50n);
draw(0.7[a, b] -- 0.7[a, b] + 50n);
```

`length(v)` and `distance(a, b)` are also on hand, perfectly rounding out your vector toolkit.

## Numeric builtins

```ductus norender
min(a, b)  max(a, b)  clamp(x, lo, hi)    # clamp errors when lo > hi
abs(a)  sign(a)                           # sign(0) = 0
sqrt(a)  floor(a)  ceil(a)  round(a)
sin(a)  cos(a)  tan(a)                    # degrees, like everything else
```

These handy builtins pretty much do exactly what their names suggest. You'll find a complete rundown, detailing the precise behavior of each and every builtin, in the reference section of this book. Oh, and a little heads-up: builtins like `point`, `direction`, `directionpoint`, and `intersection` are also part of the ductus family, but they're so special they get their own spotlight in the very next chapter.

## Conditional expressions

With `a if cond else b`, you can elegantly choose between two values. For setting up your conditions, you'll use familiar comparison operators like `< <= > >= == !=`, along with logical operators `and`, `or`, and `not`. It's worth noting that ductus doesn't have an `if` *statement* in the traditional sense. Instead, a conditional is always an expression, and it *always* requires both arms to be present. This keeps things nicely balanced:

```ductus
u = 100;
w = 260;
capped = w if w < 2u else 2u;    # by hand; clamp(w, 0, 2u) is the builtin

fill((0, 0) -- (w, 0) -- (w, 30) -- (0, 30) -- cycle);
fill((0, 60) -- (capped, 60) -- (capped, 90) -- (0, 90) -- cycle);
```

Now, a friendly piece of advice: try to use conditionals with a bit of restraint, and please, stick to using them only with *numbers*. Imagine a conditional that decided which points even existed — one arm creating a four-point contour, the other a six-point one. This kind of flexibility, while tempting, would give different instances of your design completely different topologies, which unfortunately breaks that important interpolation-compatibility rule we talked about back in the first chapter. So, clamping a counter's radius is a perfect, intended use case. But swapping out entire pieces of geometry, well, that's generally not what we're aiming for here.