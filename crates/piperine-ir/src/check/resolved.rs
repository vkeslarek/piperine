//! The invariants of resolved nets (resolved-nets RN-03): a `Resolve` form names the resolver body's drive carriers,
//! which hold one driven type `X`, and one instance of the net's `resolve` per count of present drives, each taking
//! `[X; k]` and returning the form's type `V`; a net's `Resolution` names nets and instances of the design.

use super::{Checker, Walk};
use crate::ids::FunctionId;
use crate::model::{Origin, ValueType};

impl Checker<'_> {
    /// The checks of one `Expr::Resolve`: `ty` is the type the expression records.
    pub(super) fn resolve_form(&mut self, w: &Walk, origin: &Origin, ty: &ValueType, by_count: &[FunctionId], drives: &[u32], except: Option<u32>) {
        if !w.owner.in_body {
            self.wrong(w, origin.clone(), "`resolve` outside a body".to_string());
            return;
        }
        if by_count.len() != drives.len() + 1 {
            self.wrong(w, origin.clone(), format!("`resolve` names {} instances of the function for {} drives; it needs one per count 0..={}", by_count.len(), drives.len(), drives.len()));
            return;
        }
        if except.is_some_and(|k| k as usize >= drives.len()) {
            self.wrong(w, origin.clone(), format!("`resolve` leaves out drive {}, out of range ({} drives)", except.unwrap_or_default(), drives.len()));
        }
        let mut driven: Option<&ValueType> = None;
        for &carrier in drives {
            match w.owner.carriers.get(carrier as usize) {
                None => self.wrong(w, origin.clone(), format!("`resolve` reads drive carrier {carrier}, out of range ({} carriers)", w.owner.carriers.len())),
                Some(held) => match driven {
                    Some(first) if first != held => self.wrong(w, origin.clone(), format!("`resolve` drives hold two types, {first} and {held}")),
                    Some(_) => {}
                    None => driven = Some(held),
                },
            }
        }
        for (count, function) in by_count.iter().enumerate() {
            let Some(data) = self.ir.functions.get(function.index()) else { continue };
            let single = data.params.len() == 1;
            let takes_drives = match (data.params.first().map(|p| &p.ty), driven) {
                (Some(ValueType::Array(element, length)), Some(driven)) => single && **element == *driven && *length == count as u64,
                (Some(ValueType::Array(_, length)), None) => single && *length == count as u64,
                _ => false,
            };
            if !takes_drives {
                self.wrong(w, origin.clone(), format!("`resolve` instance {count} (`{}`) must take one `[X; {count}]` of the drives' type", data.name));
            }
            if data.ret != *ty {
                self.wrong(w, origin.clone(), format!("`resolve` instance {count} (`{}`) returns {}, the form is typed {ty}", data.name, data.ret));
            }
        }
    }

    /// The checks of every net's `Resolution`: its resolver and holders are instances of the design, its drive and others
    /// nets are nets of it, the drive nets hold the driven type, and an others net names a drive and holds the value type.
    pub(super) fn resolutions(&mut self) {
        let ir = self.ir;
        for net in &ir.nets {
            let Some(resolution) = &net.resolution else { continue };
            let item = format!("net {}", net.path);
            if resolution.resolver.index() >= ir.instances.len() {
                self.report(item.clone(), net.origin.clone(), format!("resolver instance {} out of range ({} instances)", resolution.resolver.0, ir.instances.len()));
            }
            if resolution.holders.len() != resolution.drives.len() {
                self.report(item.clone(), net.origin.clone(), format!("{} holders for {} drives", resolution.holders.len(), resolution.drives.len()));
            }
            for holder in resolution.holders.iter().flatten() {
                match ir.instances.get(holder.instance.index()) {
                    None => self.report(item.clone(), net.origin.clone(), format!("holder instance {} out of range ({} instances)", holder.instance.0, ir.instances.len())),
                    Some(instance) if holder.carrier as usize >= ir.bodies[instance.body.index()].carriers.len() => {
                        self.report(item.clone(), net.origin.clone(), format!("holder carrier {} out of range for instance {}", holder.carrier, instance.path))
                    }
                    Some(_) => {}
                }
            }
            for drive in &resolution.drives {
                match ir.nets.get(drive.index()) {
                    None => self.report(item.clone(), net.origin.clone(), format!("drive net {} out of range ({} nets)", drive.0, ir.nets.len())),
                    Some(data) if data.ty != resolution.driven => self.report(item.clone(), net.origin.clone(), format!("drive net {} holds {}, the net's driven type is {}", data.path, data.ty, resolution.driven)),
                    Some(_) => {}
                }
            }
            for (drive, other) in &resolution.others {
                if *drive as usize >= resolution.drives.len() {
                    self.report(item.clone(), net.origin.clone(), format!("an others net names drive {drive}, out of range ({} drives)", resolution.drives.len()));
                }
                match ir.nets.get(other.index()) {
                    None => self.report(item.clone(), net.origin.clone(), format!("others net {} out of range ({} nets)", other.0, ir.nets.len())),
                    Some(data) if data.ty != net.ty => self.report(item.clone(), net.origin.clone(), format!("others net {} holds {}, the value type is {}", data.path, data.ty, net.ty)),
                    Some(_) => {}
                }
            }
        }
    }
}
