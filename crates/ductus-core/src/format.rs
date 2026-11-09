//! Format Ductus source into one canonical layout.
//!
//! The formatter walks the tree-sitter CST, not the typed AST: lowering
//! drops comments, number spellings, implicit multiplication and grouping
//! parentheses, and a formatter must keep all of them.

use crate::parse::{self, SyntaxError};
use tree_sitter::Node;

/// A statement whose path makes it longer than this breaks at every joint.
const MAX_WIDTH: usize = 80;
/// One block level.
const INDENT: &str = "  ";
/// Extra indent for the joint lines of a broken path.
const PATH_INDENT: &str = "    ";

/// Format `source` into the canonical layout. Source with syntax errors
/// is refused: formatting an ERROR tree can destroy code.
pub fn format(source: &str) -> Result<String, SyntaxError> {
    let tree = parse::parse(source)?;
    let formatter = Formatter { source };
    let mut out = formatter.body(tree.root_node(), &[], 0, true);
    if !out.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

struct Formatter<'a> {
    source: &'a str,
}

/// A comment that starts on the last line of the previous node stays on
/// that line; every other comment gets its own line.
fn is_trailing(previous: Option<Node>, comment: Node) -> bool {
    previous.is_some_and(|node| node.end_position().row == comment.start_position().row)
}

/// Top-level blocks are separated by a forced blank line.
fn is_block(kind: &str) -> bool {
    matches!(
        kind,
        "params_block" | "metrics_block" | "def_declaration" | "glyph_block"
    )
}

/// True for a comment that sits directly above a block (through more
/// comments, without a blank line): it belongs to that block.
fn leads_a_block(node: Node) -> bool {
    if node.kind() != "comment" {
        return false;
    }
    let mut current = node;
    while let Some(next) = current.next_named_sibling() {
        if next.start_position().row > current.end_position().row + 1 {
            return false;
        }
        if next.kind() != "comment" {
            return is_block(next.kind());
        }
        current = next;
    }
    false
}

