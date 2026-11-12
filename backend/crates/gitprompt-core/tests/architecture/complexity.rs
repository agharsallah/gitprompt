//! Cognitive complexity (G. Ann Campbell, `SonarSource`, 2017) of every function, on the syntax tree.
//!
//! Each break in the linear flow costs 1, plus 1 per level it is nested in: `if`, `match`, loops,
//! `let … else`. `else` / `else if` cost 1 (no nesting), a labelled `break`/`continue` 1, and each run
//! of the same boolean operator (`a && b && c || d` is two runs) 1. Closures and async blocks nest
//! without costing. `?` costs nothing, and a macro call is one opaque call: clippy's
//! `cognitive_complexity` counts what macros expand to, which put a function holding two `tracing`
//! events over the limit.

use std::path::{Path, PathBuf};

use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{BinOp, Block, Expr, ImplItem, Item, Local, Signature, TraitItem};

/// One function: where it is, its score and its length in code lines (no blanks, no comments).
#[derive(Debug)]
pub struct Function {
    pub file: PathBuf,
    pub line: usize,
    pub name: String,
    pub complexity: usize,
    pub lines: usize,
}

/// Every function with a body in the file: free functions, methods, default trait methods, and
/// those of inline modules (`mod tests { … }`). A function nested in another is not measured.
pub fn functions(path: &Path) -> Vec<Function> {
    let text = std::fs::read_to_string(path).unwrap();
    let file = syn::parse_file(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let lines: Vec<&str> = text.lines().collect();
    file.items
        .iter()
        .flat_map(bodies)
        .map(|(sig, block)| {
            let start = sig.fn_token.span.start().line;
            let end = block.span().end().line;
            Function {
                file: path.to_path_buf(),
                line: start,
                name: sig.ident.to_string(),
                complexity: score(block),
                lines: code_lines(lines.get(start.saturating_sub(1)..end).unwrap_or_default()),
            }
        })
        .collect()
}

/// The functions with a body an item declares, with their signatures.
fn bodies(item: &Item) -> Vec<(&Signature, &Block)> {
    match item {
        Item::Fn(f) => vec![(&f.sig, &*f.block)],
        Item::Impl(imp) => imp.items.iter().filter_map(|i| if let ImplItem::Fn(f) = i { Some((&f.sig, &f.block)) } else { None }).collect(),
        Item::Trait(t) => {
            t.items.iter().filter_map(|i| if let TraitItem::Fn(f) = i { Some((&f.sig, f.default.as_ref()?)) } else { None }).collect()
        }
        Item::Mod(m) => m.content.iter().flat_map(|(_, items)| items).flat_map(bodies).collect(),
        _ => Vec::new(),
    }
}

fn code_lines(lines: &[&str]) -> usize {
    lines.iter().map(|line| line.trim()).filter(|line| !line.is_empty() && !line.starts_with("//")).count()
}

/// The cognitive complexity of a function body.
pub fn score(block: &Block) -> usize {
    let mut scorer = Scorer::default();
    scorer.visit_block(block);
    scorer.score
}

#[derive(Default)]
struct Scorer {
    score: usize,
    nesting: usize,
}

impl Scorer {
    /// A structure that breaks the flow and nests what it holds.
    fn structure(&mut self) {
        self.score += 1 + self.nesting;
    }

    fn nested(&mut self, f: impl FnOnce(&mut Self)) {
        self.nesting += 1;
        f(self);
        self.nesting -= 1;
    }

    /// `if` (`is_else_if`: reached through `else`, which already paid without nesting).
    fn branch(&mut self, expr: &syn::ExprIf, is_else_if: bool) {
        if is_else_if {
            self.score += 1;
        } else {
            self.structure();
        }
        self.visit_expr(&expr.cond);
        self.nested(|s| s.visit_block(&expr.then_branch));
        if let Some((_, otherwise)) = &expr.else_branch {
            if let Expr::If(next) = otherwise.as_ref() {
                self.branch(next, true);
            } else {
                self.score += 1;
                self.nested(|s| s.visit_expr(otherwise));
            }
        }
    }
}

/// `a && b || c` → its operands and operators, left to right, through parentheses.
fn flatten<'a>(expr: &'a Expr, operands: &mut Vec<&'a Expr>, operators: &mut Vec<bool>) {
    match expr {
        Expr::Binary(b) if matches!(b.op, BinOp::And(_) | BinOp::Or(_)) => {
            flatten(&b.left, operands, operators);
            operators.push(matches!(b.op, BinOp::And(_)));
            flatten(&b.right, operands, operators);
        }
        Expr::Paren(p) => flatten(&p.expr, operands, operators),
        _ => operands.push(expr),
    }
}

