//! The evaluated document: everything the renderer needs.

use crate::value::{NodeValue, PathValue};
use kurbo::Point;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Document {
    pub params: Vec<Param>,
    pub metrics: Metrics,
    pub glyphs: Vec<Glyph>,
    pub sketch: Vec<SketchCommand>,
}

/// A design-space axis with its resolved value for this instance.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub value: f64,
    pub default: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Default)]
pub struct Metrics {
    pub values: HashMap<String, f64>,
}

impl Metrics {
    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.get(name).copied()
    }

    pub fn upm(&self) -> f64 {
        self.get("upm").unwrap_or(1000.0)
    }
}

#[derive(Debug)]
pub struct Glyph {
    pub name: String,
    pub unicode: Option<u32>,
    pub advance: f64,
    pub contours: Vec<(String, PathValue)>,
    pub guides: Vec<(String, PathValue)>,
    pub anchors: Vec<(String, Point)>,
    /// Skeleton nodes bound in the glyph body, for debug rendering.
    pub nodes: Vec<(String, NodeValue)>,
}

#[derive(Debug)]
pub enum SketchCommand {
    Draw(PathValue),
    Fill(PathValue),
}
