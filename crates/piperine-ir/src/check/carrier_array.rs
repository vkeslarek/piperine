//! The invariants of arrays of carriers and of nodes (composite-values CV-23 to CV-26): an element form names an
//! array carrier, its index is an integer, a constant index lies below the count, a whole read is typed as the
//! array of its elements' values, an element event names an element below the count, and an array of nodes names
//! consecutive body nodes `name[k]`.

use super::{Checker, Walk};
use crate::ids::ExprId;
use crate::model::{BodyData, Expr, Origin, ValueType};

impl Checker<'_> {
    /// An element read of carrier `carrier` at `index`: `form` names the read in the message, `ty` is the type the
    /// expression records and `want_bool` says it is `x[k].changed()` (a `bool`) rather than a value of the element.
    pub(super) fn element_read(&mut self, w: &Walk, origin: &Origin, form: &str, carrier: u32, index: ExprId, ty: &ValueType, want_bool: bool) {
        if !w.owner.in_body {
            self.wrong(w, origin.clone(), format!("{form} outside a body"));
            return;
        }
        let Some(element) = w.owner.carriers.get(carrier as usize) else {
            self.wrong(w, origin.clone(), format!("{form} of carrier {carrier}, out of range ({} carriers)", w.owner.carriers.len()));
            return;
        };
        let Some(count) = w.owner.counts.get(carrier as usize).copied().flatten() else {
            self.wrong(w, origin.clone(), format!("{form} of carrier {carrier}, which is no array of carriers"));
            return;
        };
        let want = if want_bool { ValueType::Bool } else { element.clone() };
        self.own(w, origin, ty, &want);
        self.index_in_range(w, origin, form, index, count);
    }

    /// `index` is an integer expression, and when it is a constant it is below `count` (CV-25 AC 5 owns the run-time
    /// case).
    pub(super) fn index_in_range(&mut self, w: &Walk, origin: &Origin, form: &str, index: ExprId, count: u64) {
        if let Some(got) = self.ty(index) {
            if !got.is_integer() {
                self.wrong(w, origin.clone(), format!("the index of {form} must be an integer, found {got}"));
            }
        }
        if let Some(Expr::Int(n)) = self.ir.exprs.get(index.index()).map(|d| &d.kind) {
            if *n < 0 || *n as u128 >= count as u128 {
                self.wrong(w, origin.clone(), format!("the constant index {n} of {form} is out of range for an array of {count}"));
            }
        }
    }

    /// The element `element` of an element event names an array carrier of the body, below its count.
    pub(super) fn element_event(&mut self, item: &str, origin: &Origin, body: &BodyData, carrier: u32, element: u64) {
        match body.carriers.get(carrier as usize) {
            None => self.report(item.to_string(), origin.clone(), format!("element event of carrier {carrier}, out of range ({} carriers)", body.carriers.len())),
            Some(c) => match c.count {
                None => self.report(item.to_string(), origin.clone(), format!("element event of carrier {}, which is no array of carriers", c.name)),
                Some(count) if element >= count => {
                    self.report(item.to_string(), origin.clone(), format!("element event {element} of carrier {}, out of range for an array of {count}", c.name))
                }
                Some(_) => {}
            },
        }
    }

    /// The arrays of nodes of `body`: each names `count` consecutive nodes, `name[0]` to `name[count - 1]`, and a
    /// carrier's `count` is never zero.
    pub(super) fn carrier_arrays(&mut self, item: &str, body: &BodyData) {
        for array in &body.node_arrays {
            let end = array.first as u64 + array.count as u64;
            if array.count == 0 || end > body.nodes.len() as u64 {
                self.report(item.to_string(), array.origin.clone(), format!("array of nodes {} holds nodes {}..{end}, out of range ({} nodes)", array.name, array.first, body.nodes.len()));
                continue;
            }
            for k in 0..array.count {
                let node = &body.nodes[(array.first + k) as usize];
                let named = format!("{}[{k}]", array.name);
                if node.name != named {
                    self.report(item.to_string(), array.origin.clone(), format!("node {} of the array {} is named {}, not {named}", array.first + k, array.name, node.name));
                }
            }
        }
        for carrier in &body.carriers {
            if carrier.count == Some(0) {
                self.report(format!("{item} carrier {}", carrier.name), carrier.origin.clone(), format!("carrier {} is an array of no element", carrier.name));
            }
        }
    }
}
