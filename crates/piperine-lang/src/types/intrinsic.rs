//! The functions of §22.1, §22.2.15 and §22.6.4 that read the characters of a `str` or a file (`logic`, `matches_bits`, `parse_table`, `parse_hex`,
//! `table_model`), typed by their normative contract at the concrete type; `cast` and `same` are ordinary generic
//! functions of `std/values.msl` since `generics` T55. A call whose callee is the stdlib's own item never reaches
//! `signature_parts`, which refuses a generic function and an `extern` one; a project's own
//! function of the same name is an ordinary function (§22 rule 3).

use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind};

use super::check::Checker;
use super::{Callee, ConstTerm, GenericValue, LogicPattern, Nominal, Ty};

/// A function of §22.1 the checker types by its contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intrinsic {
    /// `parse_table<Row, const N: usize>(text: str) -> [Row; N]`: the type of the `const` being initialized
    /// gives `Row` and `N` (§22.6.4 rule 2, PI-09).
    ParseTable,
    /// `parse_hex<T, const N: usize>(text: str) -> [T; N]`, `T` an integer type (§22.6.4, PI-09).
    ParseHex,
    /// `table_model<const N: usize>(x: f64, table: [(f64, f64); N], control: str) -> f64` (§22.2.15, PI-10).
    TableModel,
    /// `logic<const N: usize>(pattern: str) -> Logic<N>` (§22.1.4): `N` from the type the context expects, else from the
    /// length of a literal pattern.
    Logic,
    /// `matches_bits<const N: usize>(s: Logic<N>, pattern: str) -> bool` (§22.1.4).
    MatchesBits,
}

impl Intrinsic {
    /// The function called `name` in the std namespace `namespace` (`values`, `io`, `analog`), if it is one of them.
    fn named(namespace: &str, name: &str) -> Option<Intrinsic> {
        match (namespace, name) {
            ("io", "parse_table") => Some(Intrinsic::ParseTable),
            ("io", "parse_hex") => Some(Intrinsic::ParseHex),
            ("analog", "table_model") => Some(Intrinsic::TableModel),
            ("values", "logic") => Some(Intrinsic::Logic),
            ("values", "matches_bits") => Some(Intrinsic::MatchesBits),
            _ => None,
        }
    }
}

impl Checker<'_> {
    /// Which §22.1 function `function` is, when it is the stdlib's own (`std/values.msl`).
    pub(super) fn intrinsic_of(&self, function: DeclId) -> Option<Intrinsic> {
        let data = self.design.decl_data(function);
        if !matches!(data.kind, DeclKind::Fn(_)) || !self.is_std(function) {
            return None;
        }
        let parent = data.parent.map(|p| self.design.decl_data(p))?;
        let namespace = matches!(parent.kind, DeclKind::Namespace(_)).then(|| parent.name.as_ref().map(|n| n.text.as_str())).flatten()?;
        Intrinsic::named(namespace, &data.name.as_ref()?.text)
    }

    /// A call of one of the two: its type from the arguments, the turbofish or the type the
    /// context expects, the contract's errors reported (`E0701`, `E0702`), the callee recorded.
    pub(super) fn intrinsic_call(&mut self, expr: ExprId, intrinsic: Intrinsic, args: &[Arg]) -> Option<Ty> {
        let expected = self.call_expected.take().filter(|(at, _)| *at == expr).map(|(_, ty)| ty);
        let result = match intrinsic {
            Intrinsic::ParseTable | Intrinsic::ParseHex => self.parsed_array(expr, intrinsic, args, expected)?,
            Intrinsic::TableModel => self.table_model_call(expr, args)?,
            Intrinsic::Logic => self.logic_call(expr, args, expected)?,
            Intrinsic::MatchesBits => self.matches_bits_call(expr, args)?,
        };
        self.callees.insert(expr, Callee::Intrinsic(intrinsic));
        Some(result)
    }

    /// `parse_table(text)` and `parse_hex(text)`: the text is a `str`, the result the type the `const` being
    /// initialized gives (§22.6.4 rule 2): an array of `f64` or of a tuple of `f64` for a table, of one integer
    /// type for a hex image. `None` after a diagnostic.
    fn parsed_array(&mut self, expr: ExprId, intrinsic: Intrinsic, args: &[Arg], expected: Option<Ty>) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        self.apply(expr, &[Ty::Str], args)?;
        let name = if intrinsic == Intrinsic::ParseTable { "parse_table" } else { "parse_hex" };
        let Some(array) = expected.filter(|ty| matches!(ty, Ty::Array(..))) else {
            self.unsupported(file, span, &format!("`{name}` whose result type is not written (the array type is the `const`'s, §22.6.4 rule 2)"));
            return None;
        };
        let Ty::Array(element, _) = &array else { return None };
        let fits = match (intrinsic, &**element) {
            (Intrinsic::ParseTable, Ty::F64) => true,
            (Intrinsic::ParseTable, Ty::Tuple(parts)) => !parts.is_empty() && parts.iter().all(|part| *part == Ty::F64),
            (_, other) => intrinsic == Intrinsic::ParseHex && other.is_integer(),
        };
        if !fits {
            let wanted = if intrinsic == Intrinsic::ParseTable { "an array of `f64` or of a tuple of `f64`" } else { "an array of one integer type" };
            let message = format!("`{name}` gives {wanted}, not `{}` (§22.6.4)", array.name(self.design));
            self.coded(file, "E0180", span, message, "the array type does not fit");
            return None;
        }
        Some(array)
    }

    /// `table_model(x, table, control)`: `x` an `f64`, `table` an array of `(f64, f64)` pairs of any length and
    /// `control` a `str`; the value is an `f64` (§22.2.15). `None` after a diagnostic.
    fn table_model_call(&mut self, expr: ExprId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let pair = Ty::Tuple(vec![Ty::F64, Ty::F64]);
        let table = match args.get(1).and_then(|arg| self.infer(arg.value)) {
            Some(table @ Ty::Array(..)) => table,
            Some(other) => {
                let span = self.design.expr_node(args[1].value).span;
                self.mismatch_text(file, span, "[(f64, f64); N]", &other.name(self.design));
                return None;
            }
            None => return None,
        };
        if let Ty::Array(element, _) = &table
            && **element != pair
        {
            let span = self.design.expr_node(args[1].value).span;
            self.mismatch_text(file, span, "[(f64, f64); N]", &table.name(self.design));
            return None;
        }
        self.apply(expr, &[Ty::F64, table, Ty::Str], args)?;
        Some(Ty::F64)
    }
}

