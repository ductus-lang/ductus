//! Curve fitting and SVG output.

use crate::ast::Joint;
use crate::doc::{Document, Glyph, SketchCommand};
use crate::value::{PathValue, unitvec};
use curve_fitter::{InputPoint, PointType, fit_curve};
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape};

#[derive(Debug, Clone, PartialEq)]
pub struct RenderError {
    pub message: String,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "render error: {}", self.message)
    }
}

impl std::error::Error for RenderError {}

fn render_err(message: impl Into<String>) -> RenderError {
    RenderError {
        message: message.into(),
    }
}

#[derive(Debug, Default)]
pub struct RenderOptions {
    /// Render only the named glyph; all glyphs otherwise.
    pub glyph: Option<String>,
    /// Draw guides, skeleton nodes, metrics lines, points and handles.
    pub debug: bool,
}

// ---- Curve fitting ---------------------------------------------------------

/// Fit an evaluated path to cubic Béziers with the two-parameter spline.
pub fn fit_path(path: &PathValue) -> Result<BezPath, RenderError> {
    let count = path.points.len();
    if count < 2 {
        return Err(render_err("a path needs at least 2 points"));
    }

    let joint_before = |index: usize| -> Option<Joint> {
        if index > 0 {
            Some(path.joints[index - 1])
        } else if path.closed {
            path.joints.last().copied()
        } else {
            None
        }
    };
    let joint_after = |index: usize| -> Option<Joint> {
        path.joints.get(index).copied()
    };

    let chord = |from: usize, to: usize| -> f64 {
        let delta = path.points[to].pos - path.points[from].pos;
        delta.y.atan2(delta.x).to_degrees()
    };

    // Per spec §5.1 a line/curve meeting is a corner: the line side's tangent
    // is the chord (a line is a line), the curve side is free unless the
    // source constrains it. Line-side tangents are computed here rather than
    // left to the fitter, whose built-in conversion only fires when a point
    // has no explicit angles at all.
    let input_points: Vec<InputPoint> = path
        .points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let mut point_type = PointType::Smooth;
            let mut incoming = point.enter;
            let mut outgoing = point.leave;
            match (joint_before(index), joint_after(index)) {
                (Some(Joint::Line), Some(Joint::Line)) => point_type = PointType::Corner,
                (Some(Joint::Line), _) => {
                    point_type = PointType::Corner;
                    incoming = Some(chord((index + count - 1) % count, index));
                }
                (_, Some(Joint::Line)) => {
                    point_type = PointType::Corner;
                    outgoing = Some(chord(index, (index + 1) % count));
                }
                _ => {}
            }
            if point.corner {
                point_type = PointType::Corner;
            }
            InputPoint {
                x: point.pos.x,
                y: point.pos.y,
                point_type,
                incoming_angle: incoming,
                outgoing_angle: outgoing,
            }
        })
        .collect();

    fit_curve(input_points, path.closed).map_err(render_err)
}

// ---- SVG -------------------------------------------------------------------

/// Vertical extent of the drawing, font units, y-up.
struct Extent {
    ascent: f64,
    descent: f64,
}

pub fn render(document: &Document, options: &RenderOptions) -> Result<String, RenderError> {
    let glyphs: Vec<&Glyph> = match &options.glyph {
        Some(name) => {
            let glyph = document
                .glyphs
                .iter()
                .find(|glyph| glyph.name == *name)
                .ok_or_else(|| render_err(format!("no glyph named '{name}'")))?;
            vec![glyph]
        }
        None => document.glyphs.iter().collect(),
    };

    let upm = document.metrics.upm();
    let margin = upm / 10.0;
    let mut body = String::new();
    let mut fitted_sketch = Vec::new();

    // Sketch commands share the sheet, drawn at x = 0.
    for command in &document.sketch {
        let path = match command {
            SketchCommand::Draw(path) | SketchCommand::Fill(path) => path,
        };
        fitted_sketch.push((command, fit_path(path)?));
    }

    let extent = compute_extent(document, &glyphs, &fitted_sketch)?;

    let mut x_offset = 0.0;
    for glyph in &glyphs {
        body.push_str(&render_glyph(glyph, document, x_offset, &extent, options)?);
        x_offset += glyph.advance + margin;
    }

    for (command, fitted) in &fitted_sketch {
        let mut flipped = fitted.clone();
        flipped.apply_affine(flip_at(0.0));
        match command {
            SketchCommand::Fill(_) => body.push_str(&format!(
                "  <path d=\"{}\" fill=\"black\" fill-rule=\"nonzero\"/>\n",
                flipped.to_svg()
            )),
            SketchCommand::Draw(_) => body.push_str(&format!(
                "  <path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"2\"/>\n",
                flipped.to_svg()
            )),
        }
    }

    let total_width = if glyphs.is_empty() {
        fitted_sketch
            .iter()
            .map(|(_, fitted)| fitted.bounding_box().max_x())
            .fold(upm, f64::max)
    } else {
        x_offset - margin
    };

    let min_x = -margin;
    let min_y = -extent.ascent - margin;
    let width = total_width + 2.0 * margin;
    let height = extent.ascent - extent.descent + 2.0 * margin;

    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\">\n{}</svg>\n",
        fmt(min_x),
        fmt(min_y),
        fmt(width),
        fmt(height),
        body
    ))
}

