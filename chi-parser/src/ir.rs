//! Intermediate Representation for the parsed AST

use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
}

impl Display for Program {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        writeln!(f, "Program {{")?;
        for statement in &self.statements {
            writeln!(f, "  {}", statement)?;
        }
        write!(f, "}}")
    }
}

#[derive(Debug, Clone)]
pub enum Statement {
    Assignment {
        name: String,
        expression: Expression,
    },
    FunctionCall {
        name: String,
        args: Vec<String>, // variable names
    },
}

impl Display for Statement {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Statement::Assignment { name, expression } => {
                write!(f, "{} = {};", name, expression)
            }
            Statement::FunctionCall { name, args } => {
                write!(f, "{}({});", name, args.join(", "))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expression {
    Point { x: f64, y: f64 },
    Path(PathExpression),
    Variable(String),
}

impl Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Expression::Point { x, y } => write!(f, "({}, {})", x, y),
            Expression::Path(path) => write!(f, "{}", path),
            Expression::Variable(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PathExpression {
    pub segments: Vec<PathSegment>,
}

impl Display for PathExpression {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "")?;
        for (i, segment) in self.segments.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", segment)?;
        }
        write!(f, "")
    }
}

#[derive(Debug, Clone)]
pub enum PathSegment {
    Point(PointRef),
    Connection { connector: Connector, to: PointRef },
}

impl Display for PathSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            PathSegment::Point(point_ref) => write!(f, "{}", point_ref),
            PathSegment::Connection { connector, to } => {
                write!(f, "{} {}", connector, to)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum PointRef {
    Literal { x: f64, y: f64 },
    Variable(String),
}

impl Display for PointRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            PointRef::Literal { x, y } => write!(f, "({}, {})", x, y),
            PointRef::Variable(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Connector {
    Line,
    Curve {
        start_options: CurveOptions,
        end_options: CurveOptions,
    },
}

impl Display for Connector {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Connector::Line => write!(f, "--"),
            Connector::Curve {
                start_options,
                end_options,
            } => {
                write!(f, "{}..{}", start_options, end_options)
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CurveOptions {
    pub direction: Option<f64>, // angle in degrees
    pub tension: Option<f64>,   // for future extensions
}

impl Display for CurveOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let mut options = Vec::new();

        if let Some(dir) = self.direction {
            options.push(format!("dir:{}", dir));
        }

        if let Some(tension) = self.tension {
            options.push(format!("tension:{}", tension));
        }

        if options.is_empty() {
            write!(f, "")
        } else {
            write!(f, "[{}]", options.join(","))
        }
    }
}
