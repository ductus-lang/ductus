//! Runtime values.

use crate::ast::Joint;
use kurbo::{Point, Vec2};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Point(Point),
    Node(NodeValue),
    Pen(PenValue),
    Path(PathValue),
    List(Vec<Value>),
    /// Multiple returns from a `def`; exists only between a call and its
    /// destructuring.
    Tuple(Vec<Value>),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Number(_) => "number",
            Value::Bool(_) => "boolean",
            Value::Point(_) => "point",
            Value::Node(_) => "node",
            Value::Pen(_) => "pen",
            Value::Path(_) => "path",
            Value::List(_) => "list",
            Value::Tuple(_) => "tuple",
        }
    }
}

pub fn unitvec(degrees: f64) -> Vec2 {
    let radians = degrees.to_radians();
    Vec2::new(radians.cos(), radians.sin())
}

/// A pen: an ellipse with full axes `2a` (along `angle`) and `2b`.
#[derive(Debug, Clone, Copy)]
pub struct PenValue {
    /// Semi-axis along `angle`.
    pub a: f64,
    /// Semi-axis perpendicular to `angle`. 0 is a razor.
    pub b: f64,
    /// Major-axis orientation, degrees.
    pub angle: f64,
}

impl PenValue {
    /// Support point of the pen centered at the origin: the boundary point
    /// farthest along `unitvec(theta)`. The pen's tangent there is
    /// perpendicular to `theta`. Degenerate for a razor (`b == 0`); callers
    /// handle that case separately.
    pub fn support(&self, theta: f64) -> Vec2 {
        let psi = (theta - self.angle).to_radians();
        let (sin, cos) = psi.sin_cos();
        let h = (self.a * self.a * cos * cos + self.b * self.b * sin * sin).sqrt();
        let local = Vec2::new(self.a * self.a * cos / h, self.b * self.b * sin / h);
        let (rot_sin, rot_cos) = self.angle.to_radians().sin_cos();
        Vec2::new(
            rot_cos * local.x - rot_sin * local.y,
            rot_sin * local.x + rot_cos * local.y,
        )
    }

    /// Ellipse parameter of the contact point `support(theta)`: the
    /// support formula's local coordinates are (a·cos φ, b·sin φ).
    fn contact(&self, theta: f64) -> f64 {
        let psi = (theta - self.angle).to_radians();
        (self.b * psi.sin()).atan2(self.a * psi.cos())
    }
}

/// A skeleton node: position, direction of travel, and the pen that
/// draws through it.
#[derive(Debug, Clone, Copy)]
pub struct NodeValue {
    pub pos: Point,
    /// Direction of travel, degrees.
    pub dir: f64,
    pub pen: PenValue,
}

impl NodeValue {
    /// Offset from `pos` to the left edge. A pen with height has its
    /// support point along the left normal of travel; a razor (`b == 0`)
    /// is degenerate there and offsets along its fixed axis instead.
    fn edge(&self) -> Vec2 {
        if self.pen.b > 0.0 {
            self.pen.support(self.dir + 90.0)
        } else {
            self.pen.a * unitvec(self.pen.angle)
        }
    }

    pub fn left(&self) -> Point {
        self.pos + self.edge()
    }

    pub fn right(&self) -> Point {
        self.pos - self.edge()
    }

    pub fn width(&self) -> f64 {
        2.0 * self.edge().length()
    }

    /// The front of the pen: its support point along the travel direction.
    /// None for a razor pen (`b == 0`) — a razor has no defined front point.
    pub fn tip(&self) -> Option<Point> {
        let pen = self.pen;
        (pen.b > 0.0).then(|| self.pos + pen.support(self.dir))
    }

    /// The back of the pen; see `tip`.
    pub fn tail(&self) -> Option<Point> {
        let pen = self.pen;
        (pen.b > 0.0).then(|| self.pos + pen.support(self.dir + 180.0))
    }

    /// The front terminal: the pen's boundary from the right bank contact,
    /// around `tip`, to the left bank contact, as fixed cubics. None for
    /// a razor, like `tip`.
    pub fn tip_cap(&self) -> Option<PathValue> {
        self.cap(self.dir)
    }

    /// The back terminal: from the left bank contact, around `tail`, to
    /// the right bank contact; see `tip_cap`.
    pub fn tail_cap(&self) -> Option<PathValue> {
        self.cap(self.dir + 180.0)
    }

    /// Half of the pen's boundary centered on the contact point of
    /// `center`, traversed from the contact of `center - 90` to the
    /// contact of `center + 90`.
    ///
    /// Built in unit-circle space, where support contacts sit at their
    /// ellipse parameter and every cap is a half-turn of circular arc:
    /// cubics for arcs of at most 90 degrees are visually exact there
    /// (max radial error ~2.7e-4), and mapping them through the pen's
    /// affine preserves that bound at any eccentricity — precisely where
    /// spline-fitting support points in world space breaks down.
    fn cap(&self, center: f64) -> Option<PathValue> {
        let pen = self.pen;
        if pen.b <= 0.0 {
            return None;
        }

        let start = pen.contact(center - 90.0);
        // Antipodal normals have antipodal parameters, so the cap spans
        // exactly half a turn; the center contact lands strictly inside.
        let end = start + PI;
        let mid = start + (pen.contact(center) - start).rem_euclid(TAU);

        // On-curve parameters: both bank contacts, the center contact
        // (so `.tip`/`.tail` stay on the outline), and a split wherever a
        // piece exceeds a quarter turn.
        let mut params = vec![start];
        for target in [mid, end] {
            let from = *params.last().expect("params start non-empty");
            if target - from > FRAC_PI_2 {
                params.push((from + target) / 2.0);
            }
            params.push(target);
        }

        Some(self.boundary_arc(pen, &params))
    }