fn compute_extent(
    document: &Document,
    glyphs: &[&Glyph],
    fitted_sketch: &[(&SketchCommand, BezPath)],
) -> Result<Extent, RenderError> {
    let metrics = &document.metrics;
    if let (Some(ascent), Some(descent)) = (metrics.get("asc"), metrics.get("desc")) {
        return Ok(Extent { ascent, descent });
    }
    // No vertical metrics: size to the content.
    let mut bounds: Option<Rect> = None;
    let mut include = |rect: Rect| {
        bounds = Some(match bounds {
            Some(bounds) => bounds.union(rect),
            None => rect,
        });
    };
    for glyph in glyphs {
        for (_, contour) in &glyph.contours {
            include(fit_path(contour)?.bounding_box());
        }
    }
    for (_, fitted) in fitted_sketch {
        include(fitted.bounding_box());
    }
    let bounds = bounds.unwrap_or(Rect::new(0.0, 0.0, 1000.0, 1000.0));
    Ok(Extent {
        ascent: bounds.max_y(),
        descent: bounds.min_y().min(0.0),
    })
}

/// Map font coordinates (y-up, baseline 0) to SVG (y-down), shifted to
/// `x_offset`.
fn flip_at(x_offset: f64) -> Affine {
    Affine::new([1.0, 0.0, 0.0, -1.0, x_offset, 0.0])
}

fn render_glyph(
    glyph: &Glyph,
    document: &Document,
    x_offset: f64,
    extent: &Extent,
    options: &RenderOptions,
) -> Result<String, RenderError> {
    let flip = flip_at(x_offset);
    let mut body = String::new();
    body.push_str(&format!("  <g id=\"glyph-{}\">\n", escape(&glyph.name)));

    // All contours in one path element: nonzero fill carves the counters.
    let mut outline = BezPath::new();
    let mut fitted_contours = Vec::new();
    for (name, contour) in &glyph.contours {
        let fitted = fit_path(contour)
            .map_err(|error| render_err(format!("contour '{name}': {}", error.message)))?;
        outline.extend(fitted.elements().iter().cloned());
        fitted_contours.push(fitted);
    }
    let mut flipped_outline = outline.clone();
    flipped_outline.apply_affine(flip);
    body.push_str(&format!(
        "    <path d=\"{}\" fill=\"black\" fill-rule=\"nonzero\"/>\n",
        flipped_outline.to_svg()
    ));

    if options.debug {
        body.push_str(&debug_layer(
            glyph,
            document,
            x_offset,
            extent,
            &fitted_contours,
        )?);
    }

    body.push_str("  </g>\n");
    Ok(body)
}

