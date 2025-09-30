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

impl PathExpression {
    /// Check if this path forms a cycle by comparing first and last points
    pub fn is_cyclic(&self) -> bool {
        if self.segments.len() < 3 {
            return false; // Need at least 3 points to form a meaningful cycle
        }

        let first_point = match self.segments.first() {
            Some(PathSegment::Point(point_ref)) => point_ref,
            _ => return false,
        };

        let last_point = match self.segments.last() {
            Some(PathSegment::Point(point_ref)) => point_ref,
            Some(PathSegment::Connection { to, .. }) => to,
            _ => return false,
        };

        // Check if first and last points are the same
        match (first_point, last_point) {
            (PointRef::Literal { x: x1, y: y1 }, PointRef::Literal { x: x2, y: y2 }) => {
                (x1 - x2).abs() < 1e-6 && (y1 - y2).abs() < 1e-6
            }
            (PointRef::Variable(name1), PointRef::Variable(name2)) => name1 == name2,
            _ => false, // Mixed literal/variable references can't be easily compared here
        }
    }
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
    Curve,
}

impl Display for Connector {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Connector::Line => write!(f, "--"),
            Connector::Curve => write!(f, ".."),
        }
    }
}
