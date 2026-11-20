//! The one place that names a construct outside the M1 subset. Every `match` is exhaustive with
//! no wildcard arm, so a new POM variant fails to compile until it is classified.

use crate::pom::{BinOp, DeclKind, ExprKind, PatKind, StmtKind, TypeKind, UnOp};

pub(crate) struct Unsupported;

impl Unsupported {
    /// The name the specification gives an expression form.
    pub(crate) fn expr(kind: &ExprKind) -> &'static str {
        match kind {
            ExprKind::Lit(_) => "literal",
            ExprKind::Path(_) => "path expression",
            ExprKind::Unary { op: UnOp::Deref, .. } => "dereference `*`",
            ExprKind::Unary { op: UnOp::Ref, .. } => "reference `&`",
            ExprKind::Unary { .. } => "unary expression",
            ExprKind::Binary { .. } => "binary expression",
            ExprKind::Assign { .. } => "assignment",
            ExprKind::Cast { .. } => "`as` cast",
            ExprKind::Call { .. } => "call",
            ExprKind::MethodCall { .. } => "method call",
            ExprKind::Field { .. } => "field access",
            ExprKind::Index { .. } => "index expression",
            ExprKind::Range { .. } => "range expression",
            ExprKind::Paren(_) => "parenthesized expression",
            ExprKind::Tuple(_) => "tuple expression",
            ExprKind::Array(_) => "array expression",
            ExprKind::ArrayRepeat { .. } => "array repeat expression",
            ExprKind::Struct { .. } => "struct expression",
            ExprKind::Block(_) => "block expression",
            ExprKind::If { .. } => "`if` expression",
            ExprKind::Let { .. } => "`if let` condition",
            ExprKind::Match { .. } => "`match` expression",
            ExprKind::While { .. } => "`while` loop",
            ExprKind::Loop { .. } => "`loop`",
            ExprKind::For { .. } => "`for` loop",
            ExprKind::Break(_) => "`break`",
            ExprKind::Continue => "`continue`",
            ExprKind::Return(_) => "`return`",
            ExprKind::Closure { .. } => "closure",
            ExprKind::Try(_) => "`?` expression",
            ExprKind::Emit { .. } => "`emit!` statement",
            ExprKind::IncludeStr(_) => "`include_str!`",
            ExprKind::Error => "malformed expression",
        }
    }

    /// The name of a binary operator the subset does not type.
    pub(super) fn binary(op: BinOp) -> &'static str {
        match op {
            BinOp::Add => "operator `+`",
            BinOp::Sub => "operator `-`",
            BinOp::Mul => "operator `*`",
            BinOp::Div => "operator `/`",
            BinOp::Rem => "operator `%`",
            BinOp::BitXor => "operator `^`",
            BinOp::BitAnd => "operator `&`",
            BinOp::BitOr => "operator `|`",
            BinOp::Shl => "operator `<<`",
            BinOp::Shr => "operator `>>`",
            BinOp::And => "operator `&&`",
            BinOp::Or => "operator `||`",
            BinOp::Eq => "operator `==`",
            BinOp::Ne => "operator `!=`",
            BinOp::Lt => "operator `<`",
            BinOp::Le => "operator `<=`",
            BinOp::Gt => "operator `>`",
            BinOp::Ge => "operator `>=`",
        }
    }

    /// The name of a statement form.
    pub(super) fn stmt(kind: &StmtKind) -> &'static str {
        match kind {
            StmtKind::Let { else_: Some(_), .. } => "`let ... else`",
            StmtKind::Let { .. } => "`let` statement",
            StmtKind::Expr { .. } => "expression statement",
            StmtKind::Item(_) => "item inside a block",
            StmtKind::Contribution { .. } => "contribution",
            StmtKind::Commit { .. } => "commit `<-`",
            StmtKind::Empty => "empty statement",
            StmtKind::Signal(_) => "`signal` statement",
            StmtKind::Reaction(_) => "`on` reaction",
        }
    }

    /// The name of a pattern form.
    pub(super) fn pat(kind: &PatKind) -> &'static str {
        match kind {
            PatKind::Wild => "wildcard pattern",
            PatKind::Rest => "rest pattern",
            PatKind::Ident { mutable: true, .. } => "`let mut` binding",
            PatKind::Ident { by_ref: true, .. } => "`ref` binding",
            PatKind::Ident { sub: Some(_), .. } => "`@` pattern",
            PatKind::Ident { .. } => "binding",
            PatKind::Lit(_) => "literal pattern",
            PatKind::Path(_) => "path pattern",
            PatKind::Tuple(_) => "tuple pattern",
            PatKind::TupleStruct { .. } => "tuple-struct pattern",
            PatKind::Struct { .. } => "struct pattern",
            PatKind::Or(_) => "or-pattern",
            PatKind::Range { .. } => "range pattern",
            PatKind::Ref(_) => "reference pattern",
            PatKind::Slice(_) => "slice pattern",
        }
    }

    /// The name of a type form outside the subset.
    pub(super) fn ty(kind: &TypeKind) -> &'static str {
        match kind {
            TypeKind::Path(_) => "type",
            TypeKind::Array { .. } => "array type",
            TypeKind::Slice(_) => "slice type",
            TypeKind::Tuple(items) if items.is_empty() => "unit type",
            TypeKind::Tuple(_) => "tuple type",
            TypeKind::Ref { .. } => "reference type",
            TypeKind::Flip(_) => "`flip` type",
            TypeKind::FnPtr { .. } => "function type",
            TypeKind::Infer => "inferred type `_`",
        }
    }

    /// The name of a declaration kind where the subset has none to give: a reached item, or a
    /// name that resolves to one.
    pub(super) fn item(kind: &DeclKind) -> &'static str {
        match kind {
            DeclKind::Namespace(_) => "`mod` item",
            DeclKind::Use(_) => "`use` item",
            DeclKind::Const(_) => "`const` item",
            DeclKind::Fn(_) => "`fn` item",
            DeclKind::FnParam(_) => "function parameter",
            DeclKind::Struct(_) => "`struct` item",
            DeclKind::Field(_) => "field",
            DeclKind::Enum(_) => "`enum` item",
            DeclKind::Variant(_) => "`enum` variant",
            DeclKind::Trait(_) => "`trait` item",
            DeclKind::Impl(_) => "`impl` item",
            DeclKind::GenericParam(_) => "generic parameter",
            DeclKind::Nature(_) => "`nature` item",
            DeclKind::Quantity(_) => "nature quantity",
            DeclKind::Module(_) => "`module` item",
            DeclKind::Port(_) => "module port",
            DeclKind::Param(_) => "module param",
            DeclKind::Signal(_) => "`signal` item",
            DeclKind::Let(_) => "module-body `let`",
            DeclKind::Binding(_) => "binding",
            DeclKind::Definition(_) => "definition `=`",
            DeclKind::Generation(_) => "generation `for`/`if`",
            DeclKind::Behavior(_) => "behavior block",
            DeclKind::Reaction(_) => "`on` reaction",
            DeclKind::Error => "malformed item",
        }
    }
}
