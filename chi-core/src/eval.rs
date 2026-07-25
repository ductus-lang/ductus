//! Evaluate the AST: single pass, lexically scoped, immutable bindings.

use crate::ast::*;
use crate::doc::{Document, Glyph, Param, SketchCommand};
use crate::value::{NodeValue, PathPoint, PathValue, Value, unitvec};
use kurbo::{Affine, Point, Vec2};
use std::collections::{HashMap, HashSet};

const BUILTIN_FUNCTIONS: &[&str] = &[
    "min",
    "max",
    "abs",
    "sqrt",
    "floor",
    "ceil",
    "round",
    "sin",
    "cos",
    "tan",
    "angle",
    "unitvec",
    "length",
    "distance",
    "intersection",
    "draw",
    "fill",
];

#[derive(Debug, Clone, PartialEq)]
pub struct EvalError {
    pub span: Span,
    pub message: String,
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "error at line {}, column {}: {}",
            self.span.line, self.span.column, self.message
        )
    }
}

impl std::error::Error for EvalError {}

fn err(span: Span, message: impl Into<String>) -> EvalError {
    EvalError {
        span,
        message: message.into(),
    }
}

/// Evaluate a whole program into a renderable document.
pub fn evaluate(
    program: &Program,
    overrides: &HashMap<String, f64>,
) -> Result<Document, EvalError> {
    let mut evaluator = Evaluator::new();
    let mut document = Document::default();
    let mut known_params = HashSet::new();

    for item in &program.items {
        match item {
            Item::Params(params) => {
                for param in params {
                    known_params.insert(param.name.clone());
                    let resolved = evaluator.eval_param(param, overrides)?;
                    document.params.push(resolved);
                }
            }
            Item::Metrics(entries) => {
                for entry in entries {
                    let value = evaluator.eval_rhs(&entry.value)?;
                    let Value::Number(number) = value else {
                        return Err(err(
                            entry.span,
                            format!("metrics must be numbers, got {}", value.type_name()),
                        ));
                    };
                    evaluator.define(&entry.name, Value::Number(number), entry.span)?;
                    document.metrics.values.insert(entry.name.clone(), number);
                }
            }
            Item::Pen(pen) => {
                return Err(err(pen.span, "'pen' is not implemented yet"));
            }
            Item::Def(def) => evaluator.register_def(def)?,
            Item::Glyph(glyph) => {
                if document.glyphs.iter().any(|g| g.name == glyph.name) {
                    return Err(err(
                        glyph.span,
                        format!("glyph '{}' is already defined", glyph.name),
                    ));
                }
                let evaluated = evaluator.eval_glyph(glyph)?;
                document.glyphs.push(evaluated);
            }
            Item::Import { span, .. } => {
                return Err(err(*span, "'import' is not implemented yet (phase 2)"));
            }
            Item::Stmt(stmt) => evaluator.exec_statement(stmt)?,
        }
    }

    for name in overrides.keys() {
        if !known_params.contains(name) {
            return Err(err(
                Span { line: 1, column: 1 },
                format!("--set {name}: no such param in this file"),
            ));
        }
    }

    document.sketch = evaluator.sketch;
    Ok(document)
}

pub struct Evaluator {
    /// Innermost scope last. Scope 0 is the prelude.
    scopes: Vec<HashMap<String, Value>>,
    defs: HashMap<String, DefDecl>,
    /// Names of defs currently executing, to reject recursion.
    call_stack: Vec<String>,
    /// True while evaluating a glyph body: draw/fill are sketch-only.
    in_glyph: bool,
    sketch: Vec<SketchCommand>,
}

impl Evaluator {
    pub fn new() -> Self {
        let mut prelude = HashMap::new();
        prelude.insert("right".to_string(), Value::Number(0.0));
        prelude.insert("up".to_string(), Value::Number(90.0));
        prelude.insert("left".to_string(), Value::Number(180.0));
        prelude.insert("down".to_string(), Value::Number(270.0));
        Self {
            scopes: vec![prelude, HashMap::new()],
            defs: HashMap::new(),
            call_stack: Vec::new(),
            in_glyph: false,
            sketch: Vec::new(),
        }
    }

    fn eval_param(
        &mut self,
        param: &ParamDecl,
        overrides: &HashMap<String, f64>,
    ) -> Result<Param, EvalError> {
        let default = self.expect_number(&param.default)?;
        let min = self.expect_number(&param.min)?;
        let max = self.expect_number(&param.max)?;
        let value = overrides.get(&param.name).copied().unwrap_or(default);
        if value < min || value > max {
            return Err(err(
                param.span,
                format!("{} = {} is outside its range {}..{}", param.name, value, min, max),
            ));
        }
        self.define(&param.name, Value::Number(value), param.span)?;
        Ok(Param {
            name: param.name.clone(),
            value,
            default,
            min,
            max,
        })
    }

    fn register_def(&mut self, def: &DefDecl) -> Result<(), EvalError> {
        if BUILTIN_FUNCTIONS.contains(&def.name.as_str()) {
            return Err(err(
                def.span,
                format!("'{}' is a built-in function and cannot be redefined", def.name),
            ));
        }
        if self.defs.contains_key(&def.name) {
            return Err(err(def.span, format!("def '{}' is already defined", def.name)));
        }
        self.defs.insert(def.name.clone(), def.clone());
        Ok(())
    }

