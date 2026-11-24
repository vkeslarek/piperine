//! `resolve` of the present drives (resolved-nets RN-03, part II §4 rule 8, abi.md §9 D1).
//!
//! A resolver body reads each drive net through a pin of the drive layout: the driven type `X` and a plain `driving`
//! byte. The read of such a carrier binds two variables, the value (`d0`) and whether it is present (`d0_driving_`).
//! `Expr::Resolve` gathers the value of each drive whose byte is 1, in drive order and but `except`, into `[X; k]`, and
//! calls the instance of the nature's `resolve` of that count (`by_count[k]`); an absent drive is left out, a present
//! one of any value is passed.

use piperine_ir::{ExprId, FunctionId, ValueType};

use crate::Error;
use crate::print::value::{Piece, Prec};
use crate::print::{AnalogPrinter, ValuePrinter};

impl ValuePrinter<'_> {
    /// The variable holding whether the drive net read by carrier `c` of `body` holds a drive present.
    pub(crate) fn driving_name(body: piperine_ir::Body<'_>, c: u32) -> String {
        AnalogPrinter::carrier_part(body, c, "driving")
    }

    /// The expression of `Expr::Resolve`: a block that gathers the present drives and calls the instance of their count.
    pub(super) fn resolve_piece(&self, e: ExprId, by_count: &[FunctionId], drives: &[u32], except: Option<u32>) -> Result<Piece, Error> {
        let Some(reads) = self.analog.as_ref() else { return Err(self.unsupported(e, "a resolve of drives")) };
        let mut gathered: Vec<(String, String)> = Vec::new();
        for (k, carrier) in drives.iter().enumerate() {
            if except == Some(k as u32) {
                continue;
            }
            let (Some(value), Some(driving)) = (reads.carriers.get(*carrier as usize), reads.driving.get(*carrier as usize)) else {
                return Err(self.unsupported(e, "a drive of a resolve outside the body's carriers"));
            };
            gathered.push((self.read(value), self.read(driving)));
        }
        let Some(first) = by_count.first() else { return Err(self.unsupported(e, "a resolve with no instance of its function")) };
        let Some(ValueType::Array(driven, _)) = self.ir.function(*first).params().first().map(|p| &p.ty) else {
            return Err(self.unsupported(e, "an instance of resolve that takes no array"));
        };
        let driven = self.type_of(driven)?;
        if gathered.is_empty() {
            // no drive to gather: the instance of no drive, its empty array typed so that the call fixes its `Real`
            return Ok(Piece::new(self.call_text(*first, vec![format!("[<{driven}>::default(); 0]")]), Prec::Atom));
        }
        let mut text = format!("{{ let mut present_ = [<{driven}>::default(); {}]; let mut count_ = 0usize; ", gathered.len());
        for (value, driving) in &gathered {
            text.push_str(&format!("if {driving} {{ present_[count_] = {value}; count_ += 1; }} "));
        }
        text.push_str("match count_ { ");
        for (count, function) in by_count.iter().enumerate().take(gathered.len() + 1) {
            let elements: Vec<String> = (0..count).map(|k| format!("present_[{k}]")).collect();
            text.push_str(&format!("{count} => {}, ", self.call_text(*function, vec![format!("[{}]", elements.join(", "))])));
        }
        text.push_str("_ => unreachable!(\"more drives present than drives\") } }");
        Ok(Piece::new(text, Prec::Atom))
    }
}