    /// The outer arc of a joint: the pen's boundary swept while the
    /// stroke pivots at a corner, turning from this node's direction to
    /// `leaving`'s. The sweep is the turn angle of the joint — a cap is
    /// the half-turn special case. The arc is oriented for the standard
    /// traversal (right bank with the stroke, left bank against it): a
    /// left turn runs from this node's right contact to `leaving`'s; a
    /// right turn runs from `leaving`'s left contact back to this
    /// node's.
    pub fn joint_cap(&self, leaving: &NodeValue) -> Result<PathValue, String> {
        let (pen, other) = (self.pen, leaving.pen);
        if pen.b <= 0.0 {
            return Err(
                "jointcap() needs a pen with height > 0; \
                 a razor's joint is not a curve"
                    .to_string(),
            );
        }
        if pen.a != other.a || pen.b != other.b || pen.angle != other.angle {
            return Err("jointcap() needs both nodes to hold the same pen".to_string());
        }
        if (self.pos - leaving.pos).length() > 1e-9 {
            return Err(
                "jointcap() needs both nodes at the same position — \
                 the corner the pen pivots on"
                    .to_string(),
            );
        }
        let turn = (leaving.dir - self.dir).rem_euclid(360.0);
        if turn == 0.0 {
            return Err("jointcap() needs a turn; the directions are equal".to_string());
        }
        if turn == 180.0 {
            return Err(
                "a 180-degree turn has no outer side; use .tipcap or .tailcap".to_string(),
            );
        }

        // Contact normals of the outer bank, in counterclockwise order.
        let (from, to) = if turn < 180.0 {
            (self.dir - 90.0, leaving.dir - 90.0)
        } else {
            (leaving.dir + 90.0, self.dir + 90.0)
        };
        // Under half a turn of normals maps to under half a turn of
        // ellipse parameter (contact is monotone with antipodal
        // symmetry), so one split keeps every piece within a quarter
        // turn.
        let start = pen.contact(from);
        let end = start + (pen.contact(to) - start).rem_euclid(TAU);
        let mut params = vec![start];
        if end - start > FRAC_PI_2 {
            params.push((start + end) / 2.0);
        }
        params.push(end);
        Ok(self.boundary_arc(pen, &params))
    }

    /// Fixed cubics along the pen's boundary through the given ellipse
    /// parameters, counterclockwise, with the traversal tangents pinned
    /// at every on-curve point.
    fn boundary_arc(&self, pen: PenValue, params: &[f64]) -> PathValue {
        // The pen's affine: unit circle -> scale(a, b) -> rotate -> pos.
        let (rot_sin, rot_cos) = pen.angle.to_radians().sin_cos();
        let linear = |v: Vec2| -> Vec2 {
            let scaled = Vec2::new(pen.a * v.x, pen.b * v.y);
            Vec2::new(
                rot_cos * scaled.x - rot_sin * scaled.y,
                rot_sin * scaled.x + rot_cos * scaled.y,
            )
        };
        let on_curve = |phi: f64| -> Point { self.pos + linear(Vec2::new(phi.cos(), phi.sin())) };
        // World tangent of the counterclockwise traversal at parameter phi.
        let tangent = |phi: f64| -> f64 {
            let t = linear(Vec2::new(-phi.sin(), phi.cos()));
            t.y.atan2(t.x).to_degrees()
        };

        let points = params
            .iter()
            .map(|&phi| {
                let angle = tangent(phi);
                PathPoint {
                    pos: on_curve(phi),
                    enter: Some(angle),
                    leave: Some(angle),
                    corner: false,
                }
            })
            .collect();
        let joints = params
            .windows(2)
            .map(|pair| {
                let (u, v) = (pair[0], pair[1]);
                // Standard circular-arc cubic: handles along the tangents,
                // length (4/3)·tan(sweep/4) in circle space.
                let k = (4.0 / 3.0) * ((v - u) / 4.0).tan();
                let c1 = Vec2::new(u.cos() - k * u.sin(), u.sin() + k * u.cos());
                let c2 = Vec2::new(v.cos() + k * v.sin(), v.sin() - k * v.cos());
                PathJoint::Fixed {
                    c1: self.pos + linear(c1),
                    c2: self.pos + linear(c2),
                }
            })
            .collect();

        PathValue {
            points,
            joints,
            closed: false,
        }
    }
}

/// A joint between consecutive path points, resolved at eval time.
/// `Line` and `Curve` come from the source; `Fixed` carries geometry only
/// evaluation can produce (pen terminals), which the fitter passes through
/// untouched.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathJoint {
    Line,
    Curve,
    /// An explicit cubic: control points are final, endpoints are the
    /// adjacent path points. Transforms map the controls directly — an
    /// affine image of a cubic is that cubic's affine image.
    Fixed { c1: Point, c2: Point },
}

impl From<Joint> for PathJoint {
    fn from(joint: Joint) -> Self {
        match joint {
            Joint::Line => PathJoint::Line,
            Joint::Curve => PathJoint::Curve,
        }
    }
}

/// An evaluated path: positions with resolved direction constraints.
/// Curve fitting happens later, at render time.
#[derive(Debug, Clone)]
pub struct PathValue {
    pub points: Vec<PathPoint>,
    /// `joints[i]` connects `points[i]` to `points[i + 1]`; when `closed`,
    /// the final joint connects the last point back to the first.
    pub joints: Vec<PathJoint>,
    pub closed: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PathPoint {
    pub pos: Point,
    /// Incoming tangent constraint, degrees.
    pub enter: Option<f64>,
    /// Outgoing tangent constraint, degrees.
    pub leave: Option<f64>,
    /// Free corner: tangents on each side fitted independently.
    pub corner: bool,
}
