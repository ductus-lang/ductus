//! Typed AST for Ductus, produced by lowering the tree-sitter CST.

/// 1-based source position of the construct, for error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Params(Vec<ParamDecl>),
    Metrics(Vec<Binding>),
    Def(DefDecl),
    Glyph(GlyphDecl),
    Import { path: String, span: Span },
    Stmt(Stmt),
}

#[derive(Debug, Clone)]
pub struct ParamDecl {
    pub name: String,
    pub default: Expr,
    pub min: Expr,
    pub max: Expr,
    pub span: Span,
}

/// `name = value;` — used for plain assignments, metrics entries, and
/// contour/guide declarations.
#[derive(Debug, Clone)]
pub struct Binding {
    pub name: String,
    pub value: Rhs,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DefDecl {
    pub name: String,
    pub parameters: Vec<String>,
    pub returns: Vec<String>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct GlyphDecl {
    pub name: String,
    pub unicode: Option<u32>,
    pub body: Vec<GlyphStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum GlyphStmt {
    Stmt(Stmt),
    Contour(Binding),
    Guide(Binding),
    Anchor { name: String, value: Expr, span: Span },
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Assign(Binding),
    Destructure {
        names: Vec<String>,
        value: Expr,
        span: Span,
    },
    Node(NodeDecl),
    Call {
        function: String,
        arguments: Vec<Rhs>,
        span: Span,
    },
    For {
        name: String,
        iterable: Iterable,
        body: Vec<Stmt>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct NodeDecl {
    pub name: String,
    pub position: Expr,
    pub dir: Expr,
    pub pen: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Iterable {
    Range { min: Expr, max: Expr },
    List(Vec<Expr>),
    Expr(Expr),
}

/// A right-hand side: paths and lists are values but not sub-expressions.
#[derive(Debug, Clone)]
pub enum Rhs {
    Expr(Expr),
    Path(PathExpr),
    List(Vec<Expr>),
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Number(f64),
    Ident(String),
    Point {
        x: Box<Expr>,
        y: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Mediation {
        t: Box<Expr>,
        a: Box<Expr>,
        b: Box<Expr>,
    },
    Member {
        object: Box<Expr>,
        property: String,
    },
    Transform {
        operand: Box<Expr>,
        op: TransformOp,
        argument: Option<Box<Expr>>,
    },
    Call {
        function: String,
        arguments: Vec<Rhs>,
    },
    Conditional {
        value: Box<Expr>,
        condition: Box<Expr>,
        alternative: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformOp {
    Shifted,
    Scaled,
    XScaled,
    YScaled,
    Rotated,
    Slanted,
    Reversed,
}

/// A path: `segments` joined by `joints`. When `closed`, the final joint
/// connects the last segment back to the first.
/// Invariant: `joints.len() == segments.len() - 1 + (closed as usize)`.
#[derive(Debug, Clone)]
pub struct PathExpr {
    pub segments: Vec<PathSegment>,
    pub joints: Vec<Joint>,
    pub closed: bool,
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub enter: Option<DirSpec>,
    pub point: Expr,
    pub leave: Option<DirSpec>,
}

#[derive(Debug, Clone)]
pub enum DirSpec {
    Angle(Expr),
    Corner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Joint {
    Line,
    Curve,
}
