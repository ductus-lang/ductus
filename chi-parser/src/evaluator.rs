//! Evaluator - Stage 2: Evaluate IR with full context

use crate::ir::*;
use crate::svg_generator::SvgGenerator;
use crate::value::Value;
use bezier_solver::BezierSolver;
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
        // NOW we have full context!
        let points = self.resolve_all_points(path_expr)?;
        let connectors = self.extract_connectors(path_expr);

        // Global optimization opportunities:
        // 1. Smooth curves considering all points
        // 2. Optimize tension based on path geometry
        // 3. Auto-detect intentions (e.g., closed loops)

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

        for i in 1..points.len() {
            let start = points[i - 1];
            let end = points[i];
            let connector = connectors.get(i - 1).and_then(|c| c.as_ref());

            match connector {
                Some(Connector::Line) => {
                    bez_path.push(PathEl::LineTo(end));
                }
                Some(Connector::Curve {
                    start_options,
                    end_options,
                }) => {
                    // Now we can consider neighboring points for better curves!
                    let curve_element = self.create_contextual_curve(
                        start,
                        end,
                        start_options,
                        end_options,
                        &points,
                        i, // Pass full context
                    )?;
                    bez_path.push(curve_element);
                }
                None => {
                    // Default behavior - could be smart based on context
                    bez_path.push(PathEl::LineTo(end));
                }
            }
        }

        Ok(bez_path)
    }

    fn create_contextual_curve(
        &self,
        start: Point,
        end: Point,
        out_options: &CurveOptions,
        in_options: &CurveOptions,
        all_points: &[Point],
        current_index: usize,
    ) -> Result<PathEl, Box<dyn std::error::Error>> {
        // NOW we can make intelligent decisions based on:
        // - Previous point (if exists)
        // - Next point (if exists)
        // - Overall path shape
        // - Curvature continuity

        let _prev_point = if current_index > 1 {
            Some(all_points[current_index - 2])
        } else {
            None
        };

        let _next_point = if current_index < all_points.len() - 1 {
            Some(all_points[current_index + 1])
        } else {
            None
        };

        // For now, use the same logic as before but with the new structure
        let out_angle = out_options.direction;
        let in_angle = in_options.direction;

        // Convert SVG angles to Cartesian angles for the BezierSolver
        let theta0 = out_angle
            .map(Self::svg_angle_to_cartesian_radians)
            .unwrap_or(0.0);
        let theta1 = in_angle
            .map(Self::svg_angle_to_cartesian_radians)
            .unwrap_or(0.0);

        let solver = BezierSolver::new(100, 1e-6);
        let (delta0, delta1) = solver.solve(theta0, theta1);

        let distance = start.distance(end);

        // Calculate control points accounting for SVG coordinate system
        let cp1 = Self::calculate_control_point_svg(start, delta0 * distance, theta0, true);
        let cp2 = Self::calculate_control_point_svg(end, delta1 * distance, theta1, false);

        Ok(PathEl::CurveTo(cp1, cp2, end))
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

    /// Convert SVG angle (degrees) to Cartesian angle (radians)
    /// SVG coordinates have Y increasing downward, while Cartesian has Y increasing upward
    fn svg_angle_to_cartesian_radians(svg_angle_degrees: f64) -> f64 {
        -svg_angle_degrees.to_radians()
    }

    /// Calculate control point position accounting for SVG coordinate system
    /// In SVG, Y increases downward, so we need to negate the Y component
    fn calculate_control_point_svg(
        anchor: Point,
        distance: f64,
        angle_radians: f64,
        outward: bool,
    ) -> Point {
        let sign = if outward { 1.0 } else { -1.0 };
        Point::new(
            anchor.x + sign * distance * angle_radians.cos(),
            anchor.y - sign * distance * angle_radians.sin(), // Negate Y for SVG coordinates
        )
    }
}