    fn eval_glyph(&mut self, glyph: &GlyphDecl) -> Result<Glyph, EvalError> {
        self.push_scope();
        self.in_glyph = true;
        let result = self.eval_glyph_body(glyph);
        self.in_glyph = false;
        self.pop_scope();
        result
    }

    fn eval_glyph_body(&mut self, glyph: &GlyphDecl) -> Result<Glyph, EvalError> {
        let mut contours = Vec::new();
        let mut guides = Vec::new();
        let mut anchors = Vec::new();

        for statement in &glyph.body {
            match statement {
                GlyphStmt::Stmt(stmt) => self.exec_statement(stmt)?,
                GlyphStmt::Contour(binding) => {
                    let value = self.eval_rhs(&binding.value)?;
                    let Value::Path(path) = value else {
                        return Err(err(
                            binding.span,
                            format!("contour must be a path, got {}", value.type_name()),
                        ));
                    };
                    if !path.closed {
                        return Err(err(
                            binding.span,
                            format!(
                                "contour '{}' is not closed; end it with '.. cycle' or '-- cycle'",
                                binding.name
                            ),
                        ));
                    }
                    contours.push((binding.name.clone(), path));
                }
                GlyphStmt::Guide(binding) => {
                    let value = self.eval_rhs(&binding.value)?;
                    let Value::Path(path) = value else {
                        return Err(err(
                            binding.span,
                            format!("guide must be a path, got {}", value.type_name()),
                        ));
                    };
                    guides.push((binding.name.clone(), path));
                }
                GlyphStmt::Anchor { name, value, span } => {
                    let point = match self.eval_expr(value)? {
                        Value::Point(point) => point,
                        other => {
                            return Err(err(
                                *span,
                                format!("anchor must be a point, got {}", other.type_name()),
                            ));
                        }
                    };
                    anchors.push((name.clone(), point));
                }
            }
        }

        let advance = match self.scopes.last().and_then(|scope| scope.get("advance")) {
            Some(Value::Number(advance)) => *advance,
            Some(other) => {
                return Err(err(
                    glyph.span,
                    format!("advance must be a number, got {}", other.type_name()),
                ));
            }
            None => {
                return Err(err(
                    glyph.span,
                    format!("glyph '{}' does not set advance", glyph.name),
                ));
            }
        };

        // Collect skeleton nodes bound in the glyph scope, for debug preview.
        let mut nodes: Vec<(String, NodeValue)> = self
            .scopes
            .last()
            .expect("glyph scope")
            .iter()
            .filter_map(|(name, value)| match value {
                Value::Node(node) => Some((name.clone(), *node)),
                _ => None,
            })
            .collect();
        nodes.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(Glyph {
            name: glyph.name.clone(),
            unicode: glyph.unicode,
            advance,
            contours,
            guides,
            anchors,
            nodes,
        })
    }

    /// Look a name up through all scopes, innermost first.
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn define(&mut self, name: &str, value: Value, span: Span) -> Result<(), EvalError> {
        let scope = self.scopes.last_mut().expect("at least one scope");
        if scope.contains_key(name) {
            return Err(err(
                span,
                format!("'{name}' is already defined in this scope; bindings are immutable"),
            ));
        }
        scope.insert(name.to_string(), value);
        Ok(())
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    // ---- Statements ------------------------------------------------------

    pub fn exec_statement(&mut self, statement: &Stmt) -> Result<(), EvalError> {
        match statement {
            Stmt::Assign(binding) => {
                let value = self.eval_rhs(&binding.value)?;
                self.define(&binding.name, value, binding.span)
            }
            Stmt::Node(node) => {
                let value = self.eval_node(node)?;
                self.define(&node.name, Value::Node(value), node.span)
            }
            Stmt::Destructure { names, value, span } => {
                let evaluated = self.eval_expr(value)?;
                let Value::Tuple(values) = evaluated else {
                    return Err(err(
                        *span,
                        format!(
                            "destructuring expects a multi-value call result, got {}",
                            evaluated.type_name()
                        ),
                    ));
                };
                if values.len() != names.len() {
                    return Err(err(
                        *span,
                        format!("expected {} values, got {}", names.len(), values.len()),
                    ));
                }
                for (name, value) in names.iter().zip(values) {
                    self.define(name, value, *span)?;
                }
                Ok(())
            }
            Stmt::Call {
                function,
                arguments,
                span,
            } => match function.as_str() {
                "draw" | "fill" => {
                    if self.in_glyph || !self.call_stack.is_empty() {
                        return Err(err(
                            *span,
                            format!(
                                "{function}() is for sketching at the top level; \
                                 use 'contour' or 'guide' inside a glyph"
                            ),
                        ));
                    }
                    for argument in arguments {
                        let value = self.eval_rhs(argument)?;
                        let Value::Path(path) = value else {
                            return Err(err(
                                *span,
                                format!(
                                    "{function}() expects paths, got {}",
                                    value.type_name()
                                ),
                            ));
                        };
                        if function == "fill" {
                            if !path.closed {
                                return Err(err(*span, "fill() needs a closed path"));
                            }
                            self.sketch.push(SketchCommand::Fill(path));
                        } else {
                            self.sketch.push(SketchCommand::Draw(path));
                        }
                    }
                    Ok(())
                }
                other => Err(err(
                    *span,
                    format!("'{other}' is not a statement; only draw() and fill() are"),
                )),
            },
            Stmt::For { span, .. } => {
                Err(err(*span, "'for' is not implemented yet (phase 2)"))
            }
        }
    }

    fn eval_node(&mut self, node: &NodeDecl) -> Result<NodeValue, EvalError> {
        let pos = self.expect_point(&node.position)?;
        let dir = self.expect_number(&node.dir)?;
        let Some(width_expr) = &node.width else {
            return Err(err(node.span, "'pen' is not implemented yet"));
        };
        let (left_offset, right_offset) = match self.eval_expr(width_expr)? {
            Value::Number(total) => (total / 2.0, total / 2.0),
            Value::Point(offsets) => (offsets.x, offsets.y),
            other => {
                return Err(err(
                    width_expr.span,
                    format!(
                        "width expects a number or an (left, right) pair, got {}",
                        other.type_name()
                    ),
                ));
            }
        };
        let nib = match &node.nib {
            Some(expr) => self.expect_number(expr)?,
            None => dir + 90.0,
        };
        Ok(NodeValue {
            pos,
            dir,
            left_offset,
            right_offset,
            nib,
        })
    }

    // ---- Values ------------------------------------------------------------

    pub fn eval_rhs(&mut self, rhs: &Rhs) -> Result<Value, EvalError> {
        match rhs {
            Rhs::Expr(expr) => self.eval_expr(expr),
            Rhs::Path(path) => Ok(Value::Path(self.eval_path(path)?)),
            Rhs::List(items) => {
                let values = items
                    .iter()
                    .map(|item| self.eval_expr(item))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::List(values))
            }
        }
    }

