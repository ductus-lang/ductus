//! Runtime values.

use crate::ast::Joint;
use kurbo::{Point, Vec2};

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
}

/// A skeleton node: position, direction of travel, stroke width.
#[derive(Debug, Clone, Copy)]
pub struct NodeValue {
    pub pos: Point,
    /// Direction of travel, degrees.
    pub dir: f64,
    /// Offset from `pos` to the left edge, along `nib`.
    pub left_offset: f64,
    /// Offset from `pos` to the right edge, against `nib`.
    pub right_offset: f64,
    /// Axis along which width is applied, degrees. Defaults to `dir + 90`.
    pub nib: f64,
    /// The pen this node was declared with, if any; needed for `tip`/`tail`.
    pub pen: Option<PenValue>,
}

impl NodeValue {
    pub fn left(&self) -> Point {
        self.pos + self.left_offset * unitvec(self.nib)
    }

    pub fn right(&self) -> Point {
        self.pos - self.right_offset * unitvec(self.nib)
    }

    pub fn width(&self) -> f64 {
        self.left_offset + self.right_offset
    }

    /// The front of the pen: its support point along the travel direction.
    /// None without a pen, or for a razor pen (`b == 0`) — a razor has no
    /// defined front point.
    pub fn tip(&self) -> Option<Point> {
        let pen = self.pen?;
        (pen.b > 0.0).then(|| self.pos + pen.support(self.dir))
    }

    /// The back of the pen; see `tip`.
    pub fn tail(&self) -> Option<Point> {
        let pen = self.pen?;
        (pen.b > 0.0).then(|| self.pos + pen.support(self.dir + 180.0))
    }
}

/// An evaluated path: positions with resolved direction constraints.
/// Curve fitting happens later, at render time.
#[derive(Debug, Clone)]
pub struct PathValue {
    pub points: Vec<PathPoint>,
    /// `joints[i]` connects `points[i]` to `points[i + 1]`; when `closed`,
    /// the final joint connects the last point back to the first.
    pub joints: Vec<Joint>,
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
