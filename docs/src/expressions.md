# Expressions

Everything in chi — a stem width, a point on a contour, an angle — is an
expression, and the expression language is deliberately small. This
chapter is arithmetic, mediation, and the numeric builtins; if you have
skimmed the first-strokes chapter, some of it will already feel familiar.

## Arithmetic

`+ - * / ^` with the usual precedence; `^` is exponentiation and
right-associative; `-` also negates. One rule does a lot of work: a
**number literal directly in front of an expression multiplies it**.

```chi norender
stem = 40 + 0.14 * weight;
hair = 0.8 stem;            # same as 0.8 * stem
gap  = 2(stem + hair);      # parenthesized expressions too
```

This is why chi code full of `2u` and `3u` reads like graph paper. The
rule applies to number *literals* only — `stem hair` is a syntax error,
not a product — so it never makes two names ambiguous.

## Points

`(x, y)` is a point, and a point doubles as a 2-D vector. Points add and
subtract, and scale by numbers on either side:

```chi norender
q = p + (30, 0);        # displace
v = b - a;              # the vector from a to b
m = (a + b) / 2;        # scale by a number; also the midpoint, but see below
```

`point + number` is an error — there is no implicit broadcasting, if you
mean `(p.x + 3, p.y + 3)` you write a point. The fields `p.x` and `p.y`
read a point's coordinates; `(p.x, 0)` drops it to the baseline.

## Mediation

`t[a, b]` is the value `t` of the way from `a` to `b` — for numbers and
for points. It is chi's single most-used expression:

```chi norender
0.5[a, b]              # the midpoint
0.25[x, cap]           # a quarter of the way up from x-height to cap
(w / (w + v))[a, b]    # t can be any numeric expression
```

Read `t[a, b]` aloud as "*t* between *a* and *b*". Because chi has no
indexing, the postfix bracket is unambiguous. Mediation between points is
how construction lines stay construction lines — here it rules a ladder
between two rails without naming a single intermediate coordinate:

```chi
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

All angles in chi are **degrees, counterclockwise, 0° pointing right**,
everywhere — direction specs, `rotated`, the trigonometric builtins.
`right`, `up`, `left`, `down` are prelude bindings for 0, 90, 180, 270.

Two builtins convert between angles and vectors: `unitvec(theta)` is the
unit vector at angle `theta`, and `angle(v)` is the angle of a vector
(atan2, in degrees). Together they express "perpendicular to" without
trigonometry at the use site:

```chi
u = 100;
a = (0, 0);
b = (6u, 2u);
n = unitvec(angle(b - a) + 90);    # unit normal to the line ab

draw(a -- b);
draw(0.3[a, b] -- 0.3[a, b] + 50n);
draw(0.7[a, b] -- 0.7[a, b] + 50n);
```

`length(v)` and `distance(a, b)` round out the vector kit.

## Numeric builtins

```chi norender
min(a, b)  max(a, b)  clamp(x, lo, hi)    # clamp errors when lo > hi
abs(a)  sign(a)                           # sign(0) = 0
sqrt(a)  floor(a)  ceil(a)  round(a)
sin(a)  cos(a)  tan(a)                    # degrees, like everything else
```

These do what they say; the reference part of this book lists every
builtin with its exact behavior. The path queries — `point`, `direction`,
`directionpoint`, `intersection` — are builtins too, but they belong to
the next chapter.

## Conditional expressions

`a if cond else b` selects between two values. Conditions use
`< <= > >= == !=` and `and`, `or`, `not`. There is no `if` *statement* —
a conditional is always an expression with both arms present:

```chi
u = 100;
w = 260;
capped = w if w < 2u else 2u;    # by hand; clamp(w, 0, 2u) is the builtin

fill((0, 0) -- (w, 0) -- (w, 30) -- (0, 30) -- cycle);
fill((0, 60) -- (capped, 60) -- (capped, 90) -- (0, 90) -- cycle);
```

Use conditionals sparingly, and only over *numbers*. A conditional that
changed which points exist — one arm a four-point contour, the other a
six-point one — would give different instances of the design different
topology, breaking the interpolation-compatibility rule from the first
chapter. Clamping a counter's radius is the intended kind of use; swapping
geometry is not.