fn debug_layer(
    glyph: &Glyph,
    document: &Document,
    x_offset: f64,
    extent: &Extent,
    fitted_contours: &[BezPath],
) -> Result<String, RenderError> {
    let flip = flip_at(x_offset);
    let mut body = String::from("    <g class=\"debug\" font-family=\"monospace\" font-size=\"18\">\n");
    let to_svg = |point: Point| flip * point;

    // Metrics guidelines across this glyph's advance.
    let x0 = x_offset - 20.0;
    let x1 = x_offset + glyph.advance + 20.0;
    let mut lines = vec![("baseline", 0.0)];
    for key in ["x", "cap", "asc", "desc"] {
        if let Some(value) = document.metrics.get(key) {
            lines.push((key, value));
        }
    }
    for (name, y) in lines {
        let color = if name == "baseline" { "#999" } else { "#ccc" };
        body.push_str(&format!(
            "      <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{color}\" stroke-width=\"1\"/>\n",
            fmt(x0),
            fmt(-y),
            fmt(x1),
            fmt(-y)
        ));
    }
    let _ = extent;

    // Guides.
    for (name, guide) in &glyph.guides {
        let mut fitted = fit_path(guide)
            .map_err(|error| render_err(format!("guide '{name}': {}", error.message)))?;
        fitted.apply_affine(flip);
        body.push_str(&format!(
            "      <path d=\"{}\" fill=\"none\" stroke=\"#888\" stroke-width=\"1\" stroke-dasharray=\"6 4\"/>\n",
            fitted.to_svg()
        ));
    }

    // Skeleton nodes: position, direction arrow, derived edge points.
    for (name, node) in &glyph.nodes {
        let pos = to_svg(node.pos);
        let tip = to_svg(node.pos + 25.0 * unitvec(node.dir));
        let left = to_svg(node.left());
        let right = to_svg(node.right());
        body.push_str(&format!(
            "      <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#d33\" stroke-width=\"1\"/>\n",
            fmt(pos.x),
            fmt(pos.y),
            fmt(tip.x),
            fmt(tip.y)
        ));
        body.push_str(&format!(
            "      <circle cx=\"{}\" cy=\"{}\" r=\"3.5\" fill=\"#d33\"/>\n",
            fmt(pos.x),
            fmt(pos.y)
        ));
        for edge in [left, right] {
            body.push_str(&format!(
                "      <circle cx=\"{}\" cy=\"{}\" r=\"2.5\" fill=\"#36c\"/>\n",
                fmt(edge.x),
                fmt(edge.y)
            ));
        }
        body.push_str(&format!(
            "      <text x=\"{}\" y=\"{}\" fill=\"#d33\">{}</text>\n",
            fmt(pos.x + 6.0),
            fmt(pos.y - 6.0),
            escape(name)
        ));
    }

    // Anchors.
    for (name, anchor) in &glyph.anchors {
        let pos = to_svg(*anchor);
        body.push_str(&format!(
            "      <path d=\"M{} {}l8 8m-8 0l8 -8\" stroke=\"#909\" stroke-width=\"1.5\" fill=\"none\"/>\n",
            fmt(pos.x - 4.0),
            fmt(pos.y - 4.0)
        ));
        body.push_str(&format!(
            "      <text x=\"{}\" y=\"{}\" fill=\"#909\">{}</text>\n",
            fmt(pos.x + 8.0),
            fmt(pos.y + 4.0),
            escape(name)
        ));
    }

    // On-curve points and control handles of the fitted outline.
    for fitted in fitted_contours {
        let mut flipped = fitted.clone();
        flipped.apply_affine(flip);
        body.push_str(&handles(&flipped));
    }

    body.push_str("    </g>\n");
    Ok(body)
}

/// Dots on on-curve points, open circles on control points, thin handle
/// lines. Input is already in SVG coordinates.
fn handles(path: &BezPath) -> String {
    let mut body = String::new();
    let mut current = Point::ZERO;
    let on_curve = |point: Point, body: &mut String| {
        body.push_str(&format!(
            "      <rect x=\"{}\" y=\"{}\" width=\"5\" height=\"5\" fill=\"#222\"/>\n",
            fmt(point.x - 2.5),
            fmt(point.y - 2.5)
        ));
    };
    for element in path.elements() {
        match element {
            PathEl::MoveTo(point) | PathEl::LineTo(point) => {
                on_curve(*point, &mut body);
                current = *point;
            }
            PathEl::CurveTo(control1, control2, end) => {
                for (anchor, control) in [(current, control1), (*end, control2)] {
                    body.push_str(&format!(
                        "      <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#bbb\" stroke-width=\"1\"/>\n",
                        fmt(anchor.x),
                        fmt(anchor.y),
                        fmt(control.x),
                        fmt(control.y)
                    ));
                    body.push_str(&format!(
                        "      <circle cx=\"{}\" cy=\"{}\" r=\"2.5\" fill=\"white\" stroke=\"#888\"/>\n",
                        fmt(control.x),
                        fmt(control.y)
                    ));
                }
                on_curve(*end, &mut body);
                current = *end;
            }
            PathEl::QuadTo(_, end) => {
                on_curve(*end, &mut body);
                current = *end;
            }
            PathEl::ClosePath => {}
        }
    }
    body
}

