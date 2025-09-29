//! Evaluator - Stage 2: Evaluate IR with full context

use crate::ir::*;
use crate::svg_generator::SvgGenerator;
use crate::value::Value;
use curve_fitter::{CurveFitter, InputPoint, PointType};
use kurbo::{BezPath, Cap, Join, PathEl, Point, Stroke, stroke};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ControlPoint {
    pub point: Point,
    pub label: String,
    pub is_control: bool,
}

pub struct Evaluator {
    variables: HashMap<String, Value>,
    drawn_paths: Vec<BezPath>,
}

impl Evaluator {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            drawn_paths: Vec::new(),
        }
    }

    pub fn evaluate(&mut self, program: Program) -> Result<(), Box<dyn std::error::Error>> {
        // First pass: resolve all variable assignments
        for statement in &program.statements {
            if let Statement::Assignment { name, expression } = statement {
                let value = self.evaluate_expression(expression)?;
                self.variables.insert(name.clone(), value);
            }
        }

        // Second pass: execute function calls
        for statement in &program.statements {
            if let Statement::FunctionCall { name, args } = statement {
                self.execute_function(name, args)?;
            }
        }

        Ok(())
    }

    pub fn generate_svg(&self, debug: bool) -> String {
        let (point_labels, control_points) = if debug {
            let point_labels = self.collect_point_labels();
            let control_points = self.collect_control_points();
            (point_labels, control_points)
        } else {
            (Vec::new(), Vec::new())
        };
        SvgGenerator::generate_svg(&self.drawn_paths, &point_labels, &control_points)
    }

    fn evaluate_expression(
        &self,
        expression: &Expression,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        match expression {
            Expression::Point { x, y } => Ok(Value::Point(Point::new(*x, *y))),
            Expression::Path(path_expr) => {
                let bez_path = self.evaluate_path_expression(path_expr)?;
                Ok(Value::Path(bez_path))
            }
            Expression::Variable(name) => self
                .variables
                .get(name)
                .cloned()
                .ok_or_else(|| format!("Undefined variable: {}", name).into()),
        }
    }

    fn evaluate_path_expression(
        &self,
        path_expr: &PathExpression,
    ) -> Result<BezPath, Box<dyn std::error::Error>> {
        let points = self.resolve_all_points(path_expr)?;
        let connectors = self.extract_connectors(path_expr);

        self.construct_optimized_path(points, connectors)
    }

    fn resolve_all_points(
        &self,
        path_expr: &PathExpression,
    ) -> Result<Vec<Point>, Box<dyn std::error::Error>> {
        let mut points = Vec::new();

        for segment in &path_expr.segments {
            match segment {
                PathSegment::Point(point_ref) => {
                    points.push(self.resolve_point_ref(point_ref)?);
                }
                PathSegment::Connection { to, .. } => {
                    points.push(self.resolve_point_ref(to)?);
                }
            }
        }

        Ok(points)
    }

    fn extract_connectors(&self, path_expr: &PathExpression) -> Vec<Option<Connector>> {
        let mut connectors = Vec::new();

        for segment in &path_expr.segments {
            match segment {
                PathSegment::Point(_) => {
                    // First point has no incoming connector
                    if connectors.is_empty() {
                        connectors.push(None);
                    }
                }
                PathSegment::Connection { connector, .. } => {
                    connectors.push(Some(connector.clone()));
                }
            }
        }

        connectors
    }

    fn construct_optimized_path(
        &self,
        points: Vec<Point>,
        connectors: Vec<Option<Connector>>,
    ) -> Result<BezPath, Box<dyn std::error::Error>> {
        let mut bez_path = BezPath::new();

        if points.is_empty() {
            return Ok(bez_path);
        }

        bez_path.push(PathEl::MoveTo(points[0]));

        if points.len() == 1 {
            return Ok(bez_path);
        }

        // Now we can pass all points to hobby at once since we treat lines as curves
        self.do_curve_fitting(&mut bez_path, &points, &connectors)?;

        Ok(bez_path)
    }

    fn do_curve_fitting(
        &self,
        bez_path: &mut BezPath,
        points: &[Point],
        connectors: &[Option<Connector>],
    ) -> Result<(), Box<dyn std::error::Error>> {
        if points.len() < 2 {
            return Ok(());
        }

        let mut input_points: Vec<InputPoint> = points
            .iter()
            .map(|p| InputPoint {
                x: p.x,
                y: p.y,
                point_type: PointType::Smooth,
            })
            .collect();

        for (i, connector_opt) in connectors.iter().enumerate() {
            if let Some(connector) = connector_opt {
                match connector {
                    Connector::Line => {
                        dbg!(i);
                        input_points[i - 1].point_type = PointType::Corner;
                    }
                    _ => {}
                }
            }
        }

        let fitter = CurveFitter::new();
        let segments = fitter.fit_curve(input_points, false);
        bez_path.extend(segments.unwrap());
        Ok(())
    }

    fn resolve_point_ref(&self, point_ref: &PointRef) -> Result<Point, Box<dyn std::error::Error>> {
        match point_ref {
            PointRef::Literal { x, y } => Ok(Point::new(*x, *y)),
            PointRef::Variable(name) => match self.variables.get(name) {
                Some(Value::Point(point)) => Ok(*point),
                Some(_) => Err(format!("Variable {} is not a point", name).into()),
                None => Err(format!("Undefined variable: {}", name).into()),
            },
        }
    }

    fn execute_function(
        &mut self,
        name: &str,
        args: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        match name {
            "draw" => self.handle_draw_function(args)?,
            "stroke" => self.handle_stroke_function(args)?,
            _ => return Err(format!("Unknown function: {}", name).into()),
        }
        Ok(())
    }

    fn handle_draw_function(&mut self, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        for arg_name in args {
            if let Some(Value::Path(path)) = self.variables.get(arg_name) {
                self.drawn_paths.push(path.clone());
            } else {
                return Err(
                    format!("Unknown argument to draw. {} is not defined", arg_name).into(),
                );
            }
        }
        Ok(())
    }

    fn handle_stroke_function(
        &mut self,
        args: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        const STROKE_WIDTH: f64 = 30.0;
        let tolerance = 1e-1;
        let style = Stroke::new(STROKE_WIDTH)
            .with_caps(Cap::Round)
            .with_join(Join::Round);

        for arg_name in args {
            if let Some(Value::Path(path)) = self.variables.get(arg_name) {
                self.drawn_paths
                    .push(stroke(path.clone(), &style, &Default::default(), tolerance));
            } else {
                return Err(
                    format!("Unknown argument to stroke. {} is not defined", arg_name).into(),
                );
            }
        }
        Ok(())
    }

    fn collect_point_labels(&self) -> Vec<(Point, String)> {
        let mut labels: Vec<(Point, String)> = Vec::new();

        for (name, value) in &self.variables {
            if let Value::Point(point) = value {
                let label = format!("{}({:.0},{:.0})", name, point.x, point.y);
                labels.push((*point, label));
            }
        }

        labels
    }

    fn collect_control_points(&self) -> Vec<ControlPoint> {
        let mut control_points = Vec::new();
        let mut curve_counter = 0;

        for path in &self.drawn_paths {
            for element in path.elements() {
                match element {
                    PathEl::CurveTo(cp1, cp2, _end) => {
                        // Add control points
                        control_points.push(ControlPoint {
                            point: *cp1,
                            label: format!("c{}a({:.0},{:.0})", curve_counter, cp1.x, cp1.y),
                            is_control: true,
                        });
                        control_points.push(ControlPoint {
                            point: *cp2,
                            label: format!("c{}b({:.0},{:.0})", curve_counter, cp2.x, cp2.y),
                            is_control: true,
                        });

                        curve_counter += 1;
                    }
                    _ => {}
                }
            }
        }

        control_points
    }
}
