use syn::visit::Visit;
use syn::{BinOp, Expr, ExprBinary, ExprPath, ExprUnary, UnOp, ExprLit, Lit, Member};

/// Visitor that translates Rust expressions into SMT-LIB 2.0 format.
pub struct InvariantVisitor {
    /// The accumulated SMT-LIB strings (RPN-style or S-expressions).
    pub smt_output: String,
    /// Errors encountered during traversal (e.g., unsupported operators).
    pub errors: Vec<String>,
}

impl InvariantVisitor {
    pub fn new() -> Self {
        Self {
            smt_output: String::new(),
            errors: Vec::new(),
        }
    }

    /// Helper to visit a sub-expression and return its SMT string
    fn visit_subexpr(&mut self, expr: &Expr) -> String {
        let mut sub_visitor = InvariantVisitor::new();
        sub_visitor.visit_expr(expr);
        if !sub_visitor.errors.is_empty() {
            self.errors.extend(sub_visitor.errors);
            return String::new(); // Return empty on error, will be propagated
        }
        sub_visitor.smt_output
    }
}

impl<'ast> Visit<'ast> for InvariantVisitor {
    fn visit_expr_binary(&mut self, node: &'ast ExprBinary) {
        let left = self.visit_subexpr(&node.left);
        let right = self.visit_subexpr(&node.right);

        let op_str = match node.op {
            BinOp::Eq(_) => "=",
            BinOp::Ne(_) => "distinct", // SMT-LIB distinct
            BinOp::Lt(_) => "<",
            BinOp::Le(_) => "<=",
            BinOp::Gt(_) => ">",
            BinOp::Ge(_) => ">=",
            BinOp::Add(_) => "+",
            BinOp::Sub(_) => "-",
            BinOp::Mul(_) => "*",
            BinOp::Div(_) => "/", // Integer division in SMT usually 'div'
            BinOp::Rem(_) => "mod",
            BinOp::And(_) => "and",
            BinOp::Or(_) => "or",
            _ => {
                self.errors.push(format!("Unsupported binary operator: {:?}", node.op));
                return;
            }
        };

        self.smt_output = format!("({} {} {})", op_str, left, right);
    }

    fn visit_expr_unary(&mut self, node: &'ast ExprUnary) {
        let operand = self.visit_subexpr(&node.expr);
        let op_str = match node.op {
            UnOp::Not(_) => "not",
            UnOp::Neg(_) => "-",
            _ => {
                self.errors.push(format!("Unsupported unary operator: {:?}", node.op));
                return;
            }
        };
        self.smt_output = format!("({} {})", op_str, operand);
    }

    fn visit_expr_path(&mut self, node: &'ast ExprPath) {
        // Handle identifiers (variables)
        if let Some(ident) = node.path.get_ident() {
             self.smt_output = ident.to_string();
        } else {
             // Basic support for simple paths if needed, or error
             self.errors.push(format!("Complex paths not supported: {:?}", node.path));
        }
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        // Handle self.field
        if let Expr::Path(path) = &*node.base {
             if path.path.is_ident("self") {
                 if let Member::Named(ident) = &node.member {
                     self.smt_output = ident.to_string();
                     return;
                 }
             }
        }
        self.errors.push("Only self.field access is supported".to_string());
    }

    fn visit_expr_lit(&mut self, node: &'ast ExprLit) {
        match &node.lit {
            Lit::Int(i) => self.smt_output = i.to_string(),
            Lit::Bool(b) => self.smt_output = b.value.to_string(),
            _ => self.errors.push(format!("Unsupported literal: {:?}", node.lit)),
        }
    }
    
    fn visit_expr(&mut self, node: &'ast Expr) {
        match node {
            Expr::Binary(e) => self.visit_expr_binary(e),
            Expr::Unary(e) => self.visit_expr_unary(e),
            Expr::Path(e) => self.visit_expr_path(e),
            Expr::Field(e) => self.visit_expr_field(e),
            Expr::Lit(e) => self.visit_expr_lit(e),
            Expr::Paren(e) => self.visit_expr(&e.expr),
            _ => self.errors.push(format!("Unsupported expression type: {:?}", node)),
        }
    }
}