    fn eval_path(&mut self, path: &PathExpr) -> Result<PathValue, EvalError> {
        let mut points = Vec::with_capacity(path.segments.len());
        for segment in &path.segments {
            let pos = match self.eval_expr(&segment.point)? {
                Value::Point(point) => point,
                Value::Node(_) => {
                    return Err(err(
                        segment.point.span,
                        "a node is not a path point; use .pos, .left or .right",
                    ));
                }
                other => {
                    return Err(err(
                        segment.point.span,
                        format!("path points must be points, got {}", other.type_name()),
                    ));
                }
            };
            let mut corner = false;
            let mut resolve = |spec: &Option<DirSpec>,
                               evaluator: &mut Self|
             -> Result<Option<f64>, EvalError> {
                match spec {
                    None => Ok(None),
                    Some(DirSpec::Corner) => {
                        corner = true;
                        Ok(None)
                    }
                    Some(DirSpec::Angle(expr)) => Ok(Some(evaluator.expect_number(expr)?)),
                }
            };
            let enter = resolve(&segment.enter, self)?;
            let leave = resolve(&segment.leave, self)?;
            points.push(PathPoint {
                pos,
                enter,
                leave,
                corner,
            });
        }
        Ok(PathValue {
            points,
            joints: path.joints.clone(),
            closed: path.closed,
        })
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, EvalError> {
        match &expr.kind {
            ExprKind::Number(value) => Ok(Value::Number(*value)),
            ExprKind::Ident(name) => self
                .lookup(name)
                .cloned()
                .ok_or_else(|| err(expr.span, format!("undefined variable: {name}"))),
            ExprKind::Point { x, y } => Ok(Value::Point(Point::new(
                self.expect_number(x)?,
                self.expect_number(y)?,
            ))),
            ExprKind::Unary { op, operand } => {
                let value = self.eval_expr(operand)?;
                match (op, value) {
                    (UnaryOp::Neg, Value::Number(number)) => Ok(Value::Number(-number)),
                    (UnaryOp::Neg, Value::Point(point)) => {
                        Ok(Value::Point(Point::new(-point.x, -point.y)))
                    }
                    (UnaryOp::Not, Value::Bool(condition)) => Ok(Value::Bool(!condition)),
                    (_, other) => Err(err(
                        expr.span,
                        format!("invalid operand type: {}", other.type_name()),
                    )),
                }
            }
            ExprKind::Binary { op, left, right } => self.eval_binary(expr.span, *op, left, right),
            ExprKind::Mediation { t, a, b } => {
                let t = self.expect_number(t)?;
                let a = self.eval_expr(a)?;
                let b = self.eval_expr(b)?;
                match (a, b) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + t * (b - a))),
                    (Value::Point(a), Value::Point(b)) => {
                        Ok(Value::Point(a + t * (b - a)))
                    }
                    (a, b) => Err(err(
                        expr.span,
                        format!(
                            "mediation needs two numbers or two points, got {} and {}",
                            a.type_name(),
                            b.type_name()
                        ),
                    )),
                }
            }
            ExprKind::Member { object, property } => {
                let object = self.eval_expr(object)?;
                self.eval_member(expr.span, &object, property)
            }
            ExprKind::Transform {
                operand,
                op,
                argument,
            } => self.eval_transform(expr.span, operand, *op, argument.as_deref()),
            ExprKind::Call {
                function,
                arguments,
            } => self.eval_call(expr.span, function, arguments),
            ExprKind::Conditional {
                value,
                condition,
                alternative,
            } => {
                // Lazy: only the taken branch is evaluated.
                match self.eval_expr(condition)? {
                    Value::Bool(true) => self.eval_expr(value),
                    Value::Bool(false) => self.eval_expr(alternative),
                    other => Err(err(
                        condition.span,
                        format!("condition must be a boolean, got {}", other.type_name()),
                    )),
                }
            }
        }
    }

    fn eval_binary(
        &mut self,
        span: Span,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
    ) -> Result<Value, EvalError> {
        use BinaryOp::*;
        // Lazy boolean operators first.
        if matches!(op, And | Or) {
            let left_value = self.expect_bool(left)?;
            return match (op, left_value) {
                (And, false) => Ok(Value::Bool(false)),
                (Or, true) => Ok(Value::Bool(true)),
                _ => Ok(Value::Bool(self.expect_bool(right)?)),
            };
        }

        let left_value = self.eval_expr(left)?;
        let right_value = self.eval_expr(right)?;
        match (left_value, right_value) {
            (Value::Number(a), Value::Number(b)) => match op {
                Add => Ok(Value::Number(a + b)),
                Sub => Ok(Value::Number(a - b)),
                Mul => Ok(Value::Number(a * b)),
                Div => {
                    if b == 0.0 {
                        Err(err(span, "division by zero"))
                    } else {
                        Ok(Value::Number(a / b))
                    }
                }
                Pow => Ok(Value::Number(a.powf(b))),
                Lt => Ok(Value::Bool(a < b)),
                Le => Ok(Value::Bool(a <= b)),
                Gt => Ok(Value::Bool(a > b)),
                Ge => Ok(Value::Bool(a >= b)),
                Eq => Ok(Value::Bool(a == b)),
                Ne => Ok(Value::Bool(a != b)),
                And | Or => unreachable!("handled above"),
            },
            (Value::Point(a), Value::Point(b)) => match op {
                Add => Ok(Value::Point(a + b.to_vec2())),
                Sub => Ok(Value::Point((a - b).to_point())),
                _ => Err(err(span, "only + and - are defined between points")),
            },
            (Value::Number(k), Value::Point(p)) => match op {
                Mul => Ok(Value::Point((k * p.to_vec2()).to_point())),
                _ => Err(err(span, "only * is defined between a number and a point")),
            },
            (Value::Point(p), Value::Number(k)) => match op {
                Mul => Ok(Value::Point((k * p.to_vec2()).to_point())),
                Div => {
                    if k == 0.0 {
                        Err(err(span, "division by zero"))
                    } else {
                        Ok(Value::Point((p.to_vec2() / k).to_point()))
                    }
                }
                _ => Err(err(
                    span,
                    "only * and / are defined between a point and a number",
                )),
            },
            (a, b) => Err(err(
                span,
                format!(
                    "invalid operand types: {} and {}",
                    a.type_name(),
                    b.type_name()
                ),
            )),
        }
    }

    fn eval_member(
        &self,
        span: Span,
        object: &Value,
        property: &str,
    ) -> Result<Value, EvalError> {
        match object {
            Value::Point(point) => match property {
                "x" => Ok(Value::Number(point.x)),
                "y" => Ok(Value::Number(point.y)),
                _ => Err(err(span, format!("points have .x and .y, not .{property}"))),
            },
            Value::Node(node) => match property {
                "pos" => Ok(Value::Point(node.pos)),
                "dir" => Ok(Value::Number(node.dir)),
                "width" => Ok(Value::Number(node.width())),
                "left" => Ok(Value::Point(node.left())),
                "right" => Ok(Value::Point(node.right())),
                _ => Err(err(
                    span,
                    format!("nodes have .pos, .dir, .width, .left and .right, not .{property}"),
                )),
            },
            other => Err(err(
                span,
                format!("{} values have no fields", other.type_name()),
            )),
        }
    }

    fn eval_transform(
        &mut self,
        span: Span,
        operand: &Expr,
        op: TransformOp,
        argument: Option<&Expr>,
    ) -> Result<Value, EvalError> {
        let operand_value = self.eval_expr(operand)?;

        if op == TransformOp::Reversed {
            let Value::Path(path) = operand_value else {
                return Err(err(
                    span,
                    format!("reversed applies to paths, got {}", operand_value.type_name()),
                ));
            };
            return Ok(Value::Path(reverse_path(path)));
        }

        let argument = argument.expect("non-reversed transforms have an argument");
        let affine = match op {
            TransformOp::Shifted => {
                let delta = self.expect_point(argument)?;
                Affine::translate(delta.to_vec2())
            }
            TransformOp::Scaled => Affine::scale(self.expect_number(argument)?),
            TransformOp::XScaled => {
                Affine::scale_non_uniform(self.expect_number(argument)?, 1.0)
            }
            TransformOp::YScaled => {
                Affine::scale_non_uniform(1.0, self.expect_number(argument)?)
            }
            TransformOp::Rotated => {
                Affine::rotate(self.expect_number(argument)?.to_radians())
            }
            TransformOp::Slanted => {
                let k = self.expect_number(argument)?;
                // (x, y) -> (x + k*y, y)
                Affine::new([1.0, 0.0, k, 1.0, 0.0, 0.0])
            }
            TransformOp::Reversed => unreachable!("handled above"),
        };

        // Uniform scaling also scales node widths.
        let width_scale = match op {
            TransformOp::Scaled => transform_angle_preserving_scale(&affine),
            _ => 1.0,
        };

        match operand_value {
            Value::Point(point) => Ok(Value::Point(affine * point)),
            Value::Node(node) => Ok(Value::Node(NodeValue {
                pos: affine * node.pos,
                dir: transform_angle(&affine, node.dir),
                left_offset: node.left_offset * width_scale,
                right_offset: node.right_offset * width_scale,
                nib: transform_angle(&affine, node.nib),
            })),
            Value::Path(path) => Ok(Value::Path(PathValue {
                points: path
                    .points
                    .iter()
                    .map(|point| PathPoint {
                        pos: affine * point.pos,
                        enter: point.enter.map(|angle| transform_angle(&affine, angle)),
                        leave: point.leave.map(|angle| transform_angle(&affine, angle)),
                        corner: point.corner,
                    })
                    .collect(),
                joints: path.joints,
                closed: path.closed,
            })),
            other => Err(err(
                span,
                format!(
                    "transforms apply to points, nodes and paths, got {}",
                    other.type_name()
                ),
            )),
        }
    }

    fn eval_call(
        &mut self,
        span: Span,
        function: &str,
        arguments: &[Rhs],
    ) -> Result<Value, EvalError> {
        let values = arguments
            .iter()
            .map(|argument| self.eval_rhs(argument))
            .collect::<Result<Vec<_>, _>>()?;

        if self.defs.contains_key(function) {
            return self.call_def(span, function, values);
        }

        let expect_arity = |count: usize| -> Result<(), EvalError> {
            if values.len() == count {
                Ok(())
            } else {
                Err(err(
                    span,
                    format!("{function}() expects {count} argument(s), got {}", values.len()),
                ))
            }
        };
        let number_at = |index: usize| -> Result<f64, EvalError> {
            match &values[index] {
                Value::Number(number) => Ok(*number),
                other => Err(err(
                    span,
                    format!(
                        "{function}() expects a number for argument {}, got {}",
                        index + 1,
                        other.type_name()
                    ),
                )),
            }
        };
        let point_at = |index: usize| -> Result<Point, EvalError> {
            match &values[index] {
                Value::Point(point) => Ok(*point),
                other => Err(err(
                    span,
                    format!(
                        "{function}() expects a point for argument {}, got {}",
                        index + 1,
                        other.type_name()
                    ),
                )),
            }
        };

        match function {
            "min" => {
                expect_arity(2)?;
                Ok(Value::Number(number_at(0)?.min(number_at(1)?)))
            }
            "max" => {
                expect_arity(2)?;
                Ok(Value::Number(number_at(0)?.max(number_at(1)?)))
            }
            "abs" => {
                expect_arity(1)?;
                Ok(Value::Number(number_at(0)?.abs()))
            }
            "sqrt" => {
                expect_arity(1)?;
                Ok(Value::Number(number_at(0)?.sqrt()))
            }
            "floor" => {
                expect_arity(1)?;
                Ok(Value::Number(number_at(0)?.floor()))
            }
            "ceil" => {
                expect_arity(1)?;
                Ok(Value::Number(number_at(0)?.ceil()))
            }
            "round" => {
                expect_arity(1)?;
                Ok(Value::Number(number_at(0)?.round()))
            }
            "sin" => {
                expect_arity(1)?;
                Ok(Value::Number(number_at(0)?.to_radians().sin()))
            }
            "cos" => {
                expect_arity(1)?;
                Ok(Value::Number(number_at(0)?.to_radians().cos()))
            }
            "tan" => {
                expect_arity(1)?;
                Ok(Value::Number(number_at(0)?.to_radians().tan()))
            }
            "angle" => {
                expect_arity(1)?;
                let point = point_at(0)?;
                Ok(Value::Number(point.y.atan2(point.x).to_degrees()))
            }
            "unitvec" => {
                expect_arity(1)?;
                Ok(Value::Point(unitvec(number_at(0)?).to_point()))
            }
            "length" => {
                expect_arity(1)?;
                Ok(Value::Number(point_at(0)?.to_vec2().length()))
            }
            "distance" => {
                expect_arity(2)?;
                Ok(Value::Number(point_at(0)?.distance(point_at(1)?)))
            }
            "intersection" => {
                expect_arity(2)?;
                let (a, b) = (&values[0], &values[1]);
                let (Value::Path(a), Value::Path(b)) = (a, b) else {
                    return Err(err(span, "intersection() expects two paths"));
                };
                intersection(a, b)
                    .map(Value::Point)
                    .ok_or_else(|| err(span, "paths do not intersect"))
            }
            other => Err(err(span, format!("unknown function: {other}"))),
        }
    }

    /// Call a user `def`. The body runs in a fresh scope on top of the file
    /// top level — lexical scoping, so a caller's glyph locals are invisible.
    fn call_def(
        &mut self,
        span: Span,
        function: &str,
        values: Vec<Value>,
    ) -> Result<Value, EvalError> {
        let def = self.defs.get(function).cloned().expect("checked by caller");
        if self.call_stack.contains(&def.name) {
            return Err(err(
                span,
                format!("recursive call to '{}' is not supported", def.name),
            ));
        }
        if values.len() != def.parameters.len() {
            return Err(err(
                span,
                format!(
                    "{}() expects {} argument(s), got {}",
                    def.name,
                    def.parameters.len(),
                    values.len()
                ),
            ));
        }

        // Hide any scopes inner to the file top level (e.g. glyph locals).
        let hidden = self.scopes.split_off(2);
        self.push_scope();
        for (parameter, value) in def.parameters.iter().zip(values) {
            self.define(parameter, value, span)?;
        }
        self.call_stack.push(def.name.clone());

        let mut run = || -> Result<Value, EvalError> {
            for statement in &def.body {
                self.exec_statement(statement)?;
            }
            let scope = self.scopes.last().expect("def scope");
            let mut returns = Vec::with_capacity(def.returns.len());
            for name in &def.returns {
                match scope.get(name) {
                    Some(value) => returns.push(value.clone()),
                    None => {
                        return Err(err(
                            span,
                            format!("{}() does not bind its declared return '{name}'", def.name),
                        ));
                    }
                }
            }
            if returns.len() == 1 {
                Ok(returns.pop().expect("one return"))
            } else {
                Ok(Value::Tuple(returns))
            }
        };
        let result = run();

        self.call_stack.pop();
        self.pop_scope();
        self.scopes.extend(hidden);
        result
    }

    fn expect_number(&mut self, expr: &Expr) -> Result<f64, EvalError> {
        match self.eval_expr(expr)? {
            Value::Number(number) => Ok(number),
            other => Err(err(
                expr.span,
                format!("expected a number, got {}", other.type_name()),
            )),
        }
    }

    fn expect_point(&mut self, expr: &Expr) -> Result<Point, EvalError> {
        match self.eval_expr(expr)? {
            Value::Point(point) => Ok(point),
            other => Err(err(
                expr.span,
                format!("expected a point, got {}", other.type_name()),
            )),
        }
    }

    fn expect_bool(&mut self, expr: &Expr) -> Result<bool, EvalError> {
        match self.eval_expr(expr)? {
            Value::Bool(condition) => Ok(condition),
            other => Err(err(
                expr.span,
                format!("expected a boolean, got {}", other.type_name()),
            )),
        }
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Transform an angle (degrees) by the linear part of an affine map.
fn transform_angle(affine: &Affine, degrees: f64) -> f64 {
    let [a, b, c, d, _, _] = affine.as_coeffs();
    let v = unitvec(degrees);
    let transformed = Vec2::new(a * v.x + c * v.y, b * v.x + d * v.y);
    transformed.y.atan2(transformed.x).to_degrees()
}

/// The uniform scale factor of an affine map (valid for uniform scaling).
fn transform_angle_preserving_scale(affine: &Affine) -> f64 {
    let [a, b, _, _, _, _] = affine.as_coeffs();
    Vec2::new(a, b).length()
}

fn reverse_path(path: PathValue) -> PathValue {
    let mut points: Vec<PathPoint> = path
        .points
        .into_iter()
        .map(|point| PathPoint {
            pos: point.pos,
            // Travelling the other way: incoming and outgoing swap and flip.
            enter: point.leave.map(|angle| angle + 180.0),
            leave: point.enter.map(|angle| angle + 180.0),
            corner: point.corner,
        })
        .collect();
    points.reverse();
    let mut joints = path.joints;
    if path.closed {
        // The closing joint stays the closing joint.
        let closing = joints.pop().expect("closed path has a closing joint");
        joints.reverse();
        joints.push(closing);
    } else {
        joints.reverse();
    }
    PathValue {
        points,
        joints,
        closed: path.closed,
    }
}

/// First intersection of two paths, treating their segments as straight
/// chords. Covers the common construction: intersection(a -- b, c -- d).
fn intersection(a: &PathValue, b: &PathValue) -> Option<Point> {
    for (a0, a1) in chords(a) {
        for (b0, b1) in chords(b) {
            if let Some(point) = segment_intersection(a0, a1, b0, b1) {
                return Some(point);
            }
        }
    }
    None
}

fn chords(path: &PathValue) -> Vec<(Point, Point)> {
    let mut chords = Vec::new();
    let count = path.points.len();
    for index in 0..count.saturating_sub(1) {
        chords.push((path.points[index].pos, path.points[index + 1].pos));
    }
    if path.closed && count > 1 {
        chords.push((path.points[count - 1].pos, path.points[0].pos));
    }
    chords
}

fn segment_intersection(p1: Point, p2: Point, p3: Point, p4: Point) -> Option<Point> {
    let d1 = p2 - p1;
    let d2 = p4 - p3;
    let denominator = d1.cross(d2);
    if denominator.abs() < 1e-12 {
        return None; // parallel
    }
    let t = (p3 - p1).cross(d2) / denominator;
    let u = (p3 - p1).cross(d1) / denominator;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(p1 + t * d1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::lower;
    use crate::parse::parse;

    fn eval_ok(source: &str) -> Evaluator {
        let tree = parse(source).expect("parses");
        let program = lower(&tree, source).expect("lowers");
        let mut evaluator = Evaluator::new();
        for item in &program.items {
            match item {
                Item::Stmt(stmt) => evaluator.exec_statement(stmt).expect("evaluates"),
                other => panic!("test source uses unsupported item {other:?}"),
            }
        }
        evaluator
    }

    fn eval_err(source: &str) -> EvalError {
        let tree = parse(source).expect("parses");
        let program = lower(&tree, source).expect("lowers");
        let mut evaluator = Evaluator::new();
        for item in &program.items {
            match item {
                Item::Stmt(stmt) => {
                    if let Err(error) = evaluator.exec_statement(stmt) {
                        return error;
                    }
                }
                other => panic!("test source uses unsupported item {other:?}"),
            }
        }
        panic!("expected an error");
    }

    fn number(evaluator: &Evaluator, name: &str) -> f64 {
        match evaluator.lookup(name) {
            Some(Value::Number(number)) => *number,
            other => panic!("{name} is {other:?}"),
        }
    }

    fn point(evaluator: &Evaluator, name: &str) -> Point {
        match evaluator.lookup(name) {
            Some(Value::Point(point)) => *point,
            other => panic!("{name} is {other:?}"),
        }
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    fn assert_point(actual: Point, x: f64, y: f64) {
        assert_close(actual.x, x);
        assert_close(actual.y, y);
    }

    #[test]
    fn arithmetic_and_implicit_multiplication() {
        let evaluator = eval_ok("u = 100;\nx = 2 + 3 * 4;\ny = 2u + 1;\nz = 2 ^ 3 ^ 2;\n");
        assert_close(number(&evaluator, "x"), 14.0);
        assert_close(number(&evaluator, "y"), 201.0);
        assert_close(number(&evaluator, "z"), 512.0); // right-associative
    }

    #[test]
    fn point_arithmetic() {
        let evaluator = eval_ok(
            "a = (1, 2);\nb = (10, 20);\ns = a + b;\nd = b - a;\nk = 3 * a;\nh = b / 2;\n",
        );
        assert_point(point(&evaluator, "s"), 11.0, 22.0);
        assert_point(point(&evaluator, "d"), 9.0, 18.0);
        assert_point(point(&evaluator, "k"), 3.0, 6.0);
        assert_point(point(&evaluator, "h"), 5.0, 10.0);
    }

    #[test]
    fn point_plus_number_is_an_error() {
        let error = eval_err("a = (1, 2) + 3;\n");
        assert!(error.message.contains("point"), "{}", error.message);
    }

    #[test]
    fn mediation_on_numbers_and_points() {
        let evaluator = eval_ok(
            "m = 0.5[10, 20];\np = 0.25[(0, 0), (100, 40)];\nt = 2[0, 10];\n",
        );
        assert_close(number(&evaluator, "m"), 15.0);
        assert_point(point(&evaluator, "p"), 25.0, 10.0);
        assert_close(number(&evaluator, "t"), 20.0); // extrapolation works
    }

    #[test]
    fn member_access_on_points() {
        let evaluator = eval_ok("p = (3, 4);\na = p.x;\nb = p.y;\n");
        assert_close(number(&evaluator, "a"), 3.0);
        assert_close(number(&evaluator, "b"), 4.0);
    }

    #[test]
    fn transforms_on_points() {
        let evaluator = eval_ok(
            "a = (1, 0) rotated 90;\nb = (10, 20) shifted (5, 5);\nc = (10, 20) scaled 2;\nd = (10, 20) slanted 0.5;\ne = (10, 20) xscaled 3;\n",
        );
        assert_point(point(&evaluator, "a"), 0.0, 1.0);
        assert_point(point(&evaluator, "b"), 15.0, 25.0);
        assert_point(point(&evaluator, "c"), 20.0, 40.0);
        assert_point(point(&evaluator, "d"), 20.0, 20.0);
        assert_point(point(&evaluator, "e"), 30.0, 20.0);
    }

    #[test]
    fn conditional_is_lazy() {
        let evaluator = eval_ok("x = 1 if 2 < 3 else undefined_variable;\n");
        assert_close(number(&evaluator, "x"), 1.0);
    }

    #[test]
    fn boolean_operators_short_circuit() {
        let evaluator =
            eval_ok("x = 1 if 1 < 2 or undefined < 1 else 0;\ny = 1 if 2 < 1 and undefined < 1 else 0;\n");
        assert_close(number(&evaluator, "x"), 1.0);
        assert_close(number(&evaluator, "y"), 0.0);
    }

    #[test]
    fn builtin_functions() {
        let evaluator = eval_ok(
            "a = min(3, 7);\nb = sin(90);\nc = angle((0, 5));\nd = length((3, 4));\ne = distance((0, 0), (3, 4));\nu = unitvec(180);\n",
        );
        assert_close(number(&evaluator, "a"), 3.0);
        assert_close(number(&evaluator, "b"), 1.0);
        assert_close(number(&evaluator, "c"), 90.0);
        assert_close(number(&evaluator, "d"), 5.0);
        assert_close(number(&evaluator, "e"), 5.0);
        assert_point(point(&evaluator, "u"), -1.0, 0.0);
    }

    #[test]
    fn prelude_direction_constants() {
        let evaluator = eval_ok("a = up;\nb = down;\nc = left;\nd = right;\n");
        assert_close(number(&evaluator, "a"), 90.0);
        assert_close(number(&evaluator, "b"), 270.0);
        assert_close(number(&evaluator, "c"), 180.0);
        assert_close(number(&evaluator, "d"), 0.0);
    }

    #[test]
    fn rebinding_in_same_scope_is_an_error() {
        let error = eval_err("x = 1;\nx = 2;\n");
        assert!(error.message.contains("immutable"), "{}", error.message);
    }

    #[test]
    fn undefined_variable_reports_span() {
        let error = eval_err("x = 1;\ny = missing + 1;\n");
        assert_eq!(error.span.line, 2);
        assert!(error.message.contains("missing"));
    }

    #[test]
    fn division_by_zero() {
        let error = eval_err("x = 1 / 0;\n");
        assert!(error.message.contains("zero"));
    }

    #[test]
    fn node_derived_points() {
        // Travelling up with width 10: left bank is to the west.
        let evaluator = eval_ok(
            "node n = (100, 50) dir up width 10;\nl = n.left;\nr = n.right;\nw = n.width;\np = n.pos;\nd = n.dir;\n",
        );
        assert_point(point(&evaluator, "l"), 95.0, 50.0);
        assert_point(point(&evaluator, "r"), 105.0, 50.0);
        assert_close(number(&evaluator, "w"), 10.0);
        assert_point(point(&evaluator, "p"), 100.0, 50.0);
        assert_close(number(&evaluator, "d"), 90.0);
    }

    #[test]
    fn node_with_asymmetric_width_and_nib() {
        let evaluator = eval_ok(
            "node m = (0, 0) dir up width (2, 8);\nl = m.left;\nr = m.right;\nnode k = (0, 0) dir up width 10 nib 45;\nkl = k.left;\n",
        );
        assert_point(point(&evaluator, "l"), -2.0, 0.0);
        assert_point(point(&evaluator, "r"), 8.0, 0.0);
        let expected = 5.0 / f64::sqrt(2.0);
        assert_point(point(&evaluator, "kl"), expected, expected);
    }

    #[test]
    fn path_evaluation() {
        let evaluator = eval_ok(
            "a = (0, 0);\nb = (100, 0);\np = a {45} .. b{corner} -- (50, 50) .. cycle;\n",
        );
        let Some(Value::Path(path)) = evaluator.lookup("p") else {
            panic!("p is not a path")
        };
        assert_eq!(path.points.len(), 3);
        assert!(path.closed);
        assert_eq!(path.joints.len(), 3);
        assert_close(path.points[0].leave.unwrap(), 45.0);
        assert!(path.points[1].corner);
        assert!(path.points[0].enter.is_none());
    }

    #[test]
    fn node_used_directly_in_path_is_an_error() {
        let error = eval_err("node n = (0, 0) dir up width 10;\np = n .. (5, 5);\n");
        assert!(error.message.contains(".pos"), "{}", error.message);
    }

    #[test]
    fn reversed_path_swaps_constraints() {
        let evaluator = eval_ok("p = (0, 0) {up} .. (10, 10);\nq = p reversed;\n");
        let Some(Value::Path(path)) = evaluator.lookup("q") else {
            panic!()
        };
        assert_point(path.points[0].pos, 10.0, 10.0);
        // The old leaving-up constraint is now an entering-down constraint
        // on the last point.
        assert_close(path.points[1].enter.unwrap(), 270.0);
    }

    #[test]
    fn transformed_node_rotates_direction_and_nib() {
        let evaluator = eval_ok(
            "node n = (100, 0) dir up width 10;\nnode m = (0, 0) dir up width 10;\nr = (n rotated 90).pos;\nd = (n rotated 90).dir;\ns = (m scaled 2).width;\n",
        );
        assert_point(point(&evaluator, "r"), 0.0, 100.0);
        assert_close(number(&evaluator, "d"), 180.0);
        assert_close(number(&evaluator, "s"), 20.0);
    }

    #[test]
    fn intersection_of_line_paths() {
        let evaluator = eval_ok(
            "p = intersection((0, 0) -- (10, 10), (0, 10) -- (10, 0));\n",
        );
        assert_point(point(&evaluator, "p"), 5.0, 5.0);
    }

    #[test]
    fn shadowing_note_scopes_are_created_by_blocks_only() {
        // Same-scope rebinding fails even when the first binding came from
        // the prelude-shadowing top level.
        let evaluator = eval_ok("left = 5;\n");
        assert_close(number(&evaluator, "left"), 5.0);
    }
}
