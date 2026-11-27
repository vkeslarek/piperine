//! The forms of the syntax a kind of the object model names: what the checker refuses when it meets an expression, an
//! operator, a statement, a pattern or a type it does not type. Every `match` is exhaustive with no wildcard arm, so a
//! new kind fails to compile until it is classified.

use super::forms::{PatternForm, ValueForm};
use super::RefusedForm;
use crate::pom::{BinOp, DeclKind, ExprKind, PatKind, StmtKind, TypeKind};

impl RefusedForm {
    /// The form of an expression the checker does not type.
    pub fn expression(kind: &ExprKind) -> RefusedForm {
        match kind {
            ExprKind::Lit(_) => ValueForm::LiteralExpression.into(),
            ExprKind::Path(_) => ValueForm::PathExpression.into(),
            ExprKind::Unary { .. } => ValueForm::UnaryExpression.into(),
            ExprKind::Binary { .. } => ValueForm::BinaryExpression.into(),
            ExprKind::Assign { .. } => ValueForm::Assignment.into(),
            ExprKind::Cast { .. } => ValueForm::AsCast.into(),
            ExprKind::Call { .. } => ValueForm::Call.into(),
            ExprKind::MethodCall { .. } => ValueForm::MethodCall.into(),
            ExprKind::Field { .. } => ValueForm::FieldAccess.into(),
            ExprKind::Index { .. } => ValueForm::IndexExpression.into(),
            ExprKind::Range { .. } => ValueForm::RangeExpression.into(),
            ExprKind::Paren(_) => ValueForm::ParenthesizedExpression.into(),
            ExprKind::Tuple(_) => ValueForm::TupleExpression.into(),
            ExprKind::Array(_) => ValueForm::ArrayExpression.into(),
            ExprKind::ArrayRepeat { .. } => ValueForm::ArrayRepeatExpression.into(),
            ExprKind::Struct { .. } => ValueForm::StructExpression.into(),
            ExprKind::Block(_) => ValueForm::BlockExpression.into(),
            ExprKind::If { .. } => ValueForm::IfExpression.into(),
            ExprKind::Let { .. } => ValueForm::IfLetCondition.into(),
            ExprKind::Match { .. } => ValueForm::MatchExpression.into(),
            ExprKind::While { .. } => ValueForm::WhileLoop.into(),
            ExprKind::Loop { .. } => ValueForm::Loop.into(),
            ExprKind::For { .. } => ValueForm::ForLoop.into(),
            ExprKind::Break(_) => ValueForm::Break.into(),
            ExprKind::Continue => ValueForm::Continue.into(),
            ExprKind::Return(_) => ValueForm::Return.into(),
            ExprKind::Closure { .. } => ValueForm::Closure.into(),
            ExprKind::Try(_) => ValueForm::Expression.into(),
            ExprKind::Emit { .. } => ValueForm::EmitStatement.into(),
            ExprKind::IncludeStr(_) => ValueForm::IncludeStr.into(),
            ExprKind::Error => ValueForm::MalformedExpression.into(),
        }
    }

    /// The form of a binary operator the checker does not type.
    pub fn binary_operator(op: BinOp) -> RefusedForm {
        match op {
            BinOp::Add => ValueForm::OperatorAdd.into(),
            BinOp::Sub => ValueForm::OperatorSubtract.into(),
            BinOp::Mul => ValueForm::OperatorMultiply.into(),
            BinOp::Div => ValueForm::OperatorDivide.into(),
            BinOp::Rem => ValueForm::OperatorRemainder.into(),
            BinOp::BitXor => ValueForm::OperatorBitXor.into(),
            BinOp::BitAnd => ValueForm::OperatorBitAnd.into(),
            BinOp::BitOr => ValueForm::OperatorBitOr.into(),
            BinOp::Shl => ValueForm::OperatorShiftLeft.into(),
            BinOp::Shr => ValueForm::OperatorShiftRight.into(),
            BinOp::And => ValueForm::OperatorLogicalAnd.into(),
            BinOp::Or => ValueForm::OperatorLogicalOr.into(),
            BinOp::Eq => ValueForm::OperatorEqual.into(),
            BinOp::Ne => ValueForm::OperatorNotEqual.into(),
            BinOp::Lt => ValueForm::OperatorLess.into(),
            BinOp::Le => ValueForm::OperatorLessOrEqual.into(),
            BinOp::Gt => ValueForm::OperatorGreater.into(),
            BinOp::Ge => ValueForm::OperatorGreaterOrEqual.into(),
        }
    }

    /// The form of a statement the checker does not type.
    pub fn statement(kind: &StmtKind) -> RefusedForm {
        match kind {
            StmtKind::Let { else_: Some(_), .. } => ValueForm::LetElse.into(),
            StmtKind::Let { .. } => ValueForm::LetStatement.into(),
            StmtKind::Expr { .. } => ValueForm::ExpressionStatement.into(),
            StmtKind::Item(_) => ValueForm::ItemInsideBlock.into(),
            StmtKind::Contribution { .. } => ValueForm::Contribution.into(),
            StmtKind::Commit { .. } => ValueForm::Commit.into(),
            StmtKind::Empty => ValueForm::EmptyStatement.into(),
            StmtKind::Signal(_) => ValueForm::SignalStatement.into(),
            StmtKind::Reaction(_) => ValueForm::OnReaction.into(),
        }
    }

