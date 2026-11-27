//! The forms of arrays of carriers a reaction or an event function prints (composite-values CV-24, CV-25; abi.md §3).
//!
//! A read of a **constant** element (`mem[2]`, `mem[2].prev()`, `mem[2].changed()`) is a cell naming the element
//! (`Source::Element`), bound like a scalar pin's read: a `T`, or a `bool` for `changed`. Any other read (a run-time
//! index, the whole array) is the one `WHOLE` cell of the pin (`Source::Whole`), bound as an `rt::CarrierArray<T>` over
//! the pin's `ArrayView`: `get`, `previous`, `changed` and `whole`, each checking its index. A commit of an element
//! goes through `Commits::commit_element`, which checks the index against the count and names the carrier (R3); a
//! commit of a field of a scalar carrier, or of a field inside an element, goes through the statement `piperine-plan`
//! declares over exactly those levels (`StatementPlan::range`), so the value is printed as the field's own type.

use piperine_abi::raw::part;
use piperine_ir::{Body, CarrierRole, Expr, ExprId, ValueType};

use crate::Error;
use crate::print::expression::{Piece, Prec};
use crate::print::{Text, ExpressionPrinter};

/// The variables the reads of an array carrier bind, named once for the plan's reads and for the code that uses them.
pub struct ElementNames;

impl ElementNames {
    /// The `CarrierArray` variable of the carrier named `name`.
    pub fn whole(name: &str) -> String {
        Text::generated(&format!("{name}_whole"))
    }

    /// The variable of `part` (`part::VALUE`, `PREVIOUS` or `CHANGED`) of constant element `element` of the carrier
    /// named `name`.
    pub fn element(name: &str, element: u64, part_of: u32) -> String {
        let suffix = match part_of {
            part::PREVIOUS => "_previous",
            part::CHANGED => "_changed",
            _ => "",
        };
        Text::generated(&format!("{name}_element{element}{suffix}"))
    }
}

/// What the expression printer knows of one carrier of its body.
#[derive(Clone, Debug)]
pub struct CarrierFacts {
    /// The carrier's printed name.
    pub name: String,
    /// The MSL path a panic names: the body and the carrier.
    pub path: String,
    /// The carried type of one carrier (one element of an array).
    pub ty: ValueType,
    /// The number of elements of an array carrier.
    pub count: Option<u64>,
    /// Whether the carrier is of a resolved nature: a commit to it writes its drive pin (value, then `driving`).
    pub resolved: bool,
}

impl CarrierFacts {
    /// The facts of every carrier of `body`.
    pub fn of_body(body: Body<'_>) -> Vec<CarrierFacts> {
        body.carriers()
            .iter()
            .map(|c| CarrierFacts { name: Text::ident(&c.name), path: format!("{}.{}", body.name(), c.name), ty: c.ty.clone(), count: c.count, resolved: matches!(c.role, CarrierRole::Resolved { .. }) })
            .collect()
    }
}

/// How an element form reads: its value, its previous value or its `changed` byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ElementRead {
    /// `x[i]`.
    Value,
    /// `x[i].prev()`.
    Previous,
    /// `x[i].changed()`.
    Changed,
}

impl ElementRead {
    /// The `part` of the ABI the read asks for.
    pub(super) fn part(self) -> u32 {
        match self {
            ElementRead::Value => part::VALUE,
            ElementRead::Previous => part::PREVIOUS,
            ElementRead::Changed => part::CHANGED,
        }
    }

    /// The `CarrierArray` method that answers it.
    pub(super) fn method(self) -> &'static str {
        match self {
            ElementRead::Value => "get",
            ElementRead::Previous => "previous",
            ElementRead::Changed => "changed",
        }
    }
}

impl ExpressionPrinter<'_> {
    /// The facts of carrier `carrier`, or the refusal naming the expression `e`.
    fn carrier_facts(&self, e: ExprId, carrier: u32) -> Result<&CarrierFacts, Error> {
        match self.facts.get(carrier as usize) {
            Some(facts) => Ok(facts),
            None => Err(self.unsupported(e, "an element of a carrier out of range")),
        }
    }

    /// The constant an index expression is, when it is an integer literal (what the plan's reads call a constant
    /// element).
    fn constant_element(&self, index: ExprId) -> Option<u64> {
        match self.ir.expr(index) {
            Expr::Int(n) => u64::try_from(*n).ok(),
            _ => None,
        }
    }

    /// `x[i]`, `x[i].prev()` or `x[i].changed()` on carrier `carrier` (CV-24).
    pub(super) fn element_piece(&self, e: ExprId, carrier: u32, index: ExprId, read: ElementRead) -> Result<Piece, Error> {
        let facts = self.carrier_facts(e, carrier)?;
        let variable = match self.constant_element(index) {
            Some(element) => self.read(&ElementNames::element(&facts.name, element, read.part())),
            None => format!("{}.{}({})", self.read(&ElementNames::whole(&facts.name)), read.method(), self.index_value(index)?),
        };
        Ok(match read {
            ElementRead::Changed => Piece::new(variable, Prec::Atom),
            _ => self.carrier_value(e, variable),
        })
    }

    /// `x` read whole, an array of carriers as an array value (CV-24).
    pub(super) fn whole_piece(&self, e: ExprId, carrier: u32) -> Result<Piece, Error> {
        let facts = self.carrier_facts(e, carrier)?;
        let Some(count) = facts.count else {
            return Err(self.unsupported(e, "a whole read of a carrier that is no array"));
        };
        Ok(Piece::new(format!("*{}.whole::<{count}>()", self.read(&ElementNames::whole(&facts.name))), Prec::Unary))
    }
}
