//! Runtime values.

use crate::ast::Joint;
use kurbo::{Point, Vec2};

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Point(Point),
    Node(NodeValue),
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
