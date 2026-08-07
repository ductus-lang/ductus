# A broad-nib o

This is a tool-first study, a charming `o` where its whole personality flows from a single pen. Forget fiddling with per-node widths or complex contrast expressions. Here, an ellipse, simply held at an angle, elegantly dictates where the stroke grows thick, where it tapers to thin, and precisely how slender 'thin' gets. It's a wonderful demonstration of the pens chapter's core ideas in action: a closed skeleton, two witness nodes, and two contours, all working in harmony.

## Pen and skeleton

```ductus norender acc
params {
  # Below 45 the upper witness sits too asymmetrically in its leg and
  # the inner bank folds — see "Working the axes". The range is the fix.
  pen_angle  = 45 in 45..60;
  pen_width  = 105 in 70..150;
  pen_height = 12 in 2..40;
}

metrics { upm = 1000; x = 500; cap = 700; asc = 750; desc = -220; }

pen nib {
  width  = pen_width;
  height = pen_height;
  angle  = pen_angle;
}
```

It's all about the *pen* here. Its three defining axes — the angle it's held at, its generous breadth, and how distinct it is from a mere razor-thin line — truly steer the show. The glyph itself, you'll find, is quite content to let the pen do most of the talking. It's a simple setup: just four nodes gracefully arranged on a counterclockwise skeleton, each one holding onto that very same pen.

```ductus acc glyph=o debug
glyph o : U+006F {
  advance = 560;
  c = (advance / 2, x / 2);
  rx = 190;
  ry = x / 2 + 10;

  node b = c + (0, -ry) dir right pen nib;
  node r = c + (rx, 0)  dir up    pen nib;
  node t = c + (0, ry)  dir left  pen nib;
  node l = c + (-rx, 0) dir down  pen nib;

  guide skel = b.pos .. r.pos .. t.pos .. l.pos .. cycle;

  # The travel sweeps every direction once, so it crosses the pen's
  # axis twice: once at pen_angle, once at pen_angle + 180. Without a
  # node at each crossing the banks would bridge the thin spots fat.
  node w1 = directionpoint(skel, pen_angle) dir pen_angle pen nib;
  node w2 = directionpoint(skel, pen_angle + 180) dir pen_angle + 180 pen nib;

  contour outer =
      b.right .. {w1.dir} w1.right .. r.right .. t.right
   .. {w2.dir} w2.right .. l.right .. cycle;

  contour inner =
      b.left .. {w2.dir + 180} w2.left .. l.left .. t.left
   .. r.left .. {w1.dir + 180} w1.left .. cycle;
}
```

The real magic of this whole exploration truly blossoms within those two clever `directionpoint` lines and the graceful pair of contours. Our witness nodes are beautifully positioned right where the skeleton's tangent perfectly aligns with the pen's axis. If you peek at the debug view, you'll spot them as those two helpful extra nodes nestled between the main compass points. Their banks are precisely `pen_height` apart, ensuring the ring gently pinches down to the pen's absolute minimum thickness at just these specific spots, and nowhere else. As for the contours, they charmingly adhere to the winding rule we learned in the nodes chapter: `right` points follow the skeleton's order for our counterclockwise outer contour, while `left` points, with our witnesses included and constraints reversed, gracefully trace the clockwise inner counter.

The result, plain:

```ductus glyph=o
```

## Working the axes

Ah, but here's where the witnesses truly shine: because they operate as *queries* rather than fixed coordinates, they beautifully adapt to every single axis. Want to steepen the pen's angle? Watch as those delicate thin spots gracefully migrate around the ring. Or perhaps you'd prefer to plump the pen towards a stubbier shape? Then you'll see the pinch simply relax. The best part? It's all from the very same source code, without a single manual edit.

```ductus glyph=o set="pen_angle=60"
```

```ductus glyph=o set="pen_height=34"
```

## The range is part of the design

You'll find two intriguing constraints tucked away in that `params` block, both offering wonderful real-world insights from the interpolation chapter. Let's look at the first one: it's all about *existence*. A witness node can only truly come to life when the skeleton's tangent sweep gracefully covers the pen angle. Happily, with our closed skeleton here, the sweep spans a full circle, meaning every `pen_angle` effortlessly satisfies both queries. But if you were to cut the skeleton open — imagine an 'c', for example — that freedom vanishes, and a careful balance is needed. The `latin_set.ductus` documentation thoughtfully details this precise negotiation right in its header.

The second constraint revealed itself in the most honest way, simply by building out the instances and seeing what happened. We discovered that below `pen_angle = 45`, the upper crossing drifts a bit too close to the top node. This causes the inner bank's fitted curve to fold right over itself, resulting in a rather unexpected black teardrop hanging into the counter. Curiously, the system reports no error whatsoever, because technically, nothing is 'wrong' except for the visual shape itself. (Incidentally, our debugging chapter's field guide points out this very scenario.) While adding more skeleton samples around the crossing would certainly expand the safe range, narrowing the *declared* range to `45..60` offered a much simpler and more direct fix. Plus, it has the lovely benefit of keeping that crucial constraint clearly noted right alongside where the axis is declared. Regardless of the approach, the range always comes to mean something quite specific: it marks precisely where this thoughtful construction is known to hold true, because, well, someone took the time to truly look.