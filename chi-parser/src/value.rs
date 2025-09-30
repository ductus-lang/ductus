use kurbo::{BezPath, Point};

#[derive(Debug, Clone)]
pub enum Value {
    Point(Point),
    Path(BezPath),
    Number(f64),
}