fn fmt(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded == rounded.trunc() {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{PathPoint, PathValue};
    use kurbo::Point;

    fn smooth(x: f64, y: f64) -> PathPoint {
        PathPoint {
            pos: Point::new(x, y),
            enter: None,
            leave: None,
            corner: false,
        }
    }

    #[test]
    fn fits_a_closed_smooth_path() {
        let path = PathValue {
            points: vec![
                smooth(0.0, 100.0),
                smooth(100.0, 0.0),
                smooth(200.0, 100.0),
                smooth(100.0, 200.0),
            ],
            joints: vec![Joint::Curve; 4],
            closed: true,
        };
        let fitted = fit_path(&path).unwrap();
        assert!(fitted.elements().len() >= 5); // move + 4 curves (+ close)
        let PathEl::MoveTo(start) = fitted.elements()[0] else {
            panic!("expected MoveTo")
        };
        assert!((start.x - 0.0).abs() < 1e-6 && (start.y - 100.0).abs() < 1e-6);
    }

    #[test]
    fn direction_constraint_shapes_the_tangent() {
        // Leave the first point straight up: the first control point must
        // sit directly above the start.
        let path = PathValue {
            points: vec![
                PathPoint {
                    pos: Point::new(0.0, 0.0),
                    enter: None,
                    leave: Some(90.0),
                    corner: false,
                },
                smooth(100.0, 100.0),
            ],
            joints: vec![Joint::Curve],
            closed: false,
        };
        let fitted = fit_path(&path).unwrap();
        let PathEl::CurveTo(control1, _, _) = fitted.elements()[1] else {
            panic!("expected CurveTo, got {:?}", fitted.elements()[1])
        };
        assert!(control1.x.abs() < 1e-6, "control1 = {control1:?}");
        assert!(control1.y > 0.0);
    }

    #[test]
    fn line_only_rectangle_stays_straight() {
        let path = PathValue {
            points: vec![
                smooth(0.0, 0.0),
                smooth(100.0, 0.0),
                smooth(100.0, 200.0),
                smooth(0.0, 200.0),
            ],
            joints: vec![Joint::Line; 4],
            closed: true,
        };
        let fitted = fit_path(&path).unwrap();
        // Whatever the element types, the outline must not bulge: its
        // bounding box equals the rectangle.
        let bounds = fitted.bounding_box();
        assert!((bounds.min_x() - 0.0).abs() < 1e-6);
        assert!((bounds.max_x() - 100.0).abs() < 1e-6);
        assert!((bounds.max_y() - 200.0).abs() < 1e-6);
    }

    #[test]
    fn line_into_constrained_curve_stays_straight() {
        // The slab-serif bracket shape: a vertical -- line meeting a curve
        // whose start direction is constrained. The line must stay straight.
        let path = PathValue {
            points: vec![
                smooth(0.0, 100.0),
                PathPoint {
                    pos: Point::new(0.0, 50.0),
                    enter: None,
                    leave: Some(270.0),
                    corner: false,
                },
                PathPoint {
                    pos: Point::new(-30.0, 0.0),
                    enter: Some(180.0),
                    leave: None,
                    corner: false,
                },
            ],
            joints: vec![Joint::Line, Joint::Curve],
            closed: false,
        };
        let fitted = fit_path(&path).unwrap();
        // First segment (0,100) -> (0,50): every control must sit on x = 0.
        match fitted.elements()[1] {
            PathEl::LineTo(end) => assert!((end.x).abs() < 1e-6),
            PathEl::CurveTo(control1, control2, end) => {
                assert!(control1.x.abs() < 1e-6, "control1 = {control1:?}");
                assert!(control2.x.abs() < 1e-6, "control2 = {control2:?}");
                assert!(end.x.abs() < 1e-6);
            }
            other => panic!("unexpected element {other:?}"),
        }
    }

    #[test]
    fn line_between_curves_stays_straight() {
        let path = PathValue {
            points: vec![
                smooth(0.0, 0.0),
                smooth(50.0, 50.0),
                smooth(100.0, 50.0),
                smooth(150.0, 0.0),
            ],
            joints: vec![Joint::Curve, Joint::Line, Joint::Curve],
            closed: false,
        };
        let fitted = fit_path(&path).unwrap();
        // Segment points[1] -> points[2] is horizontal at y = 50.
        match fitted.elements()[2] {
            PathEl::LineTo(end) => assert!((end.y - 50.0).abs() < 1e-6),
            PathEl::CurveTo(control1, control2, end) => {
                assert!((control1.y - 50.0).abs() < 1e-6, "control1 = {control1:?}");
                assert!((control2.y - 50.0).abs() < 1e-6, "control2 = {control2:?}");
                assert!((end.y - 50.0).abs() < 1e-6);
            }
            other => panic!("unexpected element {other:?}"),
        }
    }

    #[test]
    fn single_point_path_is_an_error() {
        let path = PathValue {
            points: vec![smooth(0.0, 0.0)],
            joints: vec![],
            closed: false,
        };
        assert!(fit_path(&path).is_err());
    }
}
