//! Shape constructors: closed paths of on-curve points and their exact
//! tangents.
//!
//! The handles stay the fitter's business — the Spec is emphatic that they
//! are never written by hand — so these return ordinary path values: they
//! transform, they can be queried, and the designer can always write the
//! same outline out instead.
//!
//! Two rules they share. They run counterclockwise, so the result is
//! directly usable as an outer contour and a counter is `... reversed`,
//! following the Spec's winding convention. And the point count is fixed by
//! the caller, never derived from a coordinate, so a shape keeps its
//! topology across the whole design space, as interpolation compatibility
//! requires.

use crate::ast::Joint;
use crate::value::{PathPoint, PathValue};
use kurbo::{Point, Vec2};

/// The superellipse `|x/rx|^n + |y/ry|^n = 1` about `center`, sampled at
/// `count` evenly spaced bearings with the curve's tangent at each.
/// `n = 2` is the ellipse, and larger `n` squares it off.
pub fn superellipse(center: Point, rx: f64, ry: f64, n: f64, count: usize) -> PathValue {
    let points = (0..count)
        .map(|index| {
            // Multiplying before dividing keeps the axis bearings exactly
            // 0, 90, 180 and 270 for any multiple-of-four count.
            let bearing = 360.0 * index as f64 / count as f64;
            let (offset, tangent) = superellipse_at(rx, ry, n, bearing);
            PathPoint {
                pos: center + offset,
                enter: Some(tangent),
                leave: Some(tangent),
                corner: false,
            }
        })
        .collect();
    PathValue {
        points,
        joints: vec![Joint::Curve; count],
        closed: true,
    }
}

/// Offset from the centre, and the tangent angle in degrees, at `bearing`.
fn superellipse_at(rx: f64, ry: f64, n: f64, bearing: f64) -> (Vec2, f64) {
    // The axis extremes, written down rather than computed. A font wants
    // its points *on* the extremes, and the general form below would leave
    // the residue of cos(90°) amplified by the fractional exponent: 1e-14
    // off the axis for a circle, but 0.06 units at n = 10 and 2.4 at
    // n = 20, on a radius of 100. The tangent there is exactly
    // perpendicular to the radius; it is wrapped under 360° because
    // sin(360°) is -2.4e-16, not zero, which would tilt the fit.
    if bearing % 90.0 == 0.0 {
        let offset = match (bearing / 90.0) as i64 % 4 {
            0 => Vec2::new(rx, 0.0),
            1 => Vec2::new(0.0, ry),
            2 => Vec2::new(-rx, 0.0),
            _ => Vec2::new(0.0, -ry),
        };
        return (offset, (bearing + 90.0) % 360.0);
    }

    let exponent = 2.0 / n;
    let (sin, cos) = bearing.to_radians().sin_cos();
    let offset = Vec2::new(
        rx * signed_pow(cos, exponent),
        ry * signed_pow(sin, exponent),
    );
    // The parametric derivative, with its positive common factor dropped.
    let dx = -rx * cos.abs().powf(exponent - 1.0) * sin;
    let dy = ry * sin.abs().powf(exponent - 1.0) * cos;
    (offset, dy.atan2(dx).to_degrees())
}

/// `|x|` raised to `exponent`, carrying the sign of `x`. Needed because a
/// negative base with a fractional exponent is not a number.
fn signed_pow(x: f64, exponent: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    x.abs().powf(exponent) * x.signum()
}

