//! Evaluate the AST: single pass, lexically scoped, immutable bindings.

use crate::ast::*;
use crate::doc::{Document, Glyph, Param, SketchCommand};
use crate::shapes;
use crate::value::{NodeValue, PathJoint, PathPoint, PathValue, PenValue, Value, unitvec};
use kurbo::{Affine, Point, Vec2};
use std::collections::{HashMap, HashSet};

const BUILTIN_FUNCTIONS: &[&str] = &[
    "min",
    "max",
    "clamp",
    "abs",
    "sign",
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
    "point",
    "direction",
    "directionpoint",
    "jointcap",
    "circle",
    "ellipse",
    "roundrect",
    "superellipse",
    "draw",
    "fill",
];

/// Upper bound on `superellipse()` samples. The count usually comes from a
/// slider, and a runaway one would hang the playground rather than draw.
const MAX_SAMPLES: f64 = 1024.0;

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

/// Loads imported source files. ductus-core has no filesystem access (it
/// targets WebAssembly), so the host provides resolution and reading.
pub trait Importer {
    /// Resolve `path` relative to the importing file (`from` is the
    /// canonical id of the importing file, None for the root) and return
    /// (canonical id, source text). The id drives cycle detection and
    /// include-once, so it must be stable for the same file.
    fn load(&self, path: &str, from: Option<&str>) -> Result<(String, String), String>;
}

/// For hosts without file access: every import is an error.
pub struct NoImporter;

impl Importer for NoImporter {
    fn load(&self, _path: &str, _from: Option<&str>) -> Result<(String, String), String> {
        Err("imports are not supported in this environment".to_string())
    }
}

/// Evaluate a whole program into a renderable document.
pub fn evaluate(
    program: &Program,
    overrides: &HashMap<String, f64>,
    importer: &dyn Importer,
) -> Result<Document, EvalError> {
    let mut session = Session {
        evaluator: Evaluator::new(),
        document: Document::default(),
        known_params: HashSet::new(),
        overrides,
        importer,
        import_stack: Vec::new(),
        imported: HashSet::new(),
    };
    session.eval_items(&program.items)?;

    for name in overrides.keys() {
        if !session.known_params.contains(name) {
            return Err(err(
                Span { line: 1, column: 1 },
                format!("--set {name}: no such param in this file"),
            ));
        }
    }

    let mut document = session.document;
    document.sketch = session.evaluator.sketch;
    Ok(document)
}

/// One evaluation run: the evaluator plus the document and import state
/// accumulated across the root program and everything it imports.
struct Session<'a> {
    evaluator: Evaluator,
    document: Document,
    known_params: HashSet<String>,
    overrides: &'a HashMap<String, f64>,
    importer: &'a dyn Importer,
    /// Canonical ids of files currently being imported, outermost first.
    import_stack: Vec<String>,
    /// Canonical ids already evaluated: imports are include-once.
    imported: HashSet<String>,
}

