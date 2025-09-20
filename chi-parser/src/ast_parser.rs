//! AST Parser - Stage 1: Convert tree-sitter AST to IR

use crate::ir::*;
use tree_sitter::Node;

pub struct AstParser;

impl AstParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, node: Node, source: &str) -> Result<Program, Box<dyn std::error::Error>> {
        let mut statements = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "assignment_statement" => {
                    statements.push(self.parse_assignment(child, source)?);
                }
                "function_call" => {
                    statements.push(self.parse_function_call(child, source)?);
                }
                "comment" => {} // ignore comments
                _ => {}
            }
        }

        Ok(Program { statements })
    }

    fn parse_assignment(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Statement, Box<dyn std::error::Error>> {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        if children.len() >= 3 {
            let identifier = children[0];
            let expression = children[2]; // Skip the '=' token

            let var_name = node_text(identifier, source);
            let expression = self.parse_expression(expression, source)?;

            return Ok(Statement::Assignment {
                name: var_name,
                expression,
            });
        }

        Err("Invalid assignment statement".into())
    }

    fn parse_function_call(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Statement, Box<dyn std::error::Error>> {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        if let Some(function_name_node) = children.first() {
            let function_name = node_text(*function_name_node, source);
            let mut args = Vec::new();

            for child in children.iter().skip(1) {
                if child.kind() == "function_args" {
                    let mut args_cursor = child.walk();
                    for arg in child.children(&mut args_cursor) {
                        if arg.kind() == "identifier" {
                            let arg_name = node_text(arg, source);
                            args.push(arg_name);
                        }
                    }
                }
            }

            return Ok(Statement::FunctionCall {
                name: function_name,
                args,
            });
        }

        Err("Invalid function call".into())
    }

    fn parse_expression(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Expression, Box<dyn std::error::Error>> {
        match node.kind() {
            "expression" => {
                // Expression is a wrapper, evaluate its child
                let mut cursor = node.walk();
                if let Some(child) = node.children(&mut cursor).next() {
                    self.parse_expression(child, source)
                } else {
                    Err("Empty expression".into())
                }
            }
            "point" => self.parse_point(node, source),
            "path" => self.parse_path(node, source),
            "identifier" => {
                let name = node_text(node, source);
                Ok(Expression::Variable(name))
            }
            _ => Err(format!("Unknown expression type: {}", node.kind()).into()),
        }
    }

    fn parse_point(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Expression, Box<dyn std::error::Error>> {
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

        Ok(Expression::Point { x, y })
    }

    fn parse_path(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Expression, Box<dyn std::error::Error>> {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        let mut segments = Vec::new();
        let mut i = 0;

        // First pass: collect all points and connectors separately
        let mut points = Vec::new();
        let mut connectors = Vec::new();

        while i < children.len() {
            let child = children[i];

            match child.kind() {
                "identifier" => {
                    let var_name = node_text(child, source);
                    points.push(PointRef::Variable(var_name));
                }
                "point" => {
                    let point_ref = self.parse_point_literal(child, source)?;
                    points.push(point_ref);
                }
                "connector" => {
                    let connector = self.parse_connector_simple(child, source)?;
                    connectors.push(connector);
                }
                _ => {}
            }
            i += 1;
        }

        // Second pass: build segments by combining points and connectors
        if !points.is_empty() {
            // First point has no connector
            segments.push(PathSegment::Point(points[0].clone()));

            // Subsequent points are connected by connectors
            for i in 1..points.len() {
                let connector = connectors.get(i - 1).cloned().unwrap_or(Connector::Line);
                segments.push(PathSegment::Connection {
                    connector,
                    to: points[i].clone(),
                });
            }
        }

        Ok(Expression::Path(PathExpression { segments }))
    }

    fn parse_point_literal(
        &self,
        node: Node,
        source: &str,
    ) -> Result<PointRef, Box<dyn std::error::Error>> {
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

        Ok(PointRef::Literal { x, y })
    }

    fn parse_connector(
        &self,
        connector_node: Node,
        children: &[Node],
        current_index: usize,
        source: &str,
    ) -> Result<(Connector, usize), Box<dyn std::error::Error>> {
        let connector = self.parse_connector_simple(connector_node, source)?;
        Ok((connector, 0))
    }

    fn parse_connector_simple(
        &self,
        connector_node: Node,
        source: &str,
    ) -> Result<Connector, Box<dyn std::error::Error>> {
        let mut conn_cursor = connector_node.walk();

        for conn_child in connector_node.children(&mut conn_cursor) {
            match conn_child.kind() {
                "line_connector" => {
                    return Ok(Connector::Line);
                }
                "curve_connector" => {
                    let (start_options, end_options) =
                        self.parse_curve_options(conn_child, source)?;
                    return Ok(Connector::Curve {
                        start_options,
                        end_options,
                    });
                }
                _ => {}
            }
        }

        Ok(Connector::Line) // Default to line if no specific connector found
    }

    fn parse_curve_options(
        &self,
        curve_connector_node: Node,
        source: &str,
    ) -> Result<(CurveOptions, CurveOptions), Box<dyn std::error::Error>> {
        let mut start_options = CurveOptions::default();
        let mut end_options = CurveOptions::default();

        let mut curve_cursor = curve_connector_node.walk();
        for curve_child in curve_connector_node.children(&mut curve_cursor) {
            match curve_child.kind() {
                "curve_end_options" => {
                    end_options = self.parse_curve_option_set(curve_child, source)?;
                }
                "curve_start_options" => {
                    start_options = self.parse_curve_option_set(curve_child, source)?;
                }
                _ => {}
            }
        }

        Ok((start_options, end_options))
    }

    fn parse_curve_option_set(
        &self,
        options_node: Node,
        source: &str,
    ) -> Result<CurveOptions, Box<dyn std::error::Error>> {
        let mut options = CurveOptions::default();

        let mut opt_cursor = options_node.walk();
        for option in options_node.children(&mut opt_cursor) {
            if option.kind() == "option" {
                let (name, value) = self.parse_option(option, source)?;
                match name.as_str() {
                    "dir" => options.direction = Some(value.parse::<f64>()?),
                    "tension" => options.tension = Some(value.parse::<f64>()?),
                    _ => {} // ignore unknown options
                }
            }
        }

        Ok(options)
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
}

fn node_text(node: Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}
