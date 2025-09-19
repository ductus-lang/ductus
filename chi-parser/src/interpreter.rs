use bezier_solver::BezierSolver;
use kurbo::{BezPath, Cap, DEFAULT_ACCURACY, Join, PathEl, Point, Stroke, stroke};
use std::collections::HashMap;
use tree_sitter::Node;

use crate::svg_generator::SvgGenerator;
use crate::value::Value;

#[derive(Debug, Clone)]
pub struct ControlPoint {
    pub point: Point,
    pub label: String,
    pub is_control: bool,
}

pub struct Interpreter {
    variables: HashMap<String, Value>,
    drawn_paths: Vec<BezPath>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            drawn_paths: Vec::new(),
        }
    }

    pub fn interpret(
        &mut self,
        node: Node,
        source: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match node.kind() {
            "source_file" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.interpret(child, source)?;
                }
            }
            "assignment_statement" => self.handle_assignment(node, source)?,
            "function_call" => self.handle_function_call(node, source)?,
            "comment" => {} // ignore comments
            _ => {}
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
                    PathEl::CurveTo(cp1, cp2, end) => {
                        // Add control points
                        control_points.push(ControlPoint {
                            point: *cp1,
                            label: format!("c{}({:.0},{:.0})", curve_counter, cp1.x, cp1.y),
                            is_control: true,
                        });
                        control_points.push(ControlPoint {
                            point: *cp2,
                            label: format!("c{}({:.0},{:.0})", curve_counter, cp2.x, cp2.y),
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

    fn handle_assignment(
        &mut self,
        node: Node,
        source: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        if children.len() >= 3 {
            let identifier = children[0];
            let expression = children[2]; // Skip the '=' token

            let var_name = node_text(identifier, source);
            let value = self.evaluate_expression(expression, source)?;

            self.variables.insert(var_name, value);
        }

        Ok(())
    }

    fn handle_function_call(
        &mut self,
        node: Node,
        source: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        if let Some(function_name_node) = children.first() {
            let function_name = node_text(*function_name_node, source);
            dbg!(&function_name);

            match function_name.as_str() {
                "draw" => self.handle_draw_function(&children, source)?,
                "stroke" => self.handle_stroke_function(&children, source)?,
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_draw_function(
        &mut self,
        children: &[Node],
        source: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for child in children.iter().skip(1) {
            if child.kind() == "function_args" {
                let mut args_cursor = child.walk();
                for arg in child.children(&mut args_cursor) {
                    if arg.kind() == "identifier" {
                        let arg_name = node_text(arg, source);
                        if let Some(Value::Path(path)) = self.variables.get(&arg_name) {
                            self.drawn_paths.push(path.clone());
                        } else {
                            return Err(format!(
                                "Unknown argument to draw. {} is not defined",
                                arg_name
                            )
                            .into());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_stroke_function(
        &mut self,
        children: &[Node],
        source: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        const STROKE_WIDTH: f64 = 30.;
        let tolerance = DEFAULT_ACCURACY;
        let style = Stroke::new(STROKE_WIDTH)
            .with_caps(Cap::Round)
            .with_join(Join::Round);

        for child in children.iter().skip(1) {
            if child.kind() == "function_args" {
                let mut args_cursor = child.walk();
                for arg in child.children(&mut args_cursor) {
                    if arg.kind() == "identifier" {
                        let arg_name = node_text(arg, source);
                        if let Some(Value::Path(path)) = self.variables.get(&arg_name) {
                            self.drawn_paths.push(stroke(
                                path.clone(),
                                &style,
                                &Default::default(),
                                tolerance,
                            ));
                        } else {
                            return Err(format!(
                                "Unknown argument to stroke. {} is not defined",
                                arg_name
                            )
                            .into());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn evaluate_expression(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        match node.kind() {
            "expression" => {
                // Expression is a wrapper, evaluate its child
                let mut cursor = node.walk();
                if let Some(child) = node.children(&mut cursor).next() {
                    self.evaluate_expression(child, source)
                } else {
                    Err("Empty expression".into())
                }
            }
            "point" => self.evaluate_point(node, source),
            "path" => self.evaluate_path(node, source),
            "identifier" => {
                let name = node_text(node, source);
                self.variables
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| format!("Undefined variable: {}", name).into())
            }
            _ => Err(format!("Unknown expression type: {}", node.kind()).into()),
        }
    }

    fn evaluate_point(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        let mut x = 0.0;
        let mut y = 0.0;

        for child in children {
            match child.kind() {
                "x" => {
                    x = node_text(child, source).parse()?;
                }
                "y" => {
                    y = node_text(child, source).parse()?;
                }
                _ => {}
            }
        }

        Ok(Value::Point(Point { x, y }))
    }

    fn evaluate_path(&self, node: Node, source: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let mut bez_path = BezPath::new();
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        let mut i = 0;
        while i < children.len() {
            let child = children[i];

            match child.kind() {
                "point" => {
                    let point_value = self.evaluate_expression(child, source)?;
                    if let Value::Point(point) = point_value {
                        if bez_path.is_empty() {
                            bez_path.push(PathEl::MoveTo(point));
                        } else {
                            bez_path.push(PathEl::LineTo(point));
                        }
                    }
                }
                "identifier" => {
                    let point = self.get_point_from_variable(child, source)?;
                    if bez_path.is_empty() {
                        bez_path.push(PathEl::MoveTo(point));
                    } else {
                        bez_path.push(PathEl::LineTo(point));
                    }
                }
                "connector" => {
                    i = self.handle_connector(&mut bez_path, child, &children, i, source)?;
                }
                _ => {}
            }
            i += 1;
        }

        Ok(Value::Path(bez_path))
    }

    fn handle_connector(
        &self,
        bez_path: &mut BezPath,
        connector_node: Node,
        children: &[Node],
        current_index: usize,
        source: &str,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let mut conn_cursor = connector_node.walk();

        for conn_child in connector_node.children(&mut conn_cursor) {
            match conn_child.kind() {
                "line_connector" => {
                    return self.handle_line_connector(bez_path, children, current_index, source);
                }
                "curve_connector" => {
                    return self.handle_curve_connector(
                        bez_path,
                        conn_child,
                        children,
                        current_index,
                        source,
                    );
                }
                _ => {}
            }
        }

        Ok(current_index)
    }

    fn handle_line_connector(
        &self,
        bez_path: &mut BezPath,
        children: &[Node],
        current_index: usize,
        source: &str,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let next_index = current_index + 1;
        if next_index < children.len() {
            let next_child = children[next_index];
            if next_child.kind() == "identifier" {
                let point = self.get_point_from_variable(next_child, source)?;
                bez_path.push(PathEl::LineTo(point));
                return Ok(next_index);
            }
        }
        Ok(current_index)
    }

    fn handle_curve_connector(
        &self,
        bez_path: &mut BezPath,
        curve_connector_node: Node,
        children: &[Node],
        current_index: usize,
        source: &str,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let (in_angle, out_angle) = self.extract_curve_options(curve_connector_node, source)?;

        let next_index = current_index + 1;
        if next_index < children.len() {
            let next_child = children[next_index];
            if next_child.kind() == "identifier" {
                let end_point = self.get_point_from_variable(next_child, source)?;
                let start_point = self.get_current_point(bez_path);
                let curve_element =
                    self.create_curve_element(start_point, end_point, in_angle, out_angle)?;
                dbg!(curve_element);
                bez_path.push(curve_element);
                return Ok(next_index);
            }
        }
        Ok(current_index)
    }

    fn extract_curve_options(
        &self,
        curve_connector_node: Node,
        source: &str,
    ) -> Result<(Option<f64>, Option<f64>), Box<dyn std::error::Error>> {
        let mut in_angle = None;
        let mut out_angle = None;

        let mut curve_cursor = curve_connector_node.walk();
        for curve_child in curve_connector_node.children(&mut curve_cursor) {
            if curve_child.kind() == "node_options" {
                let mut opt_cursor = curve_child.walk();
                for option in curve_child.children(&mut opt_cursor) {
                    if option.kind() == "option" {
                        let (name, value) = self.parse_option(option, source)?;
                        match name.as_str() {
                            "in" => in_angle = Some(value.parse::<f64>()?),
                            "out" => out_angle = Some(value.parse::<f64>()?),
                            _ => {} // ignore unknown options
                        }
                    }
                }
            }
        }

        Ok((in_angle, out_angle))
    }

    fn parse_option(
        &self,
        node: Node,
        source: &str,
    ) -> Result<(String, String), Box<dyn std::error::Error>> {
        let mut cursor = node.walk();
        let mut name = String::new();
        let mut value = String::new();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "option_name" => name = node_text(child, source),
                "option_value" => value = node_text(child, source),
                _ => {}
            }
        }

        Ok((name, value))
    }

    fn get_current_point(&self, path: &BezPath) -> Point {
        // Get the last point in the path, or origin if empty
        path.elements()
            .last()
            .and_then(|el| match el {
                PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(*p),
                PathEl::CurveTo(_, _, p) => Some(*p),
                _ => None,
            })
            .unwrap_or(Point::ZERO)
    }

    fn create_curve_element(
        &self,
        start: Point,
        end: Point,
        in_angle: Option<f64>,
        out_angle: Option<f64>,
    ) -> Result<PathEl, Box<dyn std::error::Error>> {
        let theta0 = out_angle.unwrap_or(0.0).to_radians();
        let theta1 = in_angle.unwrap_or(0.0).to_radians();

        let solver = BezierSolver::new(100, 1e-6);
        let (delta0, delta1) = solver.solve(theta0, theta1);

        let distance = start.distance(end);
        let cp1 = Point::new(
            start.x + delta0 * distance * theta0.cos(),
            start.y + delta0 * distance * theta0.sin(),
        );
        let cp2 = Point::new(
            end.x - delta1 * distance * theta1.cos(),
            end.y - delta1 * distance * theta1.sin(),
        );

        Ok(PathEl::CurveTo(cp1, cp2, end))
    }

    fn get_point_from_variable(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Point, Box<dyn std::error::Error>> {
        let name = node_text(node, source);
        dbg!(&self.variables);
        match self.variables.get(&name) {
            Some(Value::Point(point)) => Ok(point.clone()),
            Some(_) => Err(format!("Variable {} is not a point", name).into()),
            None => Err(format!("Undefined variable: {}", name).into()),
        }
    }
}

fn node_text(node: Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}
