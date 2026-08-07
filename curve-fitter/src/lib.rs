//! Curve fitting with Raph Levien's two-parameter spline method. Solves
//! tangent angles and curvatures over the control points, then renders
//! each segment as cubic Béziers.
//!
//! Folded in from the kurbo-curve-fit-stroke repository, which keeps the
//! history and the abandoned parallel-curve (stroke refitting) exploration.

use kurbo::{BezPath, Point};
use std::f64::consts::PI;

use crate::spline::Spline;
use crate::two_param_curve::TwoParamCurve;
pub mod spline;
pub mod two_param_curve;
pub mod two_param_spline;

#[derive(Debug, Clone)]
pub struct InputPoint {
    pub x: f64,
    pub y: f64,
    pub point_type: PointType,
    /// Incoming tangent angle in degrees (direction approaching this point)
    /// Corresponds to MetaPost syntax: {dir X}..point
    pub incoming_angle: Option<f64>,
    /// Outgoing tangent angle in degrees (direction leaving this point)
    /// Corresponds to MetaPost syntax: point..{dir X}
    pub outgoing_angle: Option<f64>,
}

#[derive(Debug, Clone)]
pub enum PointType {
    Corner,
    Smooth,
    LineToCurve,
    CurveToLine,
}

#[derive(Debug, Clone)]
struct ControlPoint {
    pt: Point,
    ty: PointType,
    lth: Option<f64>, // left tangent angle
    rth: Option<f64>, // right tangent angle
    // Computed during solving
    l_th: Option<f64>,    // computed left tangent
    r_th: Option<f64>,    // computed right tangent
    l_ak: Option<f64>,    // left curvature
    r_ak: Option<f64>,    // right curvature
    k_blend: Option<f64>, // blended curvature
}

impl ControlPoint {
    fn new(pt: Point, ty: PointType, incoming_deg: Option<f64>, outgoing_deg: Option<f64>) -> Self {
        // Convert degrees to radians for internal use
        let lth = incoming_deg.map(|deg| deg.to_radians());
        let rth = outgoing_deg.map(|deg| deg.to_radians());

        Self {
            pt,
            ty,
            lth,
            rth,
            l_th: None,
            r_th: None,
            l_ak: None,
            r_ak: None,
            k_blend: None,
        }
    }
}

#[derive(Debug)]
pub struct CurvatureResult {
    pub ak0: f64, // arctan curvature at start
    pub ak1: f64, // arctan curvature at end
}

#[derive(Debug)]
pub struct SegmentParams {
    pub th0: f64,
    pub th1: f64,
    pub chord: f64,
}

pub fn fit_curve(points: Vec<InputPoint>, is_closed: bool) -> Result<BezPath, String> {
    let mut spline = Spline::new(points, is_closed);
    spline.solve()?;
    Ok(spline.render())
}

/// Normalize angle to -π..π range
fn mod2pi(th: f64) -> f64 {
    let two_pi = 2.0 * PI;
    let frac = th / two_pi;
    two_pi * (frac - frac.round())
}