impl<'a> Formatter<'a> {
    fn text(&self, node: Node) -> &'a str {
        node.utf8_text(self.source.as_bytes())
            .expect("source is valid UTF-8")
    }

    /// All children: named nodes, comments and anonymous tokens.
    fn children<'t>(&self, node: Node<'t>) -> Vec<Node<'t>> {
        let mut cursor = node.walk();
        node.children(&mut cursor).collect()
    }

    /// Named children without comments — the grammatical parts.
    fn parts<'t>(&self, node: Node<'t>) -> Vec<Node<'t>> {
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .filter(|child| child.kind() != "comment")
            .collect()
    }

    /// The parse succeeded, so every grammatical field is present; a miss
    /// is a formatter bug.
    fn field<'t>(&self, node: Node<'t>, name: &str) -> Node<'t> {
        node.child_by_field_name(name)
            .unwrap_or_else(|| panic!("{} has a '{name}' field", node.kind()))
    }

    // ---- Statement sequences ---------------------------------------------

    /// The statements of `container` (the file, or a block without its
    /// braces and header fields), one per line at `indent`. Comments and
    /// single blank lines are kept; at the top level a blank line is
    /// forced around every block.
    fn body(&self, container: Node, skip: &[Node], indent: usize, top_level: bool) -> String {
        let pad = INDENT.repeat(indent);
        let mut out = String::new();
        let mut previous: Option<Node> = None;
        for child in self.children(container) {
            if !child.is_named() || skip.contains(&child) {
                continue;
            }
            if child.kind() == "comment" && is_trailing(previous, child) {
                out.push(' ');
                out.push_str(self.text(child).trim_end());
                previous = Some(child);
                continue;
            }
            if let Some(previous) = previous {
                out.push('\n');
                if self.wants_blank_line(previous, child, top_level) {
                    out.push('\n');
                }
            }
            if child.kind() == "comment" {
                out.push_str(&pad);
                out.push_str(self.text(child).trim_end());
            } else {
                out.push_str(&self.statement(child, indent));
            }
            previous = Some(child);
        }
        out
    }

    fn wants_blank_line(&self, previous: Node, current: Node, top_level: bool) -> bool {
        if current.start_position().row > previous.end_position().row + 1 {
            return true;
        }
        if !top_level {
            return false;
        }
        if is_block(previous.kind()) {
            return true;
        }
        // A comment directly above a block belongs to it: the forced blank
        // goes above the comment, not between comment and block.
        (is_block(current.kind()) || leads_a_block(current)) && previous.kind() != "comment"
    }

    /// One statement, fully padded, without a trailing newline. Blocks and
    /// broken paths span several lines.
    fn statement(&self, node: Node, indent: usize) -> String {
        let pad = INDENT.repeat(indent);
        match node.kind() {
            "assignment" => self.binding(node, "", &pad),
            "contour_declaration" => self.binding(node, "contour ", &pad),
            "guide_declaration" => self.binding(node, "guide ", &pad),
            "anchor_declaration" => {
                let name = self.text(self.field(node, "name"));
                let value = self.expr(self.field(node, "value"));
                self.with_strays(node, None, &pad, format!("{pad}anchor {name} = {value};"))
            }
            "param_declaration" => {
                let name = self.text(self.field(node, "name"));
                let default = self.expr(self.field(node, "default"));
                let range = self.expr(self.field(node, "range"));
                self.with_strays(
                    node,
                    None,
                    &pad,
                    format!("{pad}{name} = {default} in {range};"),
                )
            }
            "node_declaration" => self.node_declaration(node, &pad),
            "destructuring_assignment" => {
                let names: Vec<&str> = self
                    .parts(self.field(node, "pattern"))
                    .into_iter()
                    .map(|name| self.text(name))
                    .collect();
                let value = self.expr(self.field(node, "value"));
                let text = format!("{pad}({}) = {value};", names.join(", "));
                self.with_strays(node, None, &pad, text)
            }
            "call_statement" => {
                let call = self.parts(node)[0];
                self.with_strays(node, None, &pad, format!("{pad}{};", self.expr(call)))
            }
            "import_statement" => {
                let path = self.text(self.field(node, "path"));
                self.with_strays(node, None, &pad, format!("{pad}import {path};"))
            }
            "for_statement" => {
                let name = self.text(self.field(node, "name"));
                let iterable = self.value(self.field(node, "iterable"));
                let header = format!("{pad}for {name} in {iterable} {{");
                let text = self.braced(header, self.field(node, "body"), &[], indent);
                self.with_strays(node, None, &pad, text)
            }
            "params_block" => self.braced(format!("{pad}params {{"), node, &[], indent),
            "metrics_block" => self.braced(format!("{pad}metrics {{"), node, &[], indent),
            "def_declaration" => self.def_declaration(node, indent),
            "glyph_block" => self.glyph_block(node, indent),
            // Unknown statement kinds stay as written.
            _ => format!("{pad}{}", self.text(node)),
        }
    }

    /// `[keyword ]name = value;` — assignments and contour/guide
    /// declarations. A path value may break at its joints.
    fn binding(&self, node: Node, keyword: &str, pad: &str) -> String {
        let name = self.text(self.field(node, "name"));
        let value = self.field(node, "value");
        let head = format!("{pad}{keyword}{name} = ");
        if value.kind() == "path" {
            let text = self.path_statement(&head, value, pad);
            return self.with_strays(node, Some(value), pad, text);
        }
        let text = format!("{head}{};", self.value(value));
        self.with_strays(node, None, pad, text)
    }

    fn node_declaration(&self, node: Node, pad: &str) -> String {
        let name = self.text(self.field(node, "name"));
        let position = self.expr(self.field(node, "position"));
        let mut out = format!("{pad}node {name} = {position}");
        // Clause keywords equal their grammar field names.
        for clause in ["dir", "pen"] {
            if let Some(clause_node) = node.child_by_field_name(clause) {
                let value = self.expr(self.parts(clause_node)[0]);
                out.push_str(&format!(" {clause} {value}"));
            }
        }
        out.push(';');
        self.with_strays(node, None, pad, out)
    }

    fn def_declaration(&self, node: Node, indent: usize) -> String {
        let pad = INDENT.repeat(indent);
        let name = self.text(self.field(node, "name"));
        let parameters: Vec<&str> = self
            .parts(self.field(node, "parameters"))
            .into_iter()
            .map(|parameter| self.text(parameter))
            .collect();
        let returns_node = self.field(node, "returns");
        let returns: Vec<&str> = self
            .parts(returns_node)
            .into_iter()
            .map(|name| self.text(name))
            .collect();
        // `-> n` and `-> (n)` parse to the same named nodes; keep the
        // author's parentheses.
        let parenthesized = self.children(returns_node)[0].kind() == "(";
        let returns = if parenthesized {
            format!("({})", returns.join(", "))
        } else {
            returns.join(", ")
        };
        let header = format!("{pad}def {name}({}) -> {returns} {{", parameters.join(", "));
        let text = self.braced(header, self.field(node, "body"), &[], indent);
        self.with_strays(node, None, &pad, text)
    }

    fn glyph_block(&self, node: Node, indent: usize) -> String {
        let pad = INDENT.repeat(indent);
        let name = self.field(node, "name");
        let mut skip = vec![name];
        let mut header = format!("{pad}glyph {}", self.text(name));
        if let Some(unicode) = node.child_by_field_name("unicode") {
            header.push_str(&format!(" : {}", self.text(unicode)));
            skip.push(unicode);
        }
        header.push_str(" {");
        self.braced(header, node, &skip, indent)
    }

    /// `header` (ending in `{`), the container's statements one level
    /// deeper, and the closing brace. An empty body prints as `{}`.
    fn braced(&self, header: String, container: Node, skip: &[Node], indent: usize) -> String {
        let body = self.body(container, skip, indent + 1, false);
        if body.is_empty() {
            format!("{header}}}")
        } else {
            format!("{header}\n{body}\n{}}}", INDENT.repeat(indent))
        }
    }

    // ---- Stray comments ----------------------------------------------------

    /// Comments buried where no printer visits them — inside an expression,
    /// a clause, or a call argument — hoisted to their own lines above the
    /// statement so they are never dropped. Direct children of `kept_path`
    /// are excluded: the path printer keeps those in place. Blocks are not
    /// entered; their statements handle their own comments.
    fn with_strays(&self, node: Node, kept_path: Option<Node>, pad: &str, text: String) -> String {
        let mut strays = Vec::new();
        self.collect_strays(node, kept_path, &mut strays);
        if strays.is_empty() {
            return text;
        }
        let mut out = String::new();
        for comment in strays {
            out.push_str(pad);
            out.push_str(self.text(comment).trim_end());
            out.push('\n');
        }
        out.push_str(&text);
        out
    }

    fn collect_strays<'t>(
        &self,
        node: Node<'t>,
        kept_path: Option<Node<'t>>,
        out: &mut Vec<Node<'t>>,
    ) {
        for child in self.children(node) {
            if child.kind() == "comment" {
                if Some(node) != kept_path {
                    out.push(child);
                }
            } else if child.kind() != "block" {
                self.collect_strays(child, kept_path, out);
            }
        }
    }

    // ---- Paths -------------------------------------------------------------

    /// A path statement prints on one line when it fits in `MAX_WIDTH`
    /// columns and holds no comments; otherwise it breaks at every joint,
    /// each joint leading a line indented `PATH_INDENT` past the statement.
    fn path_statement(&self, head: &str, node: Node, pad: &str) -> String {
        let children = self.children(node);
        let has_comments = children.iter().any(|child| child.kind() == "comment");
        let inline = self.path_inline(&children);
        if !has_comments && head.len() + inline.len() + 1 <= MAX_WIDTH {
            return format!("{head}{inline};");
        }

        let continuation = format!("{pad}{PATH_INDENT}");
        let mut lines: Vec<String> = Vec::new();
        let mut joint: Option<&str> = None;
        let mut previous: Option<Node> = None;
        for child in &children {
            match child.kind() {
                "line_joint" => joint = Some("--"),
                "curve_joint" => joint = Some(".."),
                "comment" => {
                    let text = self.text(*child).trim_end();
                    match lines.last_mut() {
                        Some(last) if is_trailing(previous, *child) => {
                            last.push(' ');
                            last.push_str(text);
                        }
                        _ => lines.push(format!("{continuation}{text}")),
                    }
                }
                _ => {
                    let element = self.path_element(*child);
                    match joint.take() {
                        None => lines.push(format!("{head}{element}")),
                        Some(operator) => lines.push(format!("{continuation}{operator} {element}")),
                    }
                }
            }
            previous = Some(*child);
        }
        let last = lines.last_mut().expect("a path has segments");
        last.push(';');
        lines.join("\n")
    }

    fn path_inline(&self, children: &[Node]) -> String {
        let mut out = String::new();
        for child in children {
            match child.kind() {
                "line_joint" | "curve_joint" => {
                    out.push(' ');
                    out.push_str(self.text(*child));
                    out.push(' ');
                }
                "comment" => {}
                _ => out.push_str(&self.path_element(*child)),
            }
        }
        out
    }

    /// A segment — `{enter} point {leave}` with optional specs — or `cycle`.
    fn path_element(&self, node: Node) -> String {
        if node.kind() == "cycle" {
            return "cycle".to_string();
        }
        let mut out = String::new();
        if let Some(enter) = node.child_by_field_name("enter") {
            out.push_str(&self.direction_spec(enter));
            out.push(' ');
        }
        out.push_str(&self.expr(self.field(node, "point")));
        if let Some(leave) = node.child_by_field_name("leave") {
            out.push(' ');
            out.push_str(&self.direction_spec(leave));
        }
        out
    }

    fn direction_spec(&self, node: Node) -> String {
        let inner = self.parts(node)[0];
        format!("{{{}}}", self.expr(inner))
    }

    // ---- Expressions -------------------------------------------------------

    /// A binding value or call argument: expression, list, or inline path.
    fn value(&self, node: Node) -> String {
        match node.kind() {
            "path" => self.path_inline(&self.children(node)),
            _ => self.expr(node),
        }
    }

    fn expr(&self, node: Node) -> String {
        match node.kind() {
            "binary_expression" => format!(
                "{} {} {}",
                self.expr(self.field(node, "left")),
                self.text(self.field(node, "operator")),
                self.expr(self.field(node, "right"))
            ),
            "unary_expression" => {
                let operand = self.expr(self.field(node, "operand"));
                match self.text(self.field(node, "operator")) {
                    "not" => format!("not {operand}"),
                    operator => format!("{operator}{operand}"),
                }
            }
            "implicit_multiplication" => format!(
                "{}{}",
                self.text(self.field(node, "left")),
                self.expr(self.field(node, "right"))
            ),
            "mediation" => format!(
                "{}[{}, {}]",
                self.expr(self.field(node, "t")),
                self.expr(self.field(node, "a")),
                self.expr(self.field(node, "b"))
            ),
            "member_expression" => format!(
                "{}.{}",
                self.expr(self.field(node, "object")),
                self.text(self.field(node, "property"))
            ),
            "transform_expression" => {
                let operand = self.expr(self.field(node, "operand"));
                let transform = self.text(self.field(node, "transform"));
                match node.child_by_field_name("argument") {
                    Some(argument) => {
                        format!("{operand} {transform} {}", self.expr(argument))
                    }
                    None => format!("{operand} {transform}"),
                }
            }
            "call_expression" => {
                let function = self.text(self.field(node, "function"));
                let arguments = match node.child_by_field_name("arguments") {
                    Some(arguments) => self
                        .parts(arguments)
                        .into_iter()
                        .map(|argument| self.value(argument))
                        .collect::<Vec<_>>()
                        .join(", "),
                    None => String::new(),
                };
                format!("{function}({arguments})")
            }
            "parenthesized_expression" => format!("({})", self.expr(self.parts(node)[0])),
            "point" => format!(
                "({}, {})",
                self.expr(self.field(node, "x")),
                self.expr(self.field(node, "y"))
            ),
            "conditional_expression" => format!(
                "{} if {} else {}",
                self.expr(self.field(node, "value")),
                self.expr(self.field(node, "condition")),
                self.expr(self.field(node, "alternative"))
            ),
            "range" => format!(
                "{}..{}",
                self.expr(self.field(node, "min")),
                self.expr(self.field(node, "max"))
            ),
            "list" => format!(
                "[{}]",
                self.parts(node)
                    .into_iter()
                    .map(|element| self.expr(element))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            // number, identifier, string, unicode, corner — and any future
            // kind, which stays as written rather than being mangled.
            _ => self.text(node).to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formatted(source: &str) -> String {
        format(source).expect("source should format")
    }

    #[test]
    fn refuses_source_with_syntax_errors() {
        assert!(format("u = ;\n").is_err());
    }

    #[test]
    fn spaces_operators_and_assignments() {
        assert_eq!(formatted("u=1+2*3;"), "u = 1 + 2 * 3;\n");
        assert_eq!(formatted("v=a<=b and c^2;"), "v = a <= b and c ^ 2;\n");
    }

    #[test]
    fn keeps_implicit_multiplication_tight() {
        assert_eq!(formatted("x = 4 pen_width;"), "x = 4pen_width;\n");
        assert_eq!(formatted("y = 0.67(u, u);"), "y = 0.67(u, u);\n");
    }

    #[test]
    fn unary_mediation_member_and_conditional() {
        assert_eq!(formatted("y=-x;"), "y = -x;\n");
        assert_eq!(formatted("m=t [ a,b ];"), "m = t[a, b];\n");
        assert_eq!(formatted("p=a . pos . x;"), "p = a.pos.x;\n");
        assert_eq!(formatted("c=a if x<2 else b;"), "c = a if x < 2 else b;\n");
        assert_eq!(formatted("q=p shifted(1,2);"), "q = p shifted (1, 2);\n");
        assert_eq!(formatted("r=p reversed;"), "r = p reversed;\n");
        assert_eq!(formatted("l=[1,2, 3];"), "l = [1, 2, 3];\n");
    }

    #[test]
    fn params_block_with_tight_range() {
        assert_eq!(
            formatted("params{pen_angle=30 in 30..45;}"),
            "params {\n  pen_angle = 30 in 30..45;\n}\n"
        );
    }

    #[test]
    fn glyph_header_and_node_declaration() {
        assert_eq!(
            formatted("glyph o:U+006F{node m=(x/2,0) dir left pen p;}"),
            "glyph o : U+006F {\n  node m = (x / 2, 0) dir left pen p;\n}\n"
        );
    }

    #[test]
    fn def_and_destructuring() {
        assert_eq!(
            formatted("def f(a,b)->(n,s){node n=a dir up pen pen(w);}\n(n,s)=f(1,2);"),
            "def f(a, b) -> (n, s) {\n  node n = a dir up pen pen(w);\n}\n\n(n, s) = f(1, 2);\n"
        );
    }

    #[test]
    fn for_statement_and_import() {
        assert_eq!(
            formatted("import \"m.ductus\";\nfor i in 0..3{draw(p);}"),
            "import \"m.ductus\";\nfor i in 0..3 {\n  draw(p);\n}\n"
        );
    }

    #[test]
    fn short_path_stays_inline() {
        assert_eq!(
            formatted("glyph o{guide skel=m.pos..n.pos..o.pos..p.pos..cycle;}"),
            "glyph o {\n  guide skel = m.pos .. n.pos .. o.pos .. p.pos .. cycle;\n}\n"
        );
    }

    #[test]
    fn long_path_breaks_at_every_joint() {
        let source = "glyph a{contour c1={w1.dir}w1.right..o.right{o.dir+180}..n.right--m.right..w2.right{w2.dir}--cycle;}";
        assert_eq!(
            formatted(source),
            "glyph a {\n\
             \x20 contour c1 = {w1.dir} w1.right\n\
             \x20     .. o.right {o.dir + 180}\n\
             \x20     .. n.right\n\
             \x20     -- m.right\n\
             \x20     .. w2.right {w2.dir}\n\
             \x20     -- cycle;\n\
             }\n"
        );
    }

    #[test]
    fn path_with_a_comment_breaks_and_keeps_it() {
        let source = "glyph a{contour c=a.right\n#..w1.right\n..b.right--cycle;}";
        assert_eq!(
            formatted(source),
            "glyph a {\n\
             \x20 contour c = a.right\n\
             \x20     #..w1.right\n\
             \x20     .. b.right\n\
             \x20     -- cycle;\n\
             }\n"
        );
    }

    #[test]
    fn keeps_own_line_and_trailing_comments() {
        assert_eq!(
            formatted("# leading\nu=100; # trailing\n"),
            "# leading\nu = 100; # trailing\n"
        );
    }

    #[test]
    fn collapses_blank_lines_to_one() {
        assert_eq!(formatted("u=1;\n\n\n\nv=2;\n"), "u = 1;\n\nv = 2;\n");
    }

    #[test]
    fn forces_one_blank_line_between_top_level_blocks() {
        assert_eq!(
            formatted("metrics{u=1;}\nglyph o{advance=1;}"),
            "metrics {\n  u = 1;\n}\n\nglyph o {\n  advance = 1;\n}\n"
        );
    }

    #[test]
    fn comment_above_a_block_stays_attached() {
        assert_eq!(
            formatted("u=1;\n# the o\nglyph o{advance=1;}"),
            "u = 1;\n\n# the o\nglyph o {\n  advance = 1;\n}\n"
        );
    }

    #[test]
    fn empty_source_and_empty_block() {
        assert_eq!(formatted(""), "");
        assert_eq!(formatted("params{}"), "params {}\n");
    }

    #[test]
    fn formatting_is_idempotent_on_a_sample() {
        let source = "glyph a{contour c1={w1.dir}w1.right..o.right{o.dir+180}..n.right--m.right..w2.right{w2.dir}--cycle;}";
        let once = formatted(source);
        assert_eq!(formatted(&once), once);
    }
}
