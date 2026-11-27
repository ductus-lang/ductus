//! Convert an evaluated Ductus `Document` into a UFO (`norad::Font`).

use ductus_core::doc::{Document, Glyph};
use ductus_core::render::fit_path;
use kurbo::{BezPath, PathEl, Point};
use norad::{Anchor, Contour, ContourPoint, Font, Name, PointType};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct UfoOptions {
    pub family_name: Option<String>,
    pub style_name: Option<String>,
}

#[derive(Debug)]
pub enum UfoError {
    Render { glyph: String, message: String },
    InvalidName { name: String, message: String },
    Save(String),
}

impl std::fmt::Display for UfoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UfoError::Render { glyph, message } => {
                write!(f, "glyph '{glyph}': {message}")
            }
            UfoError::InvalidName { name, message } => {
                write!(f, "invalid UFO name '{name}': {message}")
            }
            UfoError::Save(message) => write!(f, "failed to save UFO: {message}"),
        }
    }
}

impl std::error::Error for UfoError {}

/// Convert an evaluated Ductus `Document` into a `norad::Font`.
pub fn document_to_ufo(document: &Document, options: &UfoOptions) -> Result<Font, UfoError> {
    let mut font = Font::new();
    apply_metrics(&mut font, document, options);

    let layer = font.default_layer_mut();
    for glyph in &document.glyphs {
        let ufo_glyph = convert_glyph(glyph)?;
        layer.insert_glyph(ufo_glyph);
    }

    Ok(font)
}

/// Convert a `Document` to a UFO and save it to `path`.
pub fn save_ufo(document: &Document, options: &UfoOptions, path: &Path) -> Result<(), UfoError> {
    let font = document_to_ufo(document, options)?;
    font.save(path)
        .map_err(|err| UfoError::Save(err.to_string()))
}

fn apply_metrics(font: &mut Font, document: &Document, options: &UfoOptions) {
    let info = &mut font.font_info;
    let metrics = &document.metrics;

    info.units_per_em = Some((metrics.upm().round().max(1.0) as u32).into());
    if let Some(asc) = metrics.get("asc") {
        info.ascender = Some(asc);
    }
    if let Some(desc) = metrics.get("desc") {
        info.descender = Some(desc);
    }
    if let Some(x) = metrics.get("x") {
        info.x_height = Some(x);
    }
    if let Some(cap) = metrics.get("cap") {
        info.cap_height = Some(cap);
    }
    if let Some(family) = &options.family_name {
        info.family_name = Some(family.clone());
    }
    if let Some(style) = &options.style_name {
        info.style_name = Some(style.clone());
    }
}

fn convert_glyph(glyph: &Glyph) -> Result<norad::Glyph, UfoError> {
    let name = Name::new(&glyph.name).map_err(|err| UfoError::InvalidName {
        name: glyph.name.clone(),
        message: err.to_string(),
    })?;
    let mut ufo_glyph = norad::Glyph::new(&name);
    ufo_glyph.width = glyph.advance;

    if let Some(cp) = glyph.unicode
        && let Some(ch) = char::from_u32(cp)
    {
        ufo_glyph.codepoints.insert(ch);
    }

    for (_contour_name, path) in &glyph.contours {
        let bez = fit_path(path).map_err(|err| UfoError::Render {
            glyph: glyph.name.clone(),
            message: err.message,
        })?;
        ufo_glyph.contours.extend(bezpath_to_contours(&bez));
    }

    for (anchor_name, pos) in &glyph.anchors {
        let name = Name::new(anchor_name).map_err(|err| UfoError::InvalidName {
            name: anchor_name.clone(),
            message: err.to_string(),
        })?;
        ufo_glyph
            .anchors
            .push(Anchor::new(pos.x, pos.y, Some(name), None, None));
    }

    Ok(ufo_glyph)
}

