//! The invariants of arrays of carriers and of nodes (composite-values CV-23 to CV-26): an element form names an
//! array carrier, its index is an integer, a constant index lies below the count, a whole read is typed as the
//! array of its elements' values, an element event names an element below the count, and an array of nodes names
//! consecutive body nodes `name[k]`.

use super::walk::CodeWalk;
use super::{IdCheck, TypeCheck};
use crate::ids::ExprId;
use crate::model::{BodyData, CommitTarget, Expr, Origin, Projection, TypeKind, ValueType};

impl TypeCheck<'_> {
    /// An element read of carrier `carrier` at `index`: `form` names the read in the message, `ty` is the type the
    /// expression records and `want_bool` says it is `x[k].changed()` (a `bool`) rather than a value of the element.
    pub(super) fn element_read(&mut self, walk: &CodeWalk, origin: &Origin, form: &str, carrier: u32, index: ExprId, ty: &ValueType, want_bool: bool) {
        if !walk.owner.in_body {
            self.wrong(walk, origin.clone(), format!("{form} outside a body"));
            return;
        }
        let Some(element) = walk.owner.carriers.get(carrier as usize) else {
            self.wrong(walk, origin.clone(), format!("{form} of carrier {carrier}, out of range ({} carriers)", walk.owner.carriers.len()));
            return;
        };
        let Some(count) = walk.owner.counts.get(carrier as usize).copied().flatten() else {
            self.wrong(walk, origin.clone(), format!("{form} of carrier {carrier}, which is no array of carriers"));
            return;
        };
        let want = if want_bool { ValueType::Bool } else { element.clone() };
        self.own(walk, origin, ty, &want);
        self.index_in_range(walk, origin, form, index, count);
    }

    /// `index` is an integer expression, and when it is a constant it is below `count` (CV-25 AC 5 owns the run-time
    /// case).
    pub(super) fn index_in_range(&mut self, walk: &CodeWalk, origin: &Origin, form: &str, index: ExprId, count: u64) {
        if let Some(got) = self.reader.type_of(index) {
            if !got.is_integer() {
                self.wrong(walk, origin.clone(), format!("the index of {form} must be an integer, found {got}"));
            }
        }
        if let Some(Expr::Int(n)) = self.reader.ir.exprs.get(index.index()).map(|d| &d.kind) {
            if *n < 0 || *n as u128 >= count as u128 {
                self.wrong(walk, origin.clone(), format!("the constant index {n} of {form} is out of range for an array of {count}"));
            }
        }
    }

    /// The element `element` of an element event names an array carrier of the body, below its count.
    pub(super) fn element_event(&mut self, item: &str, origin: &Origin, body: &BodyData, carrier: u32, element: u64) {
        match body.carriers.get(carrier as usize) {
            None => self.findings.report(item.to_string(), origin.clone(), format!("element event of carrier {carrier}, out of range ({} carriers)", body.carriers.len())),
            Some(c) => match c.count {
                None => self.findings.report(item.to_string(), origin.clone(), format!("element event of carrier {}, which is no array of carriers", c.name)),
                Some(count) if element >= count => {
                    self.findings.report(item.to_string(), origin.clone(), format!("element event {element} of carrier {}, out of range for an array of {count}", c.name))
                }
                Some(_) => {}
            },
        }
    }

    /// The type of the levels `path` names inside a value of type `ty`: a field of a struct or a tuple, an element of
    /// a value array (an integer index, a constant one below the length). `Err` names the first step that fails.
    pub(super) fn projected(&self, ty: &ValueType, path: &[Projection]) -> Result<ValueType, String> {
        let mut at = ty.clone();
        for step in path {
            at = match (step, &at) {
                (Projection::Field(k), ValueType::Tuple(parts)) => {
                    parts.get(*k as usize).cloned().ok_or_else(|| format!("field {k} of a tuple of {}", parts.len()))?
                }
                (Projection::Field(k), ValueType::Struct(id)) => match self.reader.ir.types.get(id.index()).map(|t| &t.kind) {
                    Some(TypeKind::Struct { fields, .. }) => {
                        fields.get(*k as usize).map(|(_, ty)| ty.clone()).ok_or_else(|| format!("field {k} of a struct of {}", fields.len()))?
                    }
                    _ => return Err(format!("field {k} of a type that is no struct")),
                },
                (Projection::Index(index), ValueType::Array(element, length)) => {
                    if self.reader.type_of(*index).is_some_and(|t| !t.is_integer()) {
                        return Err("an index that is no integer".to_string());
                    }
                    if let Some(Expr::Int(n)) = self.reader.ir.exprs.get(index.index()).map(|d| &d.kind) {
                        if *n < 0 || *n as u128 >= *length as u128 {
                            return Err(format!("the constant index {n} of an array of {length}"));
                        }
                    }
                    (**element).clone()
                }
                (step, ty) => return Err(format!("{step:?} into a value of type {ty}")),
            };
        }
        Ok(at)
    }

    /// The type a commit or a definition of carrier `carrier` (holding `ty`) writes through `element` and `path`: the
    /// carrier's whole value, an element of an array carrier, or the levels the projections name inside either. `None`
    /// after reporting a target that does not fit the carrier.
    pub(super) fn written_type(
        &mut self,
        walk: &CodeWalk,
        origin: &Origin,
        carrier: u32,
        ty: &ValueType,
        element: Option<&dyn ElementOf>,
        path: &[Projection],
    ) -> Option<ValueType> {
        let count = walk.owner.counts.get(carrier as usize).copied().flatten();
        let base = match (count, element) {
            (Some(count), Some(element)) => {
                element.check(self, walk, origin, count);
                ty.clone()
            }
            (Some(_), None) => {
                // the ABI commits through an element (abi.md §3): a statement's range is inside one element
                self.wrong(walk, origin.clone(), format!("a whole array of carriers (carrier {carrier}) cannot be committed or defined; name an element"));
                return None;
            }
            (None, Some(_)) => {
                self.wrong(walk, origin.clone(), format!("an element of carrier {carrier}, which is no array of carriers"));
                return None;
            }
            (None, None) => ty.clone(),
        };
        if let Some(Projection::Index(index)) =
            path.iter().find(|step| matches!(step, Projection::Index(i) if !matches!(self.reader.ir.exprs.get(i.index()).map(|d| &d.kind), Some(Expr::Int(_)))))
        {
            self.wrong(walk, origin.clone(), format!("a run-time index {index:?} into the value of carrier {carrier}: a commit's byte range is fixed in the shape"));
            return None;
        }
        match self.projected(&base, path) {
            Ok(written) => Some(written),
            Err(what) => {
                self.wrong(walk, origin.clone(), format!("a target of carrier {carrier} that does not fit it: {what}"));
                None
            }
        }
    }

    /// The target of a commit: its element (an expression) and its steps.
    pub(super) fn commit_type(&mut self, walk: &CodeWalk, origin: &Origin, carrier: u32, ty: &ValueType, target: Option<&CommitTarget>) -> Option<ValueType> {
        match target {
            None => self.written_type(walk, origin, carrier, ty, None, &[]),
            Some(target) => {
                let element = target.element.map(ExpressionElement);
                self.written_type(walk, origin, carrier, ty, element.as_ref().map(|e| e as &dyn ElementOf), &target.path)
            }
        }
    }

    /// The type a definition of carrier `carrier` writes through `element` and `path`.
    pub(super) fn drive_type(&mut self, walk: &CodeWalk, origin: &Origin, carrier: u32, ty: &ValueType, element: Option<u64>, path: &[Projection]) -> Option<ValueType> {
        let element = element.map(ConstantElement);
        self.written_type(walk, origin, carrier, ty, element.as_ref().map(|e| e as &dyn ElementOf), path)
    }
}