impl Session<'_> {
    fn eval_items(&mut self, items: &[Item]) -> Result<(), EvalError> {
        for item in items {
            match item {
                Item::Params(params) => {
                    for param in params {
                        self.known_params.insert(param.name.clone());
                        let resolved = self.evaluator.eval_param(param, self.overrides)?;
                        self.document.params.push(resolved);
                    }
                }
                Item::Metrics(entries) => {
                    for entry in entries {
                        let value = self.evaluator.eval_rhs(&entry.value)?;
                        let Value::Number(number) = value else {
                            return Err(err(
                                entry.span,
                                format!("metrics must be numbers, got {}", value.type_name()),
                            ));
                        };
                        self.evaluator
                            .define(&entry.name, Value::Number(number), entry.span)?;
                        self.document
                            .metrics
                            .values
                            .insert(entry.name.clone(), number);
                    }
                }
                Item::Pen(pen) => self.evaluator.exec_pen(pen)?,
                Item::Def(def) => self.evaluator.register_def(def)?,
                Item::Glyph(glyph) => {
                    if self.document.glyphs.iter().any(|g| g.name == glyph.name) {
                        return Err(err(
                            glyph.span,
                            format!("glyph '{}' is already defined", glyph.name),
                        ));
                    }
                    let evaluated = self.evaluator.eval_glyph(glyph)?;
                    self.document.glyphs.push(evaluated);
                }
                Item::Import { path, span } => self.import(path, *span)?,
                Item::Stmt(stmt) => {
                    self.evaluator.exec_statement(stmt)?;
                    self.record_bindings(stmt);
                }
            }
        }
        Ok(())
    }

    /// Note the numeric bindings a top-level statement just made, in source
    /// order. Params and metrics arrive through their own `Item` arms and
    /// keep their own fields, so they are absent here by construction.
    fn record_bindings(&mut self, stmt: &Stmt) {
        let names: Vec<&str> = match stmt {
            Stmt::Assign(binding) => vec![binding.name.as_str()],
            Stmt::Destructure { names, .. } => names.iter().map(String::as_str).collect(),
            // A node is not a number; a `for` body's bindings live and die
            // in its own scope; a call binds nothing.
            Stmt::Node(_) | Stmt::Call { .. } | Stmt::For { .. } => return,
        };
        for name in names {
            if let Some(Value::Number(number)) = self.evaluator.lookup(name) {
                self.document.bindings.push((name.to_string(), *number));
            }
        }
    }

    /// Evaluate an imported file's items into the current scope at the
    /// import's position. Include-once by canonical id; cycles error.
    fn import(&mut self, path: &str, span: Span) -> Result<(), EvalError> {
        let from = self.import_stack.last().map(String::as_str);
        let (id, source) = self
            .importer
            .load(path, from)
            .map_err(|message| err(span, message))?;
        if self.imported.contains(&id) {
            return Ok(());
        }
        if self.import_stack.contains(&id) {
            return Err(err(
                span,
                format!("import cycle: {} -> {id}", self.import_stack.join(" -> ")),
            ));
        }
        let prefix = |message: String| err(span, format!("{path}: {message}"));
        let tree = crate::parse::parse(&source).map_err(|error| prefix(error.to_string()))?;
        let program =
            crate::lower::lower(&tree, &source).map_err(|error| prefix(error.to_string()))?;
        self.import_stack.push(id.clone());
        let result = self.eval_items(&program.items);
        self.import_stack.pop();
        self.imported.insert(id);
        result.map_err(|error| prefix(error.to_string()))
    }
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
                format!(
                    "{} = {} is outside its range {}..{}",
                    param.name, value, min, max
                ),
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
                format!(
                    "'{}' is a built-in function and cannot be redefined",
                    def.name
                ),
            ));
        }
        if self.defs.contains_key(&def.name) {
            return Err(err(
                def.span,
                format!("def '{}' is already defined", def.name),
            ));
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
                    // The name is also an ordinary binding, so later code
                    // can reference and query the path.
                    self.define(&binding.name, Value::Path(path.clone()), binding.span)?;
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
                    self.define(&binding.name, Value::Path(path.clone()), binding.span)?;
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

        // And the numeric ones, for the inspector. `advance` has its own
        // field; the scope is a map, so these come out by name rather than
        // in source order.
        let mut bindings: Vec<(String, f64)> = self
            .scopes
            .last()
            .expect("glyph scope")
            .iter()
            .filter_map(|(name, value)| match value {
                Value::Number(number) if name != "advance" => Some((name.clone(), *number)),
                _ => None,
            })
            .collect();
        bindings.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(Glyph {
            name: glyph.name.clone(),
            unicode: glyph.unicode,
            advance,
            contours,
            guides,
            anchors,
            nodes,
            bindings,
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
                                format!("{function}() expects paths, got {}", value.type_name()),
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
            Stmt::For {
                name,
                iterable,
                body,
                span,
            } => {
                let values: Vec<Value> = match iterable {
                    Iterable::Range { min, max } => {
                        let min = self.expect_number(min)?;
                        let max = self.expect_number(max)?;
                        if !min.is_finite() || !max.is_finite() {
                            return Err(err(*span, "for range bounds must be finite"));
                        }
                        // Inclusive both ends, like the params `in min..max`
                        // range. min > max iterates zero times.
                        let count = if max >= min {
                            (max - min) as usize + 1
                        } else {
                            0
                        };
                        (0..count).map(|k| Value::Number(min + k as f64)).collect()
                    }
                    Iterable::List(items) => items
                        .iter()
                        .map(|item| self.eval_expr(item))
                        .collect::<Result<Vec<_>, _>>()?,
                    Iterable::Expr(expr) => {
                        let value = self.eval_expr(expr)?;
                        let Value::List(values) = value else {
                            return Err(err(
                                expr.span,
                                format!("for expects a list, got {}", value.type_name()),
                            ));
                        };
                        values
                    }
                };
                for value in values {
                    // A fresh scope per iteration: bindings are immutable,
                    // so the loop variable and body locals must not carry
                    // over between iterations.
                    self.push_scope();
                    let mut run = || -> Result<(), EvalError> {
                        self.define(name, value.clone(), *span)?;
                        for statement in body {
                            self.exec_statement(statement)?;
                        }
                        Ok(())
                    };
                    let result = run();
                    self.pop_scope();
                    result?;
                }
                Ok(())
            }
        }
    }

    /// Evaluate a `pen` block into a first-class pen binding.
    pub fn exec_pen(&mut self, pen: &PenDecl) -> Result<(), EvalError> {
        let width = self.expect_number(&pen.width)?;
        if width <= 0.0 {
            return Err(err(
                pen.span,
                format!("pen '{}' needs width > 0, got {width}", pen.name),
            ));
        }
        let height = match &pen.height {
            Some(expr) => self.expect_number(expr)?,
            None => 0.0,
        };
        if height < 0.0 {
            return Err(err(
                pen.span,
                format!("pen '{}' needs height >= 0, got {height}", pen.name),
            ));
        }
        let angle = match &pen.angle {
            Some(expr) => self.expect_number(expr)?,
            None => 0.0,
        };
        let value = PenValue {
            a: width / 2.0,
            b: height / 2.0,
            angle,
        };
        self.define(&pen.name, Value::Pen(value), pen.span)
    }

    fn eval_node(&mut self, node: &NodeDecl) -> Result<NodeValue, EvalError> {
        let pos = self.expect_point(&node.position)?;
        let dir = self.expect_number(&node.dir)?;

        if let Some(pen_expr) = &node.pen {
            let value = self.eval_expr(pen_expr)?;
            let Value::Pen(pen) = value else {
                return Err(err(
                    pen_expr.span,
                    format!("expected a pen, got {}", value.type_name()),
                ));
            };
            // Edge points are the pen's support points along the left
            // normal of travel. A razor (b = 0) is degenerate there and
            // keeps the legacy fixed-axis rule instead.
            let (offset, nib) = if pen.b > 0.0 {
                let edge = pen.support(dir + 90.0);
                (edge.length(), edge.y.atan2(edge.x).to_degrees())
            } else {
                (pen.a, pen.angle)
            };
            return Ok(NodeValue {
                pos,
                dir,
                left_offset: offset,
                right_offset: offset,
                nib,
                pen: Some(pen),
            });
        }

        let width_expr = node
            .width
            .as_ref()
            .expect("lowering guarantees width or pen");
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
            pen: None,
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
        let mut joints: Vec<PathJoint> = Vec::with_capacity(path.joints.len());
        for (index, segment) in path.segments.iter().enumerate() {
            if index > 0 {
                joints.push(PathJoint::from(path.joints[index - 1]));
            }
            let pos = match self.eval_expr(&segment.point)? {
                Value::Point(point) => point,
                // A path element splices in: its points and joints join the
                // host path, the written joints connecting to its ends.
                Value::Path(sub) => {
                    if sub.closed {
                        return Err(err(
                            segment.point.span,
                            "a closed path cannot be spliced into a path",
                        ));
                    }
                    if segment.enter.is_some() || segment.leave.is_some() {
                        return Err(err(
                            segment.point.span,
                            "a spliced path carries its own tangents; \
                             direction constraints cannot apply to it",
                        ));
                    }
                    points.extend(sub.points);
                    joints.extend(sub.joints);
                    continue;
                }
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
            let mut resolve =
                |spec: &Option<DirSpec>, evaluator: &mut Self| -> Result<Option<f64>, EvalError> {
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
        if path.closed {
            let closing = path.joints.last().expect("closed path has a closing joint");
            joints.push(PathJoint::from(*closing));
        }
        Ok(PathValue {
            points,
            joints,
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
                    (Value::Point(a), Value::Point(b)) => Ok(Value::Point(a + t * (b - a))),
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

    fn eval_member(&self, span: Span, object: &Value, property: &str) -> Result<Value, EvalError> {
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
                "tip" | "tail" => {
                    let point = if property == "tip" {
                        node.tip()
                    } else {
                        node.tail()
                    };
                    point.map(Value::Point).ok_or_else(|| {
                        err(
                            span,
                            format!(
                                ".{property} needs a pen with height > 0; \
                                 a razor has no defined front point"
                            ),
                        )
                    })
                }
                "tipcap" | "tailcap" => {
                    let cap = if property == "tipcap" {
                        node.tip_cap()
                    } else {
                        node.tail_cap()
                    };
                    cap.map(Value::Path).ok_or_else(|| {
                        err(
                            span,
                            format!(
                                ".{property} needs a pen with height > 0; \
                                 a razor's terminal is not a curve"
                            ),
                        )
                    })
                }
                _ => Err(err(
                    span,
                    format!(
                        "nodes have .pos, .dir, .width, .left, .right, .tip, .tail, \
                         .tipcap and .tailcap, not .{property}"
                    ),
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
                    format!(
                        "reversed applies to paths, got {}",
                        operand_value.type_name()
                    ),
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
            TransformOp::XScaled => Affine::scale_non_uniform(self.expect_number(argument)?, 1.0),
            TransformOp::YScaled => Affine::scale_non_uniform(1.0, self.expect_number(argument)?),
            TransformOp::Rotated => Affine::rotate(self.expect_number(argument)?.to_radians()),
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
                pen: node.pen.map(|pen| PenValue {
                    a: pen.a * width_scale,
                    b: pen.b * width_scale,
                    angle: transform_angle(&affine, pen.angle),
                }),
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
                joints: path
                    .joints
                    .iter()
                    .map(|joint| match joint {
                        PathJoint::Fixed { c1, c2 } => PathJoint::Fixed {
                            c1: affine * *c1,
                            c2: affine * *c2,
                        },
                        other => *other,
                    })
                    .collect(),
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
                    format!(
                        "{function}() expects {count} argument(s), got {}",
                        values.len()
                    ),
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
        let path_at = |index: usize| -> Result<&PathValue, EvalError> {
            match &values[index] {
                Value::Path(path) => Ok(path),
                other => Err(err(
                    span,
                    format!(
                        "{function}() expects a path for argument {}, got {}",
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
            "clamp" => {
                expect_arity(3)?;
                let (value, low, high) = (number_at(0)?, number_at(1)?, number_at(2)?);
                if low > high {
                    return Err(err(
                        span,
                        format!("clamp() lower bound {low} is above upper bound {high}"),
                    ));
                }
                // Not f64::clamp, which panics on reversed or NaN bounds.
                Ok(Value::Number(value.max(low).min(high)))
            }
            "abs" => {
                expect_arity(1)?;
                Ok(Value::Number(number_at(0)?.abs()))
            }
            "sign" => {
                expect_arity(1)?;
                // Not f64::signum, which returns ±1 for ±0.0.
                let value = number_at(0)?;
                Ok(Value::Number(if value == 0.0 {
                    0.0
                } else {
                    value.signum()
                }))
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
            // Path queries observe the fitted curve — the geometry that
            // renders; see render.rs.
            "intersection" => {
                expect_arity(2)?;
                let (a, b) = (&values[0], &values[1]);
                let (Value::Path(a), Value::Path(b)) = (a, b) else {
                    return Err(err(span, "intersection() expects two paths"));
                };
                crate::render::intersection(a, b)
                    .map_err(|error| err(span, error.message))?
                    .map(Value::Point)
                    .ok_or_else(|| err(span, "paths do not intersect"))
            }
            "point" => {
                expect_arity(2)?;
                crate::render::point_at(path_at(0)?, number_at(1)?)
                    .map(Value::Point)
                    .map_err(|error| err(span, error.message))
            }
            "direction" => {
                expect_arity(2)?;
                crate::render::direction_at(path_at(0)?, number_at(1)?)
                    .map(Value::Number)
                    .map_err(|error| err(span, error.message))
            }
            "directionpoint" => {
                if !(2..=3).contains(&values.len()) {
                    return Err(err(
                        span,
                        format!(
                            "directionpoint() expects 2 or 3 arguments, got {}",
                            values.len()
                        ),
                    ));
                }
                // Optional 0-based index selects a later match; the two
                // argument form is the first match.
                let index = if values.len() == 3 {
                    let index = number_at(2)?;
                    if index < 0.0 || index.fract() != 0.0 {
                        return Err(err(
                            span,
                            format!(
                                "directionpoint() index must be a \
                                 non-negative integer, got {index}"
                            ),
                        ));
                    }
                    index as usize
                } else {
                    0
                };
                crate::render::direction_point(path_at(0)?, number_at(1)?, index)
                    .map(Value::Point)
                    .map_err(|error| err(span, error.message))
            }
            // The outer arc of a stroke joint; the geometry lives in
            // value.rs next to the caps it generalizes.
            "jointcap" => {
                expect_arity(2)?;
                let (Value::Node(arriving), Value::Node(leaving)) = (&values[0], &values[1])
                else {
                    return Err(err(
                        span,
                        "jointcap() expects two nodes: one arriving at the \
                         corner, one leaving it",
                    ));
                };
                arriving
                    .joint_cap(leaving)
                    .map(Value::Path)
                    .map_err(|message| err(span, message))
            }
            // Shape constructors; the geometry lives in shapes.rs, which
            // documents the rules they share.
            "circle" => {
                expect_arity(2)?;
                let radius = number_at(1)?;
                Ok(Value::Path(shapes::superellipse(
                    point_at(0)?,
                    radius,
                    radius,
                    2.0,
                    4,
                )))
            }
            "ellipse" => {
                expect_arity(3)?;
                Ok(Value::Path(shapes::superellipse(
                    point_at(0)?,
                    number_at(1)?,
                    number_at(2)?,
                    2.0,
                    8,
                )))
            }
            "roundrect" => {
                expect_arity(3)?;
                Ok(Value::Path(shapes::roundrect(
                    point_at(0)?,
                    point_at(1)?,
                    number_at(2)?,
                )))
            }
            "superellipse" => {
                expect_arity(5)?;
                let exponent = number_at(3)?;
                if exponent < 2.0 {
                    return Err(err(
                        span,
                        format!(
                            "superellipse() exponent must be at least 2 \
                             (2 is the ellipse), got {exponent}"
                        ),
                    ));
                }
                // A multiple of four puts a sample on each axis extreme,
                // where every round shape needs a point.
                let samples = number_at(4)?;
                if !(4.0..=MAX_SAMPLES).contains(&samples) || samples % 4.0 != 0.0 {
                    return Err(err(
                        span,
                        format!(
                            "superellipse() takes a multiple of 4 samples, \
                             from 4 to {MAX_SAMPLES}, got {samples}"
                        ),
                    ));
                }
                Ok(Value::Path(shapes::superellipse(
                    point_at(0)?,
                    number_at(1)?,
                    number_at(2)?,
                    exponent,
                    samples as usize,
                )))
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
    // Travelling the other way through a fixed cubic swaps its controls.
    let mut joints: Vec<PathJoint> = path
        .joints
        .into_iter()
        .map(|joint| match joint {
            PathJoint::Fixed { c1, c2 } => PathJoint::Fixed { c1: c2, c2: c1 },
            other => other,
        })
        .collect();
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
                Item::Pen(pen) => evaluator.exec_pen(pen).expect("evaluates"),
                Item::Def(def) => evaluator.register_def(def).expect("registers"),
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
            let result = match item {
                Item::Stmt(stmt) => evaluator.exec_statement(stmt),
                Item::Pen(pen) => evaluator.exec_pen(pen),
                other => panic!("test source uses unsupported item {other:?}"),
            };
            if let Err(error) = result {
                return error;
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
        let evaluator =
            eval_ok("a = (1, 2);\nb = (10, 20);\ns = a + b;\nd = b - a;\nk = 3 * a;\nh = b / 2;\n");
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
        let evaluator = eval_ok("m = 0.5[10, 20];\np = 0.25[(0, 0), (100, 40)];\nt = 2[0, 10];\n");
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
        let evaluator = eval_ok(
            "x = 1 if 1 < 2 or undefined < 1 else 0;\ny = 1 if 2 < 1 and undefined < 1 else 0;\n",
        );
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
    fn sign_is_zero_at_zero() {
        let evaluator = eval_ok("a = sign(3);\nb = sign(-0.5);\nc = sign(0);\nd = sign(-0);\n");
        assert_close(number(&evaluator, "a"), 1.0);
        assert_close(number(&evaluator, "b"), -1.0);
        assert_close(number(&evaluator, "c"), 0.0);
        assert_close(number(&evaluator, "d"), 0.0);
    }

    #[test]
    fn clamp_holds_at_both_bounds() {
        let evaluator = eval_ok(
            "a = clamp(0.28, 0.04, 0.5);\nb = clamp(1, 0.04, 0.5);\nc = clamp(-2, 0.04, 0.5);\n",
        );
        assert_close(number(&evaluator, "a"), 0.28);
        assert_close(number(&evaluator, "b"), 0.5);
        assert_close(number(&evaluator, "c"), 0.04);
    }

    #[test]
    fn clamp_with_reversed_bounds_is_an_error() {
        let error = eval_err("x = clamp(1, 0.5, 0.04);\n");
        assert!(error.message.contains("upper bound"), "{}", error.message);
    }

    /// Evaluate a whole program, the way the CLI and the playground do.
    fn document(source: &str) -> Document {
        let tree = parse(source).expect("parses");
        let program = lower(&tree, source).expect("lowers");
        evaluate(&program, &HashMap::new(), &NoImporter).expect("evaluates")
    }

    #[test]
    fn top_level_numeric_bindings_are_collected_in_source_order() {
        let document = document(concat!(
            "params { weight = 400 in 100..900; }\n",
            "metrics { upm = 1000; x = 500; }\n",
            "stem = 40 + 0.14 * weight;\n",
            "origin = (0, 0);\n",           // a point, not a number
            "hair = 0.85 * stem;\n",
        ));
        // Params and metrics keep their own fields and are not repeated here.
        assert_eq!(
            document.bindings,
            vec![("stem".to_string(), 96.0), ("hair".to_string(), 81.6)]
        );
    }

    #[test]
    fn glyph_bindings_carry_a_probe_over_a_contour() {
        let document = document(concat!(
            "metrics { upm = 1000; }\n",
            "glyph o {\n",
            "  advance = 600;\n",
            "  center = (300, 300);\n",
            "  r = 250;\n",
            "  contour main = circle(center, r);\n",
            // kappa is chosen so the arc passes exactly through 45°, so a
            // probe at the segment midpoint reads zero; the 0.027 % lives
            // between, peaking near t = 0.21. Measuring either needed an
            // SVG parser before these bindings existed.
            "  waist = distance(point(main, 0.5), center) - r;\n",
            "  bulge = distance(point(main, 0.2), center) - r;\n",
            "}\n",
        ));
        let glyph = &document.glyphs[0];
        let read = |name: &str| {
            glyph
                .bindings
                .iter()
                .find(|(bound, _)| bound == name)
                .unwrap_or_else(|| panic!("{name} is not listed"))
                .1
        };
        assert!(read("waist").abs() < 1e-9, "waist was {}", read("waist"));
        assert!(
            (read("bulge") - 0.0678).abs() < 0.001,
            "bulge was {}, expected ~0.068",
            read("bulge")
        );
        // `advance` has its own field, and `center` is a point.
        let names: Vec<&str> = glyph.bindings.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["bulge", "r", "waist"]);
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
    fn for_over_inclusive_range() {
        let evaluator = eval_ok("for i in 0..3 { draw((i, 0) -- (i, 10)); }\n");
        assert_eq!(evaluator.sketch.len(), 4); // 0, 1, 2, 3

        // Fractional start: steps of 1 while <= max.
        let evaluator = eval_ok("for i in 0.5..2 { draw((i, 0) -- (i, 10)); }\n");
        assert_eq!(evaluator.sketch.len(), 2); // 0.5, 1.5
    }

    #[test]
    fn for_with_empty_range_skips_the_body() {
        // min > max: zero iterations; the body is never evaluated, so its
        // undefined variable is not an error.
        let evaluator = eval_ok("for i in 3..1 { x = missing; }\n");
        assert_eq!(evaluator.sketch.len(), 0);
    }

    #[test]
    fn for_body_scope_is_fresh_per_iteration() {
        // x rebinds each iteration without an immutability error, and
        // neither x nor i leaks out of the loop.
        let evaluator = eval_ok("for i in 1..3 { x = 2 i; }\ny = 1;\n");
        assert!(evaluator.lookup("x").is_none());
        assert!(evaluator.lookup("i").is_none());
    }

    #[test]
    fn for_over_list_literal_and_list_binding() {
        let evaluator = eval_ok(
            "pts = [(0, 0), (10, 10)];\n\
             for p in pts { draw(p -- p shifted (5, 0)); }\n\
             for k in [1, 2, 3] { draw((0, k) -- (1, k)); }\n",
        );
        assert_eq!(evaluator.sketch.len(), 5);
    }

    #[test]
    fn nested_for() {
        let evaluator =
            eval_ok("for i in 0..1 { for j in 0..1 { draw((i, j) -- (i + 1, j + 1)); } }\n");
        assert_eq!(evaluator.sketch.len(), 4);
    }

    #[test]
    fn for_over_a_number_is_an_error() {
        let error = eval_err("for i in 5 { x = 1; }\n");
        assert!(error.message.contains("list"), "{}", error.message);
    }

    #[test]
    fn path_queries_on_line_segments() {
        let evaluator = eval_ok(
            "p = (0, 0) -- (100, 0) -- (100, 50);\n\
             a = point(p, 0);\nb = point(p, 1);\nc = point(p, 1.5);\nd = point(p, 2);\n\
             t = direction(p, 1.5);\n\
             q = directionpoint(p, 90);\n",
        );
        assert_point(point(&evaluator, "a"), 0.0, 0.0);
        assert_point(point(&evaluator, "b"), 100.0, 0.0);
        assert_point(point(&evaluator, "c"), 100.0, 25.0);
        assert_point(point(&evaluator, "d"), 100.0, 50.0);
        assert_close(number(&evaluator, "t"), 90.0);
        // First point travelling straight up is the vertical segment's start.
        assert_point(point(&evaluator, "q"), 100.0, 0.0);
    }

    #[test]
    fn directionpoint_on_a_curve_finds_the_tangent_point() {
        // Quarter turn from travelling up to travelling right. The input is
        // symmetric, so the fitted curve is symmetric about the chord's
        // perpendicular bisector (x + y = 1000), and the 45-degree tangent
        // lies exactly on it.
        let evaluator = eval_ok(
            "skel = (0, 500) {up} .. {right} (500, 1000);\n\
             w = directionpoint(skel, 45);\n",
        );
        let w = point(&evaluator, "w");
        assert!((w.x + w.y - 1000.0).abs() < 1e-3, "{w:?}");
        assert!(w.x > 0.0 && w.x < 500.0, "{w:?}");
    }

    #[test]
    fn directionpoint_index_selects_later_matches() {
        let evaluator = eval_ok(
            "p = (0, 0) -- (100, 0) -- (100, 50) -- (200, 50) -- (200, 100);\n\
             a = directionpoint(p, 90);\n\
             b = directionpoint(p, 90, 0);\n\
             c = directionpoint(p, 90, 1);\n",
        );
        assert_point(point(&evaluator, "a"), 100.0, 0.0);
        assert_point(point(&evaluator, "b"), 100.0, 0.0);
        assert_point(point(&evaluator, "c"), 200.0, 50.0);
    }

    #[test]
    fn directionpoint_counts_a_match_on_a_joint_once() {
        // The tangent is `right` exactly on the middle point, which is both
        // the end of the first segment and the start of the second.
        let error = eval_err(
            "skel = (0, 500) {up} .. {right} (500, 1000) .. {up} (1000, 1500);\n\
             w = directionpoint(skel, 0, 1);\n",
        );
        assert!(
            error.message.contains("only 1 point with direction 0"),
            "{}",
            error.message
        );
    }

    #[test]
    fn directionpoint_counts_a_closed_path_start_once() {
        // The circle starts at (100, 0) travelling up, so the start point
        // is also the closing joint's end; a circle has exactly one point
        // per direction.
        let evaluator = eval_ok("c = circle((0, 0), 100);\nw = directionpoint(c, 90);\n");
        assert_point(point(&evaluator, "w"), 100.0, 0.0);
        let error = eval_err("c = circle((0, 0), 100);\nw = directionpoint(c, 90, 1);\n");
        assert!(
            error.message.contains("only 1 point with direction 90"),
            "{}",
            error.message
        );
    }

    #[test]
    fn path_query_errors() {
        let error = eval_err("p = (0, 0) -- (100, 0);\nq = point(p, 1.5);\n");
        assert!(error.message.contains("outside"), "{}", error.message);
        let error = eval_err("p = (0, 0) -- (100, 0);\nq = directionpoint(p, 90);\n");
        assert!(
            error.message.contains("no point with direction"),
            "{}",
            error.message
        );
        let error = eval_err(
            "p = (0, 0) -- (100, 0) -- (100, 50);\nq = directionpoint(p, 90, 1);\n",
        );
        assert!(
            error.message.contains("only 1 point with direction 90"),
            "{}",
            error.message
        );
        let error = eval_err("p = (0, 0) -- (100, 0);\nq = directionpoint(p, 90, 1.5);\n");
        assert!(
            error.message.contains("non-negative integer"),
            "{}",
            error.message
        );
        let error = eval_err("p = (0, 0) -- (100, 0);\nq = directionpoint(p, 90, 1, 2);\n");
        assert!(
            error.message.contains("expects 2 or 3 arguments"),
            "{}",
            error.message
        );
        let error = eval_err("q = point((1, 2), 0.5);\n");
        assert!(
            error.message.contains("expects a path"),
            "{}",
            error.message
        );
    }

    #[test]
    fn circle_pen_gives_constant_width() {
        let evaluator = eval_ok(
            "pen round { width = 10; height = 10; }\n\
             node n = (100, 50) dir 45 pen round;\n\
             w = n.width;\nl = n.left;\nr = n.right;\n",
        );
        assert_close(number(&evaluator, "w"), 10.0);
        let k = 5.0 * f64::sqrt(0.5);
        assert_point(point(&evaluator, "l"), 100.0 - k, 50.0 + k);
        assert_point(point(&evaluator, "r"), 100.0 + k, 50.0 - k);
    }

    #[test]
    fn ellipse_pen_on_axis_edge_points() {
        let evaluator = eval_ok(
            "pen e { width = 10; height = 4; }\n\
             node n = (0, 0) dir right pen e;\n\
             node m = (0, 0) dir up pen e;\n\
             nw = n.width;\nnl = n.left;\n\
             mw = m.width;\nml = m.left;\n",
        );
        // Travelling along the major axis: thickness bottoms out at height.
        assert_close(number(&evaluator, "nw"), 4.0);
        assert_point(point(&evaluator, "nl"), 0.0, 2.0);
        // Travelling across it: full width; the left bank is to the west.
        assert_close(number(&evaluator, "mw"), 10.0);
        assert_point(point(&evaluator, "ml"), -5.0, 0.0);
    }

    #[test]
    fn ellipse_pen_oblique_direction_matches_closed_form() {
        let evaluator = eval_ok(
            "pen e { width = 10; height = 4; }\n\
             node n = (0, 0) dir 45 pen e;\n\
             w = n.width;\nl = n.left;\n",
        );
        // psi = 135 degrees: support = R(0)·(a²cosψ, b²sinψ)/h.
        let (a, b) = (5.0_f64, 2.0_f64);
        let h = ((a * a + b * b) / 2.0).sqrt();
        let ex = -a * a / (h * f64::sqrt(2.0));
        let ey = b * b / (h * f64::sqrt(2.0));
        assert_point(point(&evaluator, "l"), ex, ey);
        assert_close(number(&evaluator, "w"), 2.0 * (ex * ex + ey * ey).sqrt());
    }

    #[test]
    fn razor_pen_matches_width_and_nib() {
        let evaluator = eval_ok(
            "pen razor { width = 10; angle = 30; }\n\
             node p = (0, 0) dir up pen razor;\n\
             node q = (0, 0) dir up width 10 nib 30;\n\
             dl = p.left - q.left;\ndr = p.right - q.right;\n",
        );
        assert_point(point(&evaluator, "dl"), 0.0, 0.0);
        assert_point(point(&evaluator, "dr"), 0.0, 0.0);
    }

    #[test]
    fn pen_tip_and_tail() {
        let evaluator = eval_ok(
            "pen e { width = 10; height = 4; }\n\
             node n = (100, 50) dir right pen e;\n\
             t = n.tip;\nb = n.tail;\nrt = (n rotated 90).tip;\n",
        );
        assert_point(point(&evaluator, "t"), 105.0, 50.0);
        assert_point(point(&evaluator, "b"), 95.0, 50.0);
        // Rotating the node rotates the pen with it.
        assert_point(point(&evaluator, "rt"), -50.0, 105.0);
    }

    #[test]
    fn tip_errors_without_a_round_pen() {
        let error = eval_err("node n = (0, 0) dir up width 10;\nt = n.tip;\n");
        assert!(error.message.contains("pen"), "{}", error.message);
        let error = eval_err("pen r { width = 10; }\nnode n = (0, 0) dir up pen r;\nt = n.tip;\n");
        assert!(error.message.contains("razor"), "{}", error.message);
    }

    #[test]
    fn pen_validation_errors() {
        let error = eval_err("pen p { width = 0; }\n");
        assert!(error.message.contains("width > 0"), "{}", error.message);
        let error = eval_err("pen p { width = 10; height = -1; }\n");
        assert!(error.message.contains("height >= 0"), "{}", error.message);
        let error = eval_err("node n = (0, 0) dir up pen 5;\n");
        assert!(
            error.message.contains("expected a pen"),
            "{}",
            error.message
        );
    }

    #[test]
    fn pen_flows_through_a_def() {
        let evaluator = eval_ok(
            "pen e { width = 10; height = 10; }\n\
             def dot(c, p) -> n { node n = c dir right pen p; }\n\
             n = dot((0, 0), e);\n\
             w = n.width;\n",
        );
        assert_close(number(&evaluator, "w"), 10.0);
    }

    #[test]
    fn path_evaluation() {
        let evaluator =
            eval_ok("a = (0, 0);\nb = (100, 0);\np = a {45} .. b{corner} -- (50, 50) .. cycle;\n");
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
        let evaluator = eval_ok("p = intersection((0, 0) -- (10, 10), (0, 10) -- (10, 0));\n");
        assert_point(point(&evaluator, "p"), 5.0, 5.0);
    }

    /// Radius of a point about the origin.
    fn radius(point: Point) -> f64 {
        point.to_vec2().length()
    }

    fn path(evaluator: &Evaluator, name: &str) -> PathValue {
        match evaluator.lookup(name) {
            Some(Value::Path(path)) => path.clone(),
            other => panic!("{name} is {other:?}"),
        }
    }

    /// The geometry a path renders to. Two paths can describe the same curve
    /// through different constraints — a one-sided tangent pins both sides of
    /// a smooth point, and 360° is 0° — so equivalence is a question about
    /// the fit, not about the source.
    fn fitted(path: &PathValue) -> String {
        crate::render::fit_path(path).expect("fits").to_svg()
    }

    #[test]
    fn circle_matches_the_handwritten_ring() {
        // Literal coordinates, not `r * unitvec(90)`: the constructor puts
        // its points exactly on the axes, where unitvec leaves the residue
        // of cos(90°) behind (see shapes::superellipse_at).
        let evaluator = eval_ok(concat!(
            "r = 280;\n",
            "hand = (r, 0) {up} .. (0, r) {left}",
            " .. (-r, 0) {down} .. (0, -r) {right} .. cycle;\n",
            "made = circle((0, 0), r);\n",
        ));
        assert_eq!(
            fitted(&path(&evaluator, "made")),
            fitted(&path(&evaluator, "hand"))
        );
    }

    #[test]
    fn roundrect_matches_the_handwritten_contour() {
        let evaluator = eval_ok(concat!(
            "k = 156.8;\n",
            "hand = {right} (60 + k, 0) -- (620 - k, 0) {right}\n",
            "    .. {up}    (620, k) -- (620, 560 - k) {up}\n",
            "    .. {left}  (620 - k, 560) -- (60 + k, 560) {left}\n",
            "    .. {down}  (60, 560 - k) -- (60, k) {down}\n",
            "    .. cycle;\n",
            "made = roundrect((60, 0), (620, 560), k);\n",
        ));
        assert_eq!(
            fitted(&path(&evaluator, "made")),
            fitted(&path(&evaluator, "hand"))
        );
    }

    #[test]
    fn exponent_two_is_the_ellipse() {
        let evaluator = eval_ok(concat!(
            "a = ellipse((10, 20), 300, 200);\n",
            "b = superellipse((10, 20), 300, 200, 2, 8);\n",
        ));
        assert_eq!(
            fitted(&path(&evaluator, "a")),
            fitted(&path(&evaluator, "b"))
        );
    }

    #[test]
    fn superellipse_rejects_an_exponent_below_two() {
        let error = eval_err("s = superellipse((0, 0), 100, 100, 1, 8);\n");
        assert!(error.message.contains("at least 2"), "{}", error.message);
    }

    #[test]
    fn superellipse_rejects_a_sample_count_off_the_axes() {
        let error = eval_err("s = superellipse((0, 0), 100, 100, 4, 6);\n");
        assert!(error.message.contains("multiple of 4"), "{}", error.message);
    }

    #[test]
    fn intersection_follows_the_fitted_curve_not_the_chord() {
        // A four-point circle of radius 100 about the origin, cut by the
        // diagonal. The answer sits on the curve at 45°: radius 100, give
        // or take the fitter's 0.027 %. The chord between the on-curve
        // points would have answered (50, 50) — radius 70.7.
        let evaluator = eval_ok(concat!(
            "c = (100, 0) {up} .. (0, 100) {left}",
            " .. (-100, 0) {down} .. (0, -100) {right} .. cycle;\n",
            "d = (0, 0) -- (200, 200);\n",
            "p = intersection(c, d);\n",
        ));
        let p = point(&evaluator, "p");
        assert!((radius(p) - 100.0).abs() < 0.05, "radius {}", radius(p));
        assert!((p.x - p.y).abs() < 1e-6, "off the diagonal: {p:?}");
    }

    #[test]
    fn intersection_of_two_curves() {
        // Two unit-radius circles a radius apart meet at x = 50,
        // y = ±86.6. Neither segment is straight, so this is the
        // subdivision path.
        let evaluator = eval_ok(concat!(
            "a = (100, 0) {up} .. (0, 100) {left}",
            " .. (-100, 0) {down} .. (0, -100) {right} .. cycle;\n",
            "b = (200, 0) {up} .. (100, 100) {left}",
            " .. (0, 0) {down} .. (100, -100) {right} .. cycle;\n",
            "p = intersection(a, b);\n",
        ));
        let p = point(&evaluator, "p");
        assert!((radius(p) - 100.0).abs() < 0.05, "not on a: {p:?}");
        assert!(
            (radius(p - Vec2::new(100.0, 0.0)) - 100.0).abs() < 0.05,
            "not on b: {p:?}"
        );
        assert!((p.x - 50.0).abs() < 0.05, "{p:?}");
        assert!((p.y - 86.6025403784).abs() < 0.05, "{p:?}");
    }

    #[test]
    fn paths_that_do_not_meet_are_an_error() {
        let error = eval_err("p = intersection((0, 0) -- (1, 0), (0, 5) -- (1, 5));\n");
        assert!(
            error.message.contains("do not intersect"),
            "{}",
            error.message
        );
    }

    #[test]
    fn shadowing_note_scopes_are_created_by_blocks_only() {
        // Same-scope rebinding fails even when the first binding came from
        // the prelude-shadowing top level.
        let evaluator = eval_ok("left = 5;\n");
        assert_close(number(&evaluator, "left"), 5.0);
    }

    // ---- Pen terminals -----------------------------------------------------

    const CAP_PEN: &str = "pen main { width = 100; height = 30; angle = 20; }\n\
         node n = (10, 5) dir 40 pen main;\n\
         r = n.right;\nl = n.left;\ntip = n.tip;\ntail = n.tail;\n\
         front = n.tipcap;\nback = n.tailcap;\n";

    #[test]
    fn tip_cap_runs_from_right_contact_over_tip_to_left_contact() {
        let evaluator = eval_ok(CAP_PEN);
        for (name, from, over, to) in [("front", "r", "tip", "l"), ("back", "l", "tail", "r")] {
            let cap = path(&evaluator, name);
            let first = cap.points.first().expect("has points").pos;
            let last = cap.points.last().expect("has points").pos;
            assert!((first - point(&evaluator, from)).length() < 1e-9, "{name} start");
            assert!((last - point(&evaluator, to)).length() < 1e-9, "{name} end");
            let center = point(&evaluator, over);
            assert!(
                cap.points.iter().any(|p| (p.pos - center).length() < 1e-9),
                "{name} passes through {over}"
            );
            assert!(!cap.closed);
            assert!(
                cap.joints
                    .iter()
                    .all(|joint| matches!(joint, PathJoint::Fixed { .. })),
                "{name} is fixed geometry"
            );
        }
    }

    #[test]
    fn cap_boundary_tangents_follow_the_travel_direction() {
        let evaluator = eval_ok(CAP_PEN);
        let front = path(&evaluator, "front");
        let start = front.points.first().unwrap();
        let end = front.points.last().unwrap();
        let normalize = |angle: f64| angle.rem_euclid(360.0);
        assert_close(normalize(start.enter.expect("pinned")), 40.0);
        assert_close(normalize(end.leave.expect("pinned")), 220.0);
    }

    #[test]
    fn cap_cubics_stay_on_the_pen_boundary_at_high_eccentricity() {
        // The case the spline fit gets wrong: a near-razor pen. Map every
        // fixed cubic back into unit-circle space; the error bound there
        // is inherited from the quarter-circle approximation.
        let evaluator = eval_ok(
            "pen blade { width = 150; height = 2; angle = 9; }\n\
             node n = (0, 0) dir 50 pen blade;\n\
             cap = n.tipcap;\n",
        );
        use kurbo::ParamCurve;
        let cap = path(&evaluator, "cap");
        let (sin, cos) = 9.0_f64.to_radians().sin_cos();
        let unmap = |p: Point| {
            let local = Vec2::new(cos * p.x + sin * p.y, -sin * p.x + cos * p.y);
            Vec2::new(local.x / 75.0, local.y / 1.0)
        };
        for (index, joint) in cap.joints.iter().enumerate() {
            let PathJoint::Fixed { c1, c2 } = joint else {
                panic!("cap joint is fixed");
            };
            let cubic = kurbo::CubicBez::new(
                cap.points[index].pos,
                *c1,
                *c2,
                cap.points[index + 1].pos,
            );
            for step in 0..=20 {
                let sample = cubic.eval(step as f64 / 20.0);
                let radius = unmap(sample).length();
                assert!(
                    (radius - 1.0).abs() < 1e-3,
                    "segment {index} leaves the ellipse: radius {radius}"
                );
            }
        }
    }

    #[test]
    fn caps_splice_into_paths() {
        // The caps' boundary points ARE the bank ends, so the banks are
        // just the written joints between the two splices.
        let evaluator = eval_ok(
            "pen main { width = 100; height = 30; }\n\
             node s = (0, 0) dir up pen main;\n\
             node e = (0, 200) dir up pen main;\n\
             tc = s.tailcap;\ntip = e.tipcap;\n\
             stroke = s.tailcap .. e.tipcap .. cycle;\n",
        );
        let stroke = path(&evaluator, "stroke");
        let (tail_cap, tip_cap) = (path(&evaluator, "tc"), path(&evaluator, "tip"));
        assert!(stroke.closed);
        assert_eq!(
            stroke.points.len(),
            tail_cap.points.len() + tip_cap.points.len()
        );
        // The caps' fixed joints, plus the two written bank joints.
        assert_eq!(
            stroke.joints.len(),
            tail_cap.joints.len() + tip_cap.joints.len() + 2
        );
        let fixed = stroke
            .joints
            .iter()
            .filter(|joint| matches!(joint, PathJoint::Fixed { .. }))
            .count();
        assert_eq!(fixed, tail_cap.joints.len() + tip_cap.joints.len());
        crate::render::fit_path(&stroke).expect("mixed path fits");
    }

    #[test]
    fn transforms_carry_fixed_joints_along() {
        let evaluator = eval_ok(&format!(
            "{CAP_PEN}moved = front shifted (7, -3);\nback2 = front reversed;\n"
        ));
        let front = path(&evaluator, "front");
        let moved = path(&evaluator, "moved");
        let (PathJoint::Fixed { c1: a, .. }, PathJoint::Fixed { c1: b, .. }) =
            (&front.joints[0], &moved.joints[0])
        else {
            panic!("fixed joints survive transforms");
        };
        assert!((*b - (*a + Vec2::new(7.0, -3.0))).length() < 1e-9);

        // Reversal swaps each cubic's controls and reverses joint order.
        let reversed = path(&evaluator, "back2");
        let (PathJoint::Fixed { c1, c2 }, PathJoint::Fixed { c1: rc1, c2: rc2 }) =
            (front.joints.first().unwrap(), reversed.joints.last().unwrap())
        else {
            panic!("fixed joints survive reversal");
        };
        assert!((*rc1 - *c2).length() < 1e-9);
        assert!((*rc2 - *c1).length() < 1e-9);
    }

    #[test]
    fn cap_needs_a_pen_with_height() {
        let error = eval_err("node m = (0, 0) dir up width 10;\ncap = m.tipcap;\n");
        assert!(error.message.contains("pen"), "{}", error.message);
    }

    // ---- Joints --------------------------------------------------------

    const JOINT_PEN: &str = "pen main { width = 100; height = 30; angle = 20; }\n\
         node a = (10, 5) dir -55 pen main;\n\
         node b = (10, 5) dir 70 pen main;\n\
         ra = a.right;\nrb = b.right;\nla = a.left;\nlb = b.left;\n";

    #[test]
    fn joint_cap_left_turn_runs_between_right_contacts() {
        // -55 to 70 degrees: a 125-degree left turn; the outer corner is
        // on the right bank, traversed with the stroke.
        let evaluator = eval_ok(&format!("{JOINT_PEN}arc = jointcap(a, b);\n"));
        let arc = path(&evaluator, "arc");
        let first = arc.points.first().expect("has points");
        let last = arc.points.last().expect("has points");
        assert!((first.pos - point(&evaluator, "ra")).length() < 1e-9, "start");
        assert!((last.pos - point(&evaluator, "rb")).length() < 1e-9, "end");
        let normalize = |angle: f64| angle.rem_euclid(360.0);
        assert_close(normalize(first.enter.expect("pinned")), 305.0);
        assert_close(normalize(last.leave.expect("pinned")), 70.0);
        assert!(!arc.closed);
        assert!(
            arc.joints
                .iter()
                .all(|joint| matches!(joint, PathJoint::Fixed { .. })),
            "fixed geometry"
        );
    }

    #[test]
    fn joint_cap_right_turn_runs_between_left_contacts_in_outline_order() {
        // 70 to -55 degrees: a right turn; the outer corner is on the
        // left bank, traversed against the stroke — from the leaving
        // node's contact back to the arriving node's.
        let evaluator = eval_ok(&format!("{JOINT_PEN}arc = jointcap(b, a);\n"));
        let arc = path(&evaluator, "arc");
        let first = arc.points.first().expect("has points");
        let last = arc.points.last().expect("has points");
        assert!((first.pos - point(&evaluator, "la")).length() < 1e-9, "start");
        assert!((last.pos - point(&evaluator, "lb")).length() < 1e-9, "end");
        let normalize = |angle: f64| angle.rem_euclid(360.0);
        assert_close(normalize(first.enter.expect("pinned")), 125.0);
        assert_close(normalize(last.leave.expect("pinned")), 250.0);
    }

    #[test]
    fn joint_cap_cubics_stay_on_the_pen_boundary_at_high_eccentricity() {
        // Same check as the cap version: the analytic arc must hold at
        // eccentricities where a spline fit collapses.
        let evaluator = eval_ok(
            "pen blade { width = 150; height = 2; angle = 9; }\n\
             node a = (0, 0) dir 50 pen blade;\n\
             node b = (0, 0) dir 155 pen blade;\n\
             arc = jointcap(a, b);\n",
        );
        use kurbo::ParamCurve;
        let arc = path(&evaluator, "arc");
        let (sin, cos) = 9.0_f64.to_radians().sin_cos();
        let unmap = |p: Point| {
            let local = Vec2::new(cos * p.x + sin * p.y, -sin * p.x + cos * p.y);
            Vec2::new(local.x / 75.0, local.y / 1.0)
        };
        for (index, joint) in arc.joints.iter().enumerate() {
            let PathJoint::Fixed { c1, c2 } = joint else {
                panic!("arc joint is fixed");
            };
            let cubic = kurbo::CubicBez::new(
                arc.points[index].pos,
                *c1,
                *c2,
                arc.points[index + 1].pos,
            );
            for step in 0..=20 {
                let sample = cubic.eval(step as f64 / 20.0);
                let radius = unmap(sample).length();
                assert!(
                    (radius - 1.0).abs() < 1e-3,
                    "segment {index} leaves the ellipse: radius {radius}"
                );
            }
        }
    }

    #[test]
    fn joint_cap_rejects_degenerate_pairs() {
        let head = "pen main { width = 100; height = 30; }\n\
             node a = (0, 0) dir up pen main;\n";
        for (tail, expected) in [
            ("node b = (0, 0) dir up pen main;\n", "needs a turn"),
            ("node b = (0, 0) dir down pen main;\n", "180"),
            ("node b = (5, 0) dir right pen main;\n", "same position"),
            (
                "pen other { width = 60; height = 20; }\n\
                 node b = (0, 0) dir right pen other;\n",
                "same pen",
            ),
            ("node b = (0, 0) dir right width 10;\n", "pen nodes"),
        ] {
            let error = eval_err(&format!("{head}{tail}arc = jointcap(a, b);\n"));
            assert!(error.message.contains(expected), "{}", error.message);
        }
    }

    #[test]
    fn spliced_paths_reject_closed_paths_and_constraints() {
        let error = eval_err(
            "ring = (0, 0) .. (10, 0) .. (5, 8) .. cycle;\n\
             p = (0, -20) .. ring .. (0, 20);\n",
        );
        assert!(error.message.contains("closed"), "{}", error.message);

        let error = eval_err(
            "arc = (0, 0) {up} .. (10, 10);\n\
             p = (0, -20) .. {up} arc .. (0, 20);\n",
        );
        assert!(
            error.message.contains("direction constraints"),
            "{}",
            error.message
        );
    }
}