impl<'ast> Visit<'ast> for Scorer {
    fn visit_expr_if(&mut self, expr: &'ast syn::ExprIf) {
        self.branch(expr, false);
    }

    fn visit_expr_match(&mut self, expr: &'ast syn::ExprMatch) {
        self.structure();
        self.visit_expr(&expr.expr);
        self.nested(|s| expr.arms.iter().for_each(|arm| s.visit_arm(arm)));
    }

    fn visit_expr_for_loop(&mut self, expr: &'ast syn::ExprForLoop) {
        self.structure();
        self.visit_expr(&expr.expr);
        self.nested(|s| s.visit_block(&expr.body));
    }

    fn visit_expr_while(&mut self, expr: &'ast syn::ExprWhile) {
        self.structure();
        self.visit_expr(&expr.cond);
        self.nested(|s| s.visit_block(&expr.body));
    }

    fn visit_expr_loop(&mut self, expr: &'ast syn::ExprLoop) {
        self.structure();
        self.nested(|s| s.visit_block(&expr.body));
    }

    fn visit_local(&mut self, local: &'ast Local) {
        let Some(init) = &local.init else { return visit::visit_local(self, local) };
        self.visit_expr(&init.expr);
        if let Some((_, diverge)) = &init.diverge {
            self.structure();
            self.nested(|s| s.visit_expr(diverge));
        }
    }

    fn visit_expr_closure(&mut self, expr: &'ast syn::ExprClosure) {
        self.nested(|s| s.visit_expr(&expr.body));
    }

    fn visit_expr_async(&mut self, expr: &'ast syn::ExprAsync) {
        self.nested(|s| s.visit_block(&expr.block));
    }

    fn visit_expr_break(&mut self, expr: &'ast syn::ExprBreak) {
        self.score += usize::from(expr.label.is_some());
        visit::visit_expr_break(self, expr);
    }

    fn visit_expr_continue(&mut self, expr: &'ast syn::ExprContinue) {
        self.score += usize::from(expr.label.is_some());
    }

    fn visit_expr_binary(&mut self, expr: &'ast syn::ExprBinary) {
        if !matches!(expr.op, BinOp::And(_) | BinOp::Or(_)) {
            return visit::visit_expr_binary(self, expr);
        }
        let (mut operands, mut operators) = (Vec::new(), Vec::new());
        flatten(&expr.left, &mut operands, &mut operators);
        operators.push(matches!(expr.op, BinOp::And(_)));
        flatten(&expr.right, &mut operands, &mut operators);
        self.score += 1 + operators.windows(2).filter(|pair| pair[0] != pair[1]).count();
        for operand in operands {
            self.visit_expr(operand);
        }
    }

    /// A function or type declared inside a body is measured on its own, if at all.
    fn visit_item(&mut self, _: &'ast Item) {}
}

#[cfg(test)]
mod tests {
    use super::score;

    fn of(body: &str) -> usize {
        score(&syn::parse_str(&format!("{{ {body} }}")).unwrap())
    }

    #[test]
    fn straight_code_and_macros_cost_nothing() {
        assert_eq!(of("let x = f()?; tracing::info!(x, \"done\"); match_like!(a => b);"), 0);
    }

    #[test]
    fn branches_cost_more_when_nested() {
        // if (1) + for nested once (2) + if nested twice (3)
        assert_eq!(of("if a { for x in y { if x { g(); } } }"), 6);
    }

    #[test]
    fn else_chains_cost_one_each() {
        assert_eq!(of("if a { } else if b { } else { }"), 3);
    }

    #[test]
    fn a_match_costs_once_whatever_its_arms() {
        assert_eq!(of("match x { 1 => a(), 2 => b(), _ => c() }"), 1);
    }

    #[test]
    fn boolean_runs_cost_one_per_change_of_operator() {
        assert_eq!(of("let ok = a && b && c || d;"), 2);
    }

    #[test]
    fn closures_nest_without_costing() {
        assert_eq!(of("items.iter().for_each(|x| if x { g() });"), 2);
    }

    #[test]
    fn let_else_is_a_branch() {
        assert_eq!(of("let Some(x) = y else { return };"), 1);
    }
}