impl Checker<'_> {
    /// The struct `Logic` of the standard library (`std/logic.msl`), when the design holds it.
    fn library_logic(&self) -> Option<DeclId> {
        (0..self.design.decl_count()).map(|index| DeclId(index as u32)).find(|decl| {
            let data = self.design.decl_data(*decl);
            let in_logic = data.parent.is_some_and(|parent| self.design.decl_data(parent).name.as_ref().is_some_and(|name| name.text == "logic"));
            matches!(data.kind, DeclKind::Struct(_)) && data.name.as_ref().is_some_and(|name| name.text == "Logic") && in_logic && self.is_std(*decl)
        })
    }

    /// The number of levels of a `Logic<N>` type, `None` for any other type.
    pub(super) fn logic_width(&self, ty: &Ty) -> Option<u64> {
        let Ty::Struct(nominal) = ty else { return None };
        if Some(nominal.decl) != self.library_logic() {
            return None;
        }
        match nominal.arguments.first() {
            Some(GenericValue::Const(ConstTerm::Value(width))) => u64::try_from(*width).ok(),
            _ => None,
        }
    }

    /// The text of a string literal argument, `None` for any other expression.
    fn literal_logic_pattern(&self, arg: &Arg) -> Option<String> {
        match &self.design.expr_node(arg.value).kind {
            ExprKind::Lit(lit) => lit.str_value(),
            _ => None,
        }
    }

    /// `E0702` when the literal pattern `text` holds another character than `0`, `1`, `x` and `z`, or a length other than
    /// `width` (§22.1.4 rule 2). The levels it names, `None` after the diagnostic.
    fn checked_pattern(&mut self, arg: &Arg, text: &str, width: Option<u64>) -> Option<usize> {
        let file = self.file_of_expr(arg.value);
        let span = self.design.expr_node(arg.value).span;
        let pattern = match LogicPattern::parse(text) {
            Ok(pattern) => pattern,
            Err(character) => {
                let message = format!("the pattern `{text}` holds `{character}`, which is none of `0`, `1`, `x` and `z` (§22.1.4 rule 2)");
                self.coded(file, "E0702", span, message, "not a level of a pattern");
                return None;
            }
        };
        if let Some(width) = width.filter(|width| *width != pattern.width() as u64) {
            let message = format!("the pattern `{text}` has {} characters, and a `Logic<{width}>` has {width} levels (§22.1.4 rule 2)", pattern.width());
            self.coded(file, "E0702", span, message, "one character per level");
            return None;
        }
        Some(pattern.width())
    }

    /// `logic(pattern)`: a `Logic<N>`, `N` from the expected type or from the length of the literal pattern.
    fn logic_call(&mut self, expr: ExprId, args: &[Arg], expected: Option<Ty>) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        self.apply(expr, &[Ty::Str], args)?;
        let Some(logic) = self.library_logic() else {
            self.unsupported(file, span, "`logic` without the standard library's `Logic`");
            return None;
        };
        let wanted = expected.as_ref().and_then(|ty| self.logic_width(ty));
        let literal = self.literal_logic_pattern(&args[0]);
        let width = match (&literal, wanted) {
            (Some(text), wanted) => {
                let length = self.checked_pattern(&args[0], text, wanted)?;
                wanted.unwrap_or(length as u64)
            }
            (None, Some(wanted)) => wanted,
            (None, None) => {
                self.unsupported(file, span, "`logic` of a pattern that is no literal where no `Logic<N>` is expected");
                return None;
            }
        };
        let width = i128::from(width);
        let nominal = Nominal { decl: logic, arguments: vec![GenericValue::Const(ConstTerm::Value(width))] };
        Some(Ty::Struct(nominal))
    }

    /// `matches_bits(s, pattern)`: `s` is a `Logic<N>`, a literal pattern has `N` characters; a `bool`.
    fn matches_bits_call(&mut self, expr: ExprId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let [subject, pattern] = args else {
            self.apply(expr, &[Ty::Unit, Ty::Str], args)?;
            return None;
        };
        let found = self.infer(subject.value)?;
        let Some(width) = self.logic_width(&found) else {
            let span = self.design.expr_node(subject.value).span;
            self.mismatch_text(file, span, "a `Logic<N>`", &found.name(self.design));
            return None;
        };
        self.apply(expr, &[found, Ty::Str], args)?;
        if let Some(text) = self.literal_logic_pattern(pattern) {
            self.checked_pattern(pattern, &text, Some(width))?;
        }
        Some(Ty::Bool)
    }
}