/// A rectangle spanning two opposite corners, with circular corners of
/// `radius` — four lines and four quarter arcs, counterclockwise from the
/// bottom edge. `radius` is clamped to half the shorter side, above which
/// neighbouring arcs would overlap. At zero the arcs become zero-length
/// segments and the eight points still stand.
pub fn roundrect(corner: Point, opposite: Point, radius: f64) -> PathValue {
    let (x0, x1) = (corner.x.min(opposite.x), corner.x.max(opposite.x));
    let (y0, y1) = (corner.y.min(opposite.y), corner.y.max(opposite.y));
    let k = radius.max(0.0).min(0.5 * (x1 - x0).min(y1 - y0));

    // Every point has a line on one side and an arc on the other, so only
    // the arc side takes a tangent: fit_path reads the line side off the
    // chord and makes the point a line/curve corner.
    let at = |x: f64, y: f64, enter: Option<f64>, leave: Option<f64>| PathPoint {
        pos: Point::new(x, y),
        enter,
        leave,
        corner: false,
    };
    let (right, up, left, down) = (Some(0.0), Some(90.0), Some(180.0), Some(270.0));
    PathValue {
        points: vec![
            at(x0 + k, y0, right, None),
            at(x1 - k, y0, None, right),
            at(x1, y0 + k, up, None),
            at(x1, y1 - k, None, up),
            at(x1 - k, y1, left, None),
            at(x0 + k, y1, None, left),
            at(x0, y1 - k, down, None),
            at(x0, y0 + k, None, down),
        ],
        joints: vec![
            Joint::Line,  // bottom edge
            Joint::Curve, // bottom-right corner
            Joint::Line,  // right edge
            Joint::Curve, // top-right corner
            Joint::Line,  // top edge
            Joint::Curve, // top-left corner
            Joint::Line,  // left edge
            Joint::Curve, // closing back over the bottom-left corner
        ],
        closed: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Twice the signed area of the polygon through a path's points:
    /// positive when they run counterclockwise.
    fn shoelace(path: &PathValue) -> f64 {
        let points = &path.points;
        (0..points.len())
            .map(|index| {
                let a = points[index].pos;
                let b = points[(index + 1) % points.len()].pos;
                a.x * b.y - b.x * a.y
            })
            .sum()
    }

    fn assert_point(actual: Point, x: f64, y: f64) {
        assert!(
            (actual.x - x).abs() < 1e-9 && (actual.y - y).abs() < 1e-9,
            "expected ({x}, {y}), got ({}, {})",
            actual.x,
            actual.y
        );
    }

    #[test]
    fn every_shape_is_closed_constrained_and_counterclockwise() {
        let origin = Point::ORIGIN;
        let shapes = [
            ("circle", superellipse(origin, 100.0, 100.0, 2.0, 4), 4),
            ("ellipse", superellipse(origin, 100.0, 50.0, 2.0, 8), 8),
            ("squircle", superellipse(origin, 100.0, 100.0, 6.0, 16), 16),
            ("roundrect", roundrect(origin, Point::new(100.0, 80.0), 20.0), 8),
        ];
        for (name, shape, count) in shapes {
            assert_eq!(shape.points.len(), count, "{name} point count");
            assert_eq!(shape.joints.len(), count, "{name} joint count");
            assert!(shape.closed, "{name} is not closed");
            assert!(
                shape
                    .points
                    .iter()
                    .all(|point| point.enter.is_some() || point.leave.is_some()),
                "{name} has an unconstrained point"
            );
            assert!(shoelace(&shape) > 0.0, "{name} is not counterclockwise");
        }
    }

    #[test]
    fn superellipse_puts_a_point_on_each_axis() {
        let shape = superellipse(Point::ORIGIN, 100.0, 60.0, 4.0, 12);
        assert_point(shape.points[0].pos, 100.0, 0.0);
        assert_point(shape.points[3].pos, 0.0, 60.0);
        assert_point(shape.points[6].pos, -100.0, 0.0);
        assert_point(shape.points[9].pos, 0.0, -60.0);
        // Perpendicular to the radius, and wrapped under 360°.
        let tangents: Vec<f64> = [0, 3, 6, 9]
            .iter()
            .map(|&index| shape.points[index].leave.expect("constrained"))
            .collect();
        assert_eq!(tangents, vec![90.0, 180.0, 270.0, 0.0]);
    }

    #[test]
    fn roundrect_clamps_its_radius_to_half_the_shorter_side() {
        // 500 wide, 200 tall: the radius holds at 100, where the corner
        // arcs meet and the short edges lose their straight run.
        let shape = roundrect(Point::ORIGIN, Point::new(500.0, 200.0), 999.0);
        assert_point(shape.points[2].pos, 500.0, 100.0);
        assert_point(shape.points[3].pos, 500.0, 100.0);
    }

    #[test]
    fn roundrect_takes_its_corners_in_any_order() {
        let a = roundrect(Point::ORIGIN, Point::new(100.0, 80.0), 20.0);
        let b = roundrect(Point::new(100.0, 80.0), Point::ORIGIN, 20.0);
        for (left, right) in a.points.iter().zip(&b.points) {
            assert_point(left.pos, right.pos.x, right.pos.y);
        }
    }

    #[test]
    fn roundrect_with_no_radius_keeps_its_eight_points() {
        // A corner radius of zero is a rectangle with four zero-length
        // segments, not a four-point path: interpolation compatibility
        // forbids dropping points.
        let shape = roundrect(Point::ORIGIN, Point::new(100.0, 80.0), 0.0);
        assert_eq!(shape.points.len(), 8);
        assert_point(shape.points[1].pos, 100.0, 0.0);
        assert_point(shape.points[2].pos, 100.0, 0.0);
    }

    #[test]
    fn signed_pow_keeps_the_sign_and_handles_zero() {
        assert!((signed_pow(0.25, 0.5) - 0.5).abs() < 1e-12);
        assert!((signed_pow(-0.25, 0.5) + 0.5).abs() < 1e-12);
        assert_eq!(signed_pow(0.0, 0.5), 0.0);
        assert_eq!(signed_pow(-0.0, 0.5), 0.0);
    }
}
