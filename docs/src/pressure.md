# Pressure

You might notice ductus doesn't have a dedicated pressure primitive. But that's perfectly fine, because it doesn't really need one. Think of it this way: a node's pen is just an ordinary expression, which means *pressure is simply part of that expression*, sampled along a stroke in the very same way an outline takes shape from sampled points. In this chapter, we'll explore two handy idioms — the classic pointed pen and the versatile pressure-scaled pen — and uncover the shared pattern that makes them both tick.

## The pattern: press()

Both of these neat idioms lean on the path queries we explored in earlier chapters to build nodes right *on* a spine. Imagine this: you give our special `press` macro a path, a moment in time along that path, and a desired pressure. What you get back is a perfectly sampled node. Both its position and direction are inherited directly from the path, ensuring that the banks you see are always true, faithful offsets of the curve that eventually renders.

```ductus norender
def press(path, t, pressure) -> n { ... }

e = press(spine, 2.65, 1);    # "at 2.65 along, full weight"
```

You'll typically draw the spine just once, perhaps with a simple gesture of three or four points. Then, the magic happens: pressure is carefully placed *along* it. What's really cool is that sampling density is completely up to you, and this flexibility is quite important. Picture an arch shoulder making a sharp 125° turn in a single segment. Without a few nodes tucked inside that bend, the inner bank would simply have nothing to trace, leading to some unexpected results.

## The pointed pen

Imagine a classic pointed pen: its tines graciously spread under pressure, moving *across* the direction of travel. In essence, the slit runs  along the stroke. Conveniently, that's exactly where a round pen puts its thickness: always across travel. So a round pen whose diameter follows pressure, `pen(hair + pressure * (shade - hair))`, tells the whole story,  shaping the stroke.

```ductus
u = 100;
shade = 60;              # full-pressure thickness
hair  = 5;               # the unloaded mark

def press(path, t, pressure) -> n {
  node n = point(path, t) dir direction(path, t)
           pen pen(hair + pressure * (shade - hair));
}

spine = (2u, 4u) {265} .. (1.85u, 2u) .. {275} (2u, 0);

a = press(spine, 0,   0);
b = press(spine, 0.6, 0.9);
c = press(spine, 1.4, 0.9);
d = press(spine, 2,   0);

fill(a.right .. b.right .. c.right .. d.right
  -- d.left .. c.left .. b.left .. a.left -- cycle);
```

Here we see a graceful copperplate full-pressure pull: starting with a delicate hairline into the stroke, swelling  through the middle, and then gently releasing back to a hairline as it exits. That lovely pressure profile you're seeing is really just the story told by those four `press` lines. It's fascinating how the point where a stroke reaches its full weight, and how quickly it then lets go, often defines the unique character of one hand from another. And here, that artistry is captured quite  in just four numbers, ready for you to tweak and experiment with.

The terminals in this idiom emerge quite naturally and authentically. A pointed pen, after all, leaves you with either a crisp flat cut, where the banks simply join straight across, or a  sharp end when pressure drops to 0, causing the banks to almost perfectly meet. What you'll never see from this tool, however, is a round cap. A round pen does offer `tip`, `tail` and their caps, but this idiom consciously leaves them alone: using them would introduce caps that the very tool we're modeling simply cannot produce.

## The pressure-scaled pen

Now for our other charming idiom: this one embraces the pen's body. We take an elegant ellipse complete with its own aspect and angle, and size the *entire nib* by pressure. Since a pen is an ordinary expression, each sample simply builds its own.

```ductus
u = 100;
shade = 46;
hair  = 5;

def press(path, t, pressure) -> n {
  w = hair + pressure * (shade - hair);
  node n = point(path, t) dir direction(path, t) pen pen(w, 0.35 w, 20);
}

spine = (0, 0) .. (2u, 2u) .. (4u, 0) .. (6u, 2u);

a = press(spine, 0, 0);
b = press(spine, 1, 0.22);
c = press(spine, 2, 0.73);
d = press(spine, 3, 1);

fill(a.tailcap .. b.right{b.dir} .. c.right{c.dir} .. d.tipcap
  .. c.left{c.dir + 180} .. b.left{b.dir + 180} .. cycle);
```

A couple of details truly make this example sing. First, both the width and height of the pen shrink *together*. This clever design ensures that the pen's aspect ratio and angle remain absolutely constant. Even a 'shut' pen will still  print a hairline-sized ellipse, rather than just a crude point. Second, for our terminals, we rely on the elegant `tailcap`/`tipcap` splices from our last chapter, which  maintain their precision no matter the pressure.

## Choosing between them

When deciding between these two approaches, simply consider the tool you're trying to model. If you're working with tines that spread, think of a round pen and leave its caps alone. Here, the thickness truly lives across the direction of travel, and your stroke ends will naturally be crisp cuts or sharp points. But if your tool has a distinct nib body — perhaps a brush, a stub, or anything that  leaves round ends — then opt for an elliptical pen sized by pressure, complete with its lovely, real caps. The wonderful news is that these two idioms really only differ by a handful of lines of code. What truly unites them, though, is how pressure is *authored* with such precision, sample by sample, as plain numbers you can easily examine with `ductus inspect`.

## Where it runs out

Now, every powerful toolkit has its honest limits, and ours inherits a few from the very nature of sampling:

- **Between samples, pressure's shape is the fitter's choice.** The banks will swell and flow right along with how the spline itself swells. Your only real control here is to add more samples, though naturally, that comes with a corresponding cost in computational effort.
- **Pressure and curvature interact, silently.** It's important to remember that half the stroke width needs to stay comfortably within the spine's radius of curvature. If it doesn't, the inner bank might unfortunately fold over itself, giving your stroke a rather 'gnawed' appearance. Interestingly, real calligraphers instinctively obey this rule — they press firmly on the straight pulls and gently release into the turns. Ductus doesn't yet have a specific diagnostic for this, so for now, the shape itself will be your tell-tale symptom.

And there you have it, the complete strokes toolkit in all its glory. Get ready, because the exciting case studies ahead will put all these concepts to wonderful use, bringing real letters to life.
