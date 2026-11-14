//! Built-in function signatures and one-line descriptions, from the
//! builtins reference (`docs/src/builtins-reference.md`).

pub struct Builtin {
    pub name: &'static str,
    /// Parameter names of each overload, fewest first.
    pub overloads: &'static [&'static [&'static str]],
    /// Takes any number of arguments like the last parameter.
    pub variadic: bool,
    pub doc: &'static str,
}

const fn fixed(
    name: &'static str,
    overloads: &'static [&'static [&'static str]],
    doc: &'static str,
) -> Builtin {
    Builtin {
        name,
        overloads,
        variadic: false,
        doc,
    }
}

pub const BUILTINS: &[Builtin] = &[
    fixed("min", &[&["a", "b"]], "The smaller of `a` and `b`."),
    fixed("max", &[&["a", "b"]], "The larger of `a` and `b`."),
    fixed(
        "clamp",
        &[&["x", "lo", "hi"]],
        "`x` limited to `[lo, hi]`. An error when `lo > hi`.",
    ),
    fixed("abs", &[&["a"]], "The absolute value of `a`."),
    fixed("sign", &[&["a"]], "−1, 0 or 1; `sign(0)` is 0."),
    fixed("sqrt", &[&["a"]], "The square root of `a`."),
    fixed("floor", &[&["a"]], "`a` rounded down to an integer."),
    fixed("ceil", &[&["a"]], "`a` rounded up to an integer."),
    fixed("round", &[&["a"]], "`a` rounded to the nearest integer."),
    fixed("sin", &[&["a"]], "The sine of `a` degrees."),
    fixed("cos", &[&["a"]], "The cosine of `a` degrees."),
    fixed("tan", &[&["a"]], "The tangent of `a` degrees."),
    fixed(
        "angle",
        &[&["v"]],
        "The angle of vector `v` in degrees (atan2).",
    ),
    fixed(
        "unitvec",
        &[&["theta"]],
        "The unit vector `(cos theta, sin theta)`.",
    ),
    fixed("length", &[&["v"]], "The length of vector `v`."),
    fixed("distance", &[&["a", "b"]], "`length(b - a)`."),
    fixed(
        "intersection",
        &[&["p", "q"], &["p", "q", "i"]],
        "The first intersection of paths `p` and `q` along `p`, or the `i`-th, \
         counting from 0. An error when there are not that many.",
    ),
    fixed(
        "point",
        &[&["p", "t"]],
        "The position on path `p` at segment time `t`, from 0 to the segment count.",
    ),
    fixed(
        "direction",
        &[&["p", "t"]],
        "The tangent angle of path `p` at time `t`, in degrees.",
    ),
    fixed(
        "directionpoint",
        &[&["p", "d"], &["p", "d", "i"]],
        "The first point of path `p` whose tangent points at `d` degrees, \
         or the `i`-th such point, counting from 0.",
    ),
    fixed(
        "jointcap",
        &[&["a", "b"]],
        "The exact pen arc at a corner: node `a` arrives, node `b` leaves, \
         both at the same position with the same pen.",
    ),
    fixed(
        "pen",
        &[&["w"], &["w", "h"], &["w", "h", "angle"]],
        "A round pen `w` across, or an ellipse `w` by `h` with its major axis \
         at `angle`. `h = 0` gives a razor.",
    ),
    fixed(
        "circle",
        &[&["c", "r"]],
        "A closed circle around `c`, 4 points.",
    ),
    fixed(
        "ellipse",
        &[&["c", "rx", "ry"]],
        "A closed ellipse around `c`, 8 points.",
    ),
    fixed(
        "roundrect",
        &[&["p", "q", "r"]],
        "A closed rectangle from corner `p` to corner `q` with corner radius `r`, 8 points.",
    ),
    fixed(
        "superellipse",
        &[&["c", "rx", "ry", "n", "count"]],
        "A closed `|x/rx|^n + |y/ry|^n = 1` around `c` with `count` points; `n` is at least 2.",
    ),
    Builtin {
        name: "draw",
        overloads: &[&["path"]],
        variadic: true,
        doc: "Sketch files: render each path as a thin spine.",
    },
    Builtin {
        name: "fill",
        overloads: &[&["path"]],
        variadic: true,
        doc: "Sketch files: fill each closed path.",
    },
];

pub fn find(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|builtin| builtin.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::eval::BUILTIN_FUNCTIONS;

    #[test]
    fn covers_exactly_the_evaluator_builtins() {
        let mut ours: Vec<&str> = BUILTINS.iter().map(|builtin| builtin.name).collect();
        let mut theirs = BUILTIN_FUNCTIONS.to_vec();
        ours.sort_unstable();
        theirs.sort_unstable();
        assert_eq!(ours, theirs);
    }
}
