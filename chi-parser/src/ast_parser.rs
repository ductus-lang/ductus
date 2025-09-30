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
            "binary_expression" => self.parse_binary_expression(node, source),
            "unary_expression" => self.parse_unary_expression(node, source),
            "point" => self.parse_point(node, source),
            "path" => self.parse_path(node, source),
            "number" => {
                let value = node_text(node, source).parse::<f64>()?;
                Ok(Expression::Number(value))
            }
            "identifier" => {
                let name = node_text(node, source);
                Ok(Expression::Variable(name))
            }
            "parenthesized_expression" => {
                // Handle (expression) - parse the inner expression
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "expression" {
                        return self.parse_expression(child, source);
                    }
                }
                Err("Invalid parenthesized expression".into())
            }
            _ => Err(format!("Unknown expression type: {}", node.kind()).into()),
        }
    }

    fn parse_binary_expression(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Expression, Box<dyn std::error::Error>> {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        if children.len() < 2 {
            return Err("Invalid binary expression".into());
        }

        // Parse left operand
        let left = Box::new(self.parse_expression(children[0], source)?);

        // Check if there's an explicit operator
        let (operator, right_index) = if children.len() == 3 {
            // Format: left operator right
            let op = self.parse_operator(children[1], source)?;
            (Some(op), 2)
        } else if children.len() == 2 {
            // Format: left right (implied multiplication)
            (None, 1)
        } else {
            return Err("Invalid binary expression format".into());
        };

        // Parse right operand
        let right = Box::new(self.parse_expression(children[right_index], source)?);

        Ok(Expression::BinaryExpression {
            left,
            operator,
            right,
        })
    }

    fn parse_unary_expression(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Expression, Box<dyn std::error::Error>> {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        if children.len() != 2 {
            return Err("Invalid unary expression".into());
        }

        let operator = self.parse_unary_operator(children[0], source)?;
        let operand = Box::new(self.parse_expression(children[1], source)?);

        Ok(Expression::UnaryExpression { operator, operand })
    }

    fn parse_operator(
        &self,
        node: Node,
        source: &str,
    ) -> Result<BinaryOperator, Box<dyn std::error::Error>> {
        let op_text = node_text(node, source);
        match op_text.as_str() {
            "+" => Ok(BinaryOperator::Add),
            "-" => Ok(BinaryOperator::Subtract),
            "*" => Ok(BinaryOperator::Multiply),
            "/" => Ok(BinaryOperator::Divide),
            "^" => Ok(BinaryOperator::Power),
            _ => Err(format!("Unknown binary operator: {}", op_text).into()),
        }
    }

    fn parse_unary_operator(
        &self,
        node: Node,
        source: &str,
    ) -> Result<UnaryOperator, Box<dyn std::error::Error>> {
        let op_text = node_text(node, source);
        match op_text.as_str() {
            "+" => Ok(UnaryOperator::Plus),
            "-" => Ok(UnaryOperator::Minus),
            _ => Err(format!("Unknown unary operator: {}", op_text).into()),
        }
    }

    fn parse_point(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Expression, Box<dyn std::error::Error>> {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();

        let mut x_expr = Expression::Number(0.0);
        let mut y_expr = Expression::Number(0.0);

        for child in children {
            match child.kind() {
                "x" => {
                    // x coordinate can now be an expression
                    let mut x_cursor = child.walk();
                    if let Some(x_child) = child.children(&mut x_cursor).next() {
                        x_expr = self.parse_expression(x_child, source)?;
                    }
                }
                "y" => {
                    // y coordinate can now be an expression
                    let mut y_cursor = child.walk();
                    if let Some(y_child) = child.children(&mut y_cursor).next() {
                        y_expr = self.parse_expression(y_child, source)?;
                    }
                }
                _ => {}
            }
        }

        Ok(Expression::Point {
            x: Box::new(x_expr),
            y: Box::new(y_expr),
        })
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
                    return Ok(Connector::Curve);
                }
                _ => {}
            }
        }

        Ok(Connector::Line) // Default to line if no specific connector found
    }
}

fn node_text(node: Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}