    /// The form of a pattern the checker does not type.
    pub fn pattern(kind: &PatKind) -> RefusedForm {
        match kind {
            PatKind::Wild => PatternForm::WildcardPattern.into(),
            PatKind::Rest => PatternForm::RestPattern.into(),
            PatKind::Ident { sub: Some(_), .. } => PatternForm::Pattern.into(),
            PatKind::Ident { .. } => PatternForm::BindingPattern.into(),
            PatKind::Lit(_) => PatternForm::LiteralPattern.into(),
            PatKind::Path(_) => PatternForm::PathPattern.into(),
            PatKind::Tuple(_) => PatternForm::TuplePattern.into(),
            PatKind::TupleStruct { .. } => PatternForm::TupleStructPattern.into(),
            PatKind::Struct { .. } => PatternForm::StructPattern.into(),
            PatKind::Or(_) => PatternForm::OrPattern.into(),
            PatKind::Range { .. } => PatternForm::RangePattern.into(),
            PatKind::Array(_) => PatternForm::ArrayPattern.into(),
        }
    }

    /// The form of a type outside the supported subset.
    pub fn type_form(kind: &TypeKind) -> RefusedForm {
        match kind {
            TypeKind::Path(_) => ValueForm::TypeName.into(),
            TypeKind::Array { .. } => ValueForm::ArrayType.into(),
            TypeKind::Tuple(items) if items.is_empty() => ValueForm::UnitType.into(),
            TypeKind::Tuple(_) => ValueForm::TupleType.into(),
            TypeKind::Flip(_) => ValueForm::FlipType.into(),
            TypeKind::FnPtr { .. } => ValueForm::FunctionType.into(),
            TypeKind::Infer => ValueForm::InferredType.into(),
        }
    }

    /// The form of a declaration the checker does not type where it is reached.
    pub fn item(kind: &DeclKind) -> RefusedForm {
        match kind {
            DeclKind::Namespace(_) => ValueForm::ModItem.into(),
            DeclKind::Use(_) => ValueForm::UseItem.into(),
            DeclKind::Const(_) => ValueForm::ConstItem.into(),
            DeclKind::Fn(_) => ValueForm::FnItem.into(),
            DeclKind::FnParam(_) => ValueForm::FunctionParameter.into(),
            DeclKind::Struct(_) => ValueForm::StructItem.into(),
            DeclKind::Field(_) => ValueForm::Field.into(),
            DeclKind::Enum(_) => ValueForm::EnumItem.into(),
            DeclKind::Variant(_) => ValueForm::EnumVariant.into(),
            DeclKind::Trait(_) => ValueForm::TraitItem.into(),
            DeclKind::Impl(_) => ValueForm::ImplItem.into(),
            DeclKind::GenericParam(_) => ValueForm::GenericParameter.into(),
            DeclKind::Nature(_) => ValueForm::NatureItem.into(),
            DeclKind::Quantity(_) => ValueForm::NatureQuantity.into(),
            DeclKind::Module(_) => ValueForm::ModuleItem.into(),
            DeclKind::Port(_) => ValueForm::ModulePort.into(),
            DeclKind::Param(_) => ValueForm::ModuleParam.into(),
            DeclKind::Signal(_) => ValueForm::SignalItem.into(),
            DeclKind::Let(_) => ValueForm::ModuleBodyLet.into(),
            DeclKind::Binding(_) => PatternForm::BindingPattern.into(),
            DeclKind::Definition(_) => ValueForm::Definition.into(),
            DeclKind::Generation(_) => ValueForm::GenerationForIf.into(),
            DeclKind::Behavior(_) => ValueForm::BehaviorBlock.into(),
            DeclKind::Reaction(_) => ValueForm::OnReaction.into(),
            DeclKind::Error => ValueForm::MalformedItem.into(),
        }
    }

    /// The name of a pattern form.
    pub fn pattern_name(kind: &PatKind) -> &'static str {
        match kind {
            PatKind::Wild => "wildcard pattern",
            PatKind::Rest => "rest pattern",
            PatKind::Ident { sub: Some(_), .. } => "`@` pattern",
            PatKind::Ident { .. } => "binding",
            PatKind::Lit(_) => "literal pattern",
            PatKind::Path(_) => "path pattern",
            PatKind::Tuple(_) => "tuple pattern",
            PatKind::TupleStruct { .. } => "tuple-struct pattern",
            PatKind::Struct { .. } => "struct pattern",
            PatKind::Or(_) => "or-pattern",
            PatKind::Range { .. } => "range pattern",
            PatKind::Array(_) => "array pattern",
        }
    }

    /// The name of a declaration kind where the subset has none to give: a reached item, or a
    /// name that resolves to one.
    pub fn declaration_name(kind: &DeclKind) -> &'static str {
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