/// Convert a `kurbo::BezPath` (possibly containing multiple subpaths) into UFO contours.
fn bezpath_to_contours(bez: &BezPath) -> Vec<Contour> {
    let mut contours = Vec::new();
    let mut points: Vec<ContourPoint> = Vec::new();
    let mut start: Option<Point> = None;

    let flush = |contours: &mut Vec<Contour>,
                 points: &mut Vec<ContourPoint>,
                 start: &mut Option<Point>,
                 closed: bool| {
        if points.is_empty() && start.is_none() {
            return;
        }
        if closed {
            // If the final segment didn't already land on `start`, add an explicit
            // closing Line point back to `start`.
            if let Some(p0) = *start {
                let ends_at_start = points
                    .last()
                    .is_some_and(|p| (p.x - p0.x).abs() < 1e-6 && (p.y - p0.y).abs() < 1e-6);
                if !ends_at_start {
                    points.push(ContourPoint::new(
                        p0.x,
                        p0.y,
                        PointType::Line,
                        false,
                        None,
                        None,
                    ));
                }
            }
            mark_smooth_points(points);
        } else if let Some(p0) = *start {
            points.insert(
                0,
                ContourPoint::new(p0.x, p0.y, PointType::Move, false, None, None),
            );
        }

        if !points.is_empty() {
            contours.push(Contour::new(std::mem::take(points), None));
        }
        *start = None;
    };

    for el in bez.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                flush(&mut contours, &mut points, &mut start, false);
                start = Some(p);
            }
            PathEl::LineTo(p) => {
                points.push(ContourPoint::new(
                    p.x,
                    p.y,
                    PointType::Line,
                    false,
                    None,
                    None,
                ));
            }
            PathEl::QuadTo(c, p) => {
                // Promote quadratic to cubic if one ever appears.
                let prev = points
                    .last()
                    .map(|pt| Point::new(pt.x, pt.y))
                    .or(start)
                    .unwrap_or(p);
                let c1 = prev + (c - prev) * (2.0 / 3.0);
                let c2 = p + (c - p) * (2.0 / 3.0);
                points.push(ContourPoint::new(
                    c1.x,
                    c1.y,
                    PointType::OffCurve,
                    false,
                    None,
                    None,
                ));
                points.push(ContourPoint::new(
                    c2.x,
                    c2.y,
                    PointType::OffCurve,
                    false,
                    None,
                    None,
                ));
                points.push(ContourPoint::new(
                    p.x,
                    p.y,
                    PointType::Curve,
                    false,
                    None,
                    None,
                ));
            }
            PathEl::CurveTo(c1, c2, p) => {
                points.push(ContourPoint::new(
                    c1.x,
                    c1.y,
                    PointType::OffCurve,
                    false,
                    None,
                    None,
                ));
                points.push(ContourPoint::new(
                    c2.x,
                    c2.y,
                    PointType::OffCurve,
                    false,
                    None,
                    None,
                ));
                points.push(ContourPoint::new(
                    p.x,
                    p.y,
                    PointType::Curve,
                    false,
                    None,
                    None,
                ));
            }
            PathEl::ClosePath => {
                flush(&mut contours, &mut points, &mut start, true);
            }
        }
    }

    flush(&mut contours, &mut points, &mut start, false);
    contours
}

/// Mark on-curve points as `smooth` when incoming and outgoing handles are collinear.
fn mark_smooth_points(points: &mut [ContourPoint]) {
    let n = points.len();
    if n < 3 {
        return;
    }
    for i in 0..n {
        if !matches!(points[i].typ, PointType::Curve | PointType::Line) {
            continue;
        }
        let prev = &points[(i + n - 1) % n];
        let next = &points[(i + 1) % n];
        // Only smooth when at least one side is off-curve and tangents align.
        if prev.typ != PointType::OffCurve && next.typ != PointType::OffCurve {
            continue;
        }
        let in_vec = (points[i].x - prev.x, points[i].y - prev.y);
        let out_vec = (next.x - points[i].x, next.y - points[i].y);
        let in_len = (in_vec.0 * in_vec.0 + in_vec.1 * in_vec.1).sqrt();
        let out_len = (out_vec.0 * out_vec.0 + out_vec.1 * out_vec.1).sqrt();
        if in_len > 1e-6 && out_len > 1e-6 {
            let dot = (in_vec.0 * out_vec.0 + in_vec.1 * out_vec.1) / (in_len * out_len);
            if dot > 0.999 {
                points[i].smooth = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::eval::NoImporter;
    use std::collections::HashMap;

    fn eval_ductus(source: &str) -> Document {
        let tree = ductus_core::parse::parse(source).unwrap();
        let program = ductus_core::lower::lower(&tree, source).unwrap();
        ductus_core::eval::evaluate(&program, &HashMap::new(), &NoImporter).unwrap()
    }

    #[test]
    fn converts_glyph_metrics_anchors_and_contours() {
        let src = r#"
            metrics {
              upm = 1000;
              asc = 750;
              desc = -250;
              x = 500;
              cap = 700;
            }
            glyph a : U+0061 {
              advance = 600;
              anchor top = (300, 520);
              contour box = (50, 0) -- (550, 0) -- (550, 500) -- (50, 500) -- cycle;
            }
        "#;
        let doc = eval_ductus(src);
        let font = document_to_ufo(
            &doc,
            &UfoOptions {
                family_name: Some("Test".into()),
                style_name: Some("Regular".into()),
            },
        )
        .unwrap();

        assert_eq!(font.font_info.family_name.as_deref(), Some("Test"));
        assert_eq!(font.font_info.style_name.as_deref(), Some("Regular"));
        assert_eq!(font.font_info.ascender, Some(750.0));
        assert_eq!(font.font_info.descender, Some(-250.0));
        assert_eq!(font.font_info.x_height, Some(500.0));
        assert_eq!(font.font_info.cap_height, Some(700.0));

        let glyph = font.default_layer().get_glyph("a").unwrap();
        assert_eq!(glyph.width, 600.0);
        assert!(glyph.codepoints.contains('a'));
        assert_eq!(glyph.anchors.len(), 1);
        assert_eq!(glyph.anchors[0].x, 300.0);
        assert_eq!(glyph.anchors[0].y, 520.0);
        assert_eq!(glyph.contours.len(), 1);
        let on_curve = glyph.contours[0]
            .points
            .iter()
            .filter(|p| p.typ != PointType::OffCurve)
            .count();
        assert_eq!(on_curve, 4);
        assert_ne!(glyph.contours[0].points[0].typ, PointType::Move);
    }
}