impl IdCheck<'_> {
    /// The arrays of nodes of `body`: each names `count` consecutive nodes, `name[0]` to `name[count - 1]`, and a
    /// carrier's `count` is never zero.
    pub(super) fn carrier_arrays(&mut self, item: &str, body: &BodyData) {
        for array in &body.node_arrays {
            let end = array.first as u64 + array.count as u64;
            if array.count == 0 || end > body.nodes.len() as u64 {
                self.findings.report(item.to_string(), array.origin.clone(), format!("array of nodes {} holds nodes {}..{end}, out of range ({} nodes)", array.name, array.first, body.nodes.len()));
                continue;
            }
            for k in 0..array.count {
                let node = &body.nodes[(array.first + k) as usize];
                let named = format!("{}[{k}]", array.name);
                if node.name != named {
                    self.findings.report(item.to_string(), array.origin.clone(), format!("node {} of the array {} is named {}, not {named}", array.first + k, array.name, node.name));
                }
            }
        }
        for carrier in &body.carriers {
            if carrier.count == Some(0) {
                self.findings.report(format!("{item} carrier {}", carrier.name), carrier.origin.clone(), format!("carrier {} is an array of no element", carrier.name));
            }
        }
    }

    /// The windows of arrays of carriers a placement gives its child's carrier ports: parallel to the bindings, each
    /// naming an array carrier of the parent and a run (or one element) inside it.
    pub(super) fn child_windows(&mut self, item: &str, body: &BodyData) {
        for (ci, windows) in body.child_windows.iter().enumerate() {
            // a child given no window (the list is empty) binds whole arrays and scalars only
            if windows.is_empty() {
                continue;
            }
            let Some(child) = body.children.get(ci) else {
                self.findings.report(item.to_string(), body.origin.clone(), format!("windows of child {ci}, out of range ({} children)", body.children.len()));
                continue;
            };
            let bound = body.child_carriers.get(ci).map_or(&[][..], Vec::as_slice);
            if windows.len() != bound.len() {
                self.findings.report(item.to_string(), child.origin.clone(), format!("child {} has {} windows for {} carrier bindings", child.name, windows.len(), bound.len()));
                continue;
            }
            for (port, (window, binding)) in windows.iter().zip(bound).enumerate() {
                let Some(window) = window else { continue };
                let count = binding.and_then(|c| body.carriers.get(c as usize)).and_then(|c| c.count);
                let Some(count) = count else {
                    self.findings.report(item.to_string(), child.origin.clone(), format!("child {}: port {port} has a window of a carrier that is no array of carriers", child.name));
                    continue;
                };
                let length = window.length.unwrap_or(1);
                if length == 0 || window.first.checked_add(length).is_none_or(|end| end > count) {
                    self.findings.report(
                        item.to_string(),
                        child.origin.clone(),
                        format!("child {}: port {port} has the window {}..+{length} of an array of {count}", child.name, window.first),
                    );
                }
            }
        }
    }
}

/// An element of an array carrier a commit or a definition names, checked against the array's count.
pub(super) trait ElementOf {
    /// Reports an element that does not fit an array of `count`.
    fn check(&self, checker: &mut TypeCheck<'_>, walk: &CodeWalk, origin: &Origin, count: u64);
}

/// A commit's element: an integer expression, constant or not.
pub(super) struct ExpressionElement(ExprId);

impl ElementOf for ExpressionElement {
    fn check(&self, checker: &mut TypeCheck<'_>, walk: &CodeWalk, origin: &Origin, count: u64) {
        checker.index_in_range(walk, origin, "a commit's element", self.0, count);
    }
}

/// A definition's element: a constant (a definition's index folds at elaboration).
pub(super) struct ConstantElement(u64);

impl ElementOf for ConstantElement {
    fn check(&self, checker: &mut TypeCheck<'_>, walk: &CodeWalk, origin: &Origin, count: u64) {
        if self.0 >= count {
            checker.wrong(walk, origin.clone(), format!("the element {} of a definition is out of range for an array of {count}", self.0));
        }
    }
}
