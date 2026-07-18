//! Lower the tree-sitter CST into the typed AST.

use crate::ast::*;
use tree_sitter::{Node, Tree};

#[derive(Debug, Clone, PartialEq)]
pub struct LowerError {
    pub span: Span,
    pub message: String,
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "error at line {}, column {}: {}",
            self.span.line, self.span.column, self.message
        )
    }
}

impl std::error::Error for LowerError {}

pub fn lower(tree: &Tree, source: &str) -> Result<Program, LowerError> {
    let lowerer = Lowerer { source };
    lowerer.program(tree.root_node())
}

struct Lowerer<'a> {
    source: &'a str,
}

fn span(node: Node) -> Span {
    let position = node.start_position();
    Span {
        line: position.row + 1,
        column: position.column + 1,
    }
}

impl<'a> Lowerer<'a> {
    fn text(&self, node: Node) -> &'a str {
        node.utf8_text(self.source.as_bytes())
            .expect("source is valid UTF-8")
    }

    fn error(&self, node: Node, message: impl Into<String>) -> LowerError {
        LowerError {
            span: span(node),
            message: message.into(),
        }
    }

    /// Named children, with comments (extras) filtered out.
    fn children<'t>(&self, node: Node<'t>) -> Vec<Node<'t>> {
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .filter(|child| child.kind() != "comment")
            .collect()
    }

    fn field<'t>(&self, node: Node<'t>, name: &str) -> Result<Node<'t>, LowerError> {
        node.child_by_field_name(name)
            .ok_or_else(|| self.error(node, format!("missing '{name}' in {}", node.kind())))
    }

    fn program(&self, root: Node) -> Result<Program, LowerError> {
        let mut items = Vec::new();
        for child in self.children(root) {
            items.push(self.item(child)?);
        }
        Ok(Program { items })
    }

    fn item(&self, node: Node) -> Result<Item, LowerError> {
        match node.kind() {
            "params_block" => {
                let mut params = Vec::new();
                for child in self.children(node) {
                    params.push(self.param_declaration(child)?);
                }
                Ok(Item::Params(params))
            }
            "metrics_block" => {
                let mut entries = Vec::new();
                for child in self.children(node) {
                    entries.push(self.binding(child)?);
                }
                Ok(Item::Metrics(entries))
            }
            "def_declaration" => Ok(Item::Def(self.def_declaration(node)?)),
            "glyph_block" => Ok(Item::Glyph(self.glyph_block(node)?)),
            "import_statement" => {
                let path = self.string_value(self.field(node, "path")?);
                Ok(Item::Import {
                    path,
                    span: span(node),
                })
            }
            _ => Ok(Item::Stmt(self.statement(node)?)),
        }
    }

    fn param_declaration(&self, node: Node) -> Result<ParamDecl, LowerError> {
        let range = self.field(node, "range")?;
        Ok(ParamDecl {
            name: self.text(self.field(node, "name")?).to_string(),
            default: self.expression(self.field(node, "default")?)?,
            min: self.expression(self.field(range, "min")?)?,
            max: self.expression(self.field(range, "max")?)?,
            span: span(node),
        })
    }

    fn binding(&self, node: Node) -> Result<Binding, LowerError> {
        Ok(Binding {
            name: self.text(self.field(node, "name")?).to_string(),
            value: self.rhs(self.field(node, "value")?)?,
            span: span(node),
        })
    }

    fn def_declaration(&self, node: Node) -> Result<DefDecl, LowerError> {
        let parameters = self
            .children(self.field(node, "parameters")?)
            .into_iter()
            .map(|child| self.text(child).to_string())
            .collect();
        let returns = self
            .children(self.field(node, "returns")?)
            .into_iter()
            .map(|child| self.text(child).to_string())
            .collect();
        let body = self.block(self.field(node, "body")?)?;
        Ok(DefDecl {
            name: self.text(self.field(node, "name")?).to_string(),
            parameters,
            returns,
            body,
            span: span(node),
        })
    }

    fn block(&self, node: Node) -> Result<Vec<Stmt>, LowerError> {
        let mut statements = Vec::new();
        for child in self.children(node) {
            statements.push(self.statement(child)?);
        }
        Ok(statements)
    }

    fn glyph_block(&self, node: Node) -> Result<GlyphDecl, LowerError> {
        let name_node = self.field(node, "name")?;
        let name = match name_node.kind() {
            "string" => self.string_value(name_node),
            _ => self.text(name_node).to_string(),
        };
        let unicode = match node.child_by_field_name("unicode") {
            Some(unicode_node) => {
                let text = self.text(unicode_node);
                let hex = text.trim_start_matches("U+");
                Some(u32::from_str_radix(hex, 16).map_err(|_| {
                    self.error(unicode_node, format!("invalid codepoint {text}"))
                })?)
            }
            None => None,
        };

        let mut body = Vec::new();
        for child in self.children(node) {
            // The name/unicode fields are also named children; skip them.
            if Some(child) == node.child_by_field_name("name")
                || Some(child) == node.child_by_field_name("unicode")
            {
                continue;
            }
            body.push(self.glyph_statement(child)?);
        }
        Ok(GlyphDecl {
            name,
            unicode,
            body,
            span: span(node),
        })
    }

    fn glyph_statement(&self, node: Node) -> Result<GlyphStmt, LowerError> {
        match node.kind() {
            "contour_declaration" => Ok(GlyphStmt::Contour(self.binding(node)?)),
            "guide_declaration" => Ok(GlyphStmt::Guide(self.binding(node)?)),
            "anchor_declaration" => Ok(GlyphStmt::Anchor {
                name: self.text(self.field(node, "name")?).to_string(),
                value: self.expression(self.field(node, "value")?)?,
                span: span(node),
            }),
            _ => Ok(GlyphStmt::Stmt(self.statement(node)?)),
        }
    }

    fn statement(&self, node: Node) -> Result<Stmt, LowerError> {
        match node.kind() {
            "assignment" => Ok(Stmt::Assign(self.binding(node)?)),
            "destructuring_assignment" => {
                let names = self
                    .children(self.field(node, "pattern")?)
                    .into_iter()
                    .map(|child| self.text(child).to_string())
                    .collect();
                Ok(Stmt::Destructure {
                    names,
                    value: self.expression(self.field(node, "value")?)?,
                    span: span(node),
                })
            }
            "node_declaration" => {
                let clause_expr = |clause: Node| -> Result<Expr, LowerError> {
                    let children = self.children(clause);
                    match children.first() {
                        Some(child) => self.expression(*child),
                        None => Err(self.error(clause, "empty clause")),
                    }
                };
                let nib = match node.child_by_field_name("nib") {
                    Some(clause) => Some(clause_expr(clause)?),
                    None => None,
                };
                Ok(Stmt::Node(NodeDecl {
                    name: self.text(self.field(node, "name")?).to_string(),
                    position: self.expression(self.field(node, "position")?)?,
                    dir: clause_expr(self.field(node, "dir")?)?,
                    width: clause_expr(self.field(node, "width")?)?,
                    nib,
                    span: span(node),
                }))
            }
            "call_statement" => {
                let call = self
                    .children(node)
                    .into_iter()
                    .next()
                    .ok_or_else(|| self.error(node, "empty call statement"))?;
                let (function, arguments) = self.call_parts(call)?;
                Ok(Stmt::Call {
                    function,
                    arguments,
                    span: span(node),
                })
            }
            "for_statement" => {
                let iterable_node = self.field(node, "iterable")?;
                let iterable = match iterable_node.kind() {
                    "range" => Iterable::Range {
                        min: self.expression(self.field(iterable_node, "min")?)?,
                        max: self.expression(self.field(iterable_node, "max")?)?,
                    },
                    "list" => Iterable::List(self.expression_list(iterable_node)?),
                    _ => Iterable::Expr(self.expression(iterable_node)?),
                };
                Ok(Stmt::For {
                    name: self.text(self.field(node, "name")?).to_string(),
                    iterable,
                    body: self.block(self.field(node, "body")?)?,
                    span: span(node),
                })
            }
            other => Err(self.error(node, format!("unexpected statement: {other}"))),
        }
    }

    fn call_parts(&self, node: Node) -> Result<(String, Vec<Rhs>), LowerError> {
        let function = self.text(self.field(node, "function")?).to_string();
        let arguments = match node.child_by_field_name("arguments") {
            Some(arguments_node) => self
                .children(arguments_node)
                .into_iter()
                .map(|child| self.rhs(child))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        Ok((function, arguments))
    }

    /// A binding value or call argument: expression, path, or list.
    fn rhs(&self, node: Node) -> Result<Rhs, LowerError> {
        match node.kind() {
            "path" => Ok(Rhs::Path(self.path(node)?)),
            "list" => Ok(Rhs::List(self.expression_list(node)?)),
            _ => Ok(Rhs::Expr(self.expression(node)?)),
        }
    }

    fn expression_list(&self, node: Node) -> Result<Vec<Expr>, LowerError> {
        self.children(node)
            .into_iter()
            .map(|child| self.expression(child))
            .collect()
    }

    fn path(&self, node: Node) -> Result<PathExpr, LowerError> {
        let mut segments = Vec::new();
        let mut joints = Vec::new();
        let mut closed = false;
        let children = self.children(node);
        let child_count = children.len();
        for (index, child) in children.into_iter().enumerate() {
            match child.kind() {
                "path_segment" => {
                    if closed {
                        return Err(self.error(child, "cycle must be the last element of a path"));
                    }
                    segments.push(self.path_segment(child)?);
                }
                "line_joint" => joints.push(Joint::Line),
                "curve_joint" => joints.push(Joint::Curve),
                "cycle" => {
                    if index != child_count - 1 {
                        return Err(self.error(child, "cycle must be the last element of a path"));
                    }
                    closed = true;
                }
                other => return Err(self.error(child, format!("unexpected path element: {other}"))),
            }
        }
        Ok(PathExpr {
            segments,
            joints,
            closed,
        })
    }

    fn path_segment(&self, node: Node) -> Result<PathSegment, LowerError> {
        let enter = match node.child_by_field_name("enter") {
            Some(spec) => Some(self.direction_spec(spec)?),
            None => None,
        };
        let leave = match node.child_by_field_name("leave") {
            Some(spec) => Some(self.direction_spec(spec)?),
            None => None,
        };
        Ok(PathSegment {
            enter,
            point: self.expression(self.field(node, "point")?)?,
            leave,
        })
    }

    fn direction_spec(&self, node: Node) -> Result<DirSpec, LowerError> {
        let inner = self
            .children(node)
            .into_iter()
            .next()
            .ok_or_else(|| self.error(node, "empty direction spec"))?;
        match inner.kind() {
            "corner" => Ok(DirSpec::Corner),
            _ => Ok(DirSpec::Angle(self.expression(inner)?)),
        }
    }

    fn expression(&self, node: Node) -> Result<Expr, LowerError> {
        let expr_span = span(node);
        let kind = match node.kind() {
            "number" => ExprKind::Number(self.text(node).parse().map_err(|_| {
                self.error(node, format!("invalid number: {}", self.text(node)))
            })?),
            "identifier" => ExprKind::Ident(self.text(node).to_string()),
            "point" => ExprKind::Point {
                x: Box::new(self.expression(self.field(node, "x")?)?),
                y: Box::new(self.expression(self.field(node, "y")?)?),
            },
            "parenthesized_expression" => {
                let inner = self
                    .children(node)
                    .into_iter()
                    .next()
                    .ok_or_else(|| self.error(node, "empty parentheses"))?;
                return self.expression(inner);
            }
            "unary_expression" => {
                let operator = self.text(self.field(node, "operator")?);
                let op = match operator {
                    "-" => UnaryOp::Neg,
                    "not" => UnaryOp::Not,
                    other => return Err(self.error(node, format!("unknown operator: {other}"))),
                };
                ExprKind::Unary {
                    op,
                    operand: Box::new(self.expression(self.field(node, "operand")?)?),
                }
            }
            "binary_expression" => {
                let operator = self.text(self.field(node, "operator")?);
                let op = match operator {
                    "+" => BinaryOp::Add,
                    "-" => BinaryOp::Sub,
                    "*" => BinaryOp::Mul,
                    "/" => BinaryOp::Div,
                    "^" => BinaryOp::Pow,
                    "<" => BinaryOp::Lt,
                    "<=" => BinaryOp::Le,
                    ">" => BinaryOp::Gt,
                    ">=" => BinaryOp::Ge,
                    "==" => BinaryOp::Eq,
                    "!=" => BinaryOp::Ne,
                    "and" => BinaryOp::And,
                    "or" => BinaryOp::Or,
                    other => return Err(self.error(node, format!("unknown operator: {other}"))),
                };
                ExprKind::Binary {
                    op,
                    left: Box::new(self.expression(self.field(node, "left")?)?),
                    right: Box::new(self.expression(self.field(node, "right")?)?),
                }
            }
            "implicit_multiplication" => ExprKind::Binary {
                op: BinaryOp::Mul,
                left: Box::new(self.expression(self.field(node, "left")?)?),
                right: Box::new(self.expression(self.field(node, "right")?)?),
            },
            "mediation" => ExprKind::Mediation {
                t: Box::new(self.expression(self.field(node, "t")?)?),
                a: Box::new(self.expression(self.field(node, "a")?)?),
                b: Box::new(self.expression(self.field(node, "b")?)?),
            },
            "member_expression" => ExprKind::Member {
                object: Box::new(self.expression(self.field(node, "object")?)?),
                property: self.text(self.field(node, "property")?).to_string(),
            },
            "transform_expression" => {
                let operator = self.text(self.field(node, "transform")?);
                let op = match operator {
                    "shifted" => TransformOp::Shifted,
                    "scaled" => TransformOp::Scaled,
                    "xscaled" => TransformOp::XScaled,
                    "yscaled" => TransformOp::YScaled,
                    "rotated" => TransformOp::Rotated,
                    "slanted" => TransformOp::Slanted,
                    "reversed" => TransformOp::Reversed,
                    other => return Err(self.error(node, format!("unknown transform: {other}"))),
                };
                let argument = match node.child_by_field_name("argument") {
                    Some(argument_node) => Some(Box::new(self.expression(argument_node)?)),
                    None => None,
                };
                ExprKind::Transform {
                    operand: Box::new(self.expression(self.field(node, "operand")?)?),
                    op,
                    argument,
                }
            }
            "call_expression" => {
                let (function, arguments) = self.call_parts(node)?;
                ExprKind::Call {
                    function,
                    arguments,
                }
            }
            "conditional_expression" => ExprKind::Conditional {
                value: Box::new(self.expression(self.field(node, "value")?)?),
                condition: Box::new(self.expression(self.field(node, "condition")?)?),
                alternative: Box::new(self.expression(self.field(node, "alternative")?)?),
            },
            other => return Err(self.error(node, format!("unexpected expression: {other}"))),
        };
        Ok(Expr {
            kind,
            span: expr_span,
        })
    }

    fn string_value(&self, node: Node) -> String {
        self.text(node).trim_matches('"').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn lower_source(source: &str) -> Program {
        let tree = parse(source).expect("source should parse");
        lower(&tree, source).expect("source should lower")
    }

    fn only_stmt(program: &Program) -> &Stmt {
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Item::Stmt(stmt) => stmt,
            other => panic!("expected statement, got {other:?}"),
        }
    }

    #[test]
    fn lowers_arithmetic_with_implicit_multiplication() {
        let program = lower_source("v = 3 stem + 380;\n");
        let Stmt::Assign(binding) = only_stmt(&program) else {
            panic!("expected assignment")
        };
        let Rhs::Expr(expr) = &binding.value else {
            panic!("expected expression")
        };
        // (3 * stem) + 380
        let ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            ..
        } = &expr.kind
        else {
            panic!("expected addition at the top, got {expr:?}")
        };
        assert!(matches!(
            left.kind,
            ExprKind::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
    }

    #[test]
    fn distinguishes_point_from_parenthesized_expression() {
        let program = lower_source("p = (1, 2);\nq = (1 + 2);\n");
        let Item::Stmt(Stmt::Assign(p)) = &program.items[0] else {
            panic!()
        };
        let Item::Stmt(Stmt::Assign(q)) = &program.items[1] else {
            panic!()
        };
        assert!(matches!(
            &p.value,
            Rhs::Expr(Expr {
                kind: ExprKind::Point { .. },
                ..
            })
        ));
        assert!(matches!(
            &q.value,
            Rhs::Expr(Expr {
                kind: ExprKind::Binary { .. },
                ..
            })
        ));
    }

    #[test]
    fn lowers_path_with_direction_specs_and_cycle() {
        let program = lower_source("p = a {up} .. b -- c {corner} .. cycle;\n");
        let Stmt::Assign(binding) = only_stmt(&program) else {
            panic!()
        };
        let Rhs::Path(path) = &binding.value else {
            panic!("expected path, got {:?}", binding.value)
        };
        assert_eq!(path.segments.len(), 3);
        assert_eq!(path.joints, vec![Joint::Curve, Joint::Line, Joint::Curve]);
        assert!(path.closed);
        assert!(matches!(path.segments[0].leave, Some(DirSpec::Angle(_))));
        assert!(path.segments[0].enter.is_none());
        assert!(matches!(path.segments[2].leave, Some(DirSpec::Corner)));
    }

    #[test]
    fn lowers_node_declaration_with_nib() {
        let program = lower_source("node k = (0, 1) dir theta width w nib phi;\n");
        let Stmt::Node(node) = only_stmt(&program) else {
            panic!()
        };
        assert_eq!(node.name, "k");
        assert!(matches!(node.position.kind, ExprKind::Point { .. }));
        assert!(matches!(node.dir.kind, ExprKind::Ident(_)));
        assert!(node.nib.is_some());
    }

    #[test]
    fn lowers_glyph_with_unicode_and_declarations() {
        let source = "\
glyph o : U+006F {
  advance = 100;
  contour outer = a .. b .. cycle;
  guide spine = a .. b;
  anchor top = (50, 500);
}
";
        let program = lower_source(source);
        let Item::Glyph(glyph) = &program.items[0] else {
            panic!()
        };
        assert_eq!(glyph.name, "o");
        assert_eq!(glyph.unicode, Some(0x6F));
        assert_eq!(glyph.body.len(), 4);
        assert!(matches!(glyph.body[0], GlyphStmt::Stmt(Stmt::Assign(_))));
        assert!(matches!(glyph.body[1], GlyphStmt::Contour(_)));
        assert!(matches!(glyph.body[2], GlyphStmt::Guide(_)));
        assert!(matches!(glyph.body[3], GlyphStmt::Anchor { .. }));
    }

    #[test]
    fn lowers_glyph_with_string_name() {
        let program = lower_source("glyph \"ka.mlym\" { advance = 500; }\n");
        let Item::Glyph(glyph) = &program.items[0] else {
            panic!()
        };
        assert_eq!(glyph.name, "ka.mlym");
        assert_eq!(glyph.unicode, None);
    }

    #[test]
    fn lowers_params_and_metrics() {
        let source = "\
params { weight = 400 in 100..900; }
metrics { upm = 1000; x = 500; }
";
        let program = lower_source(source);
        let Item::Params(params) = &program.items[0] else {
            panic!()
        };
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "weight");
        let Item::Metrics(metrics) = &program.items[1] else {
            panic!()
        };
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].name, "upm");
    }

    #[test]
    fn lowers_def_with_tuple_return_and_destructuring() {
        let source = "\
def bowl(c, r) -> (n, s) {
  node n = c + (0, r) dir left width hair;
  node s = c + (0, -r) dir right width hair;
}
(n, s) = bowl((100, 100), 50);
";
        let program = lower_source(source);
        let Item::Def(def) = &program.items[0] else {
            panic!()
        };
        assert_eq!(def.parameters, vec!["c", "r"]);
        assert_eq!(def.returns, vec!["n", "s"]);
        assert_eq!(def.body.len(), 2);
        let Item::Stmt(Stmt::Destructure { names, .. }) = &program.items[1] else {
            panic!()
        };
        assert_eq!(names, &["n", "s"]);
    }

    #[test]
    fn lowers_call_statement_with_path_argument() {
        let program = lower_source("draw(a .. b);\n");
        let Stmt::Call {
            function,
            arguments,
            ..
        } = only_stmt(&program)
        else {
            panic!()
        };
        assert_eq!(function, "draw");
        assert_eq!(arguments.len(), 1);
        assert!(matches!(arguments[0], Rhs::Path(_)));
    }

    #[test]
    fn lowers_for_and_import() {
        let source = "\
import \"measures.chi\";
for i in 0..3 { draw(p); }
";
        let program = lower_source(source);
        assert!(matches!(&program.items[0], Item::Import { path, .. } if path == "measures.chi"));
        let Item::Stmt(Stmt::For { iterable, body, .. }) = &program.items[1] else {
            panic!()
        };
        assert!(matches!(iterable, Iterable::Range { .. }));
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn comments_are_skipped_everywhere() {
        let source = "\
# leading comment
u = 100; # trailing
p = a # inside a path
  .. b;
";
        let program = lower_source(source);
        assert_eq!(program.items.len(), 2);
    }
}
