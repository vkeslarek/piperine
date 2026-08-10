//! The arena `Module` and the `Program` that holds a list of them.
//!
//! Every concept (value, net, cell, block) pushes into one of these arenas
//! and receives back the new id; nothing in the public API uses `usize`.
//! The arena shape is the Cranelift idiom (already a dependency in the
//! wider workspace) — dense `Vec` storage plus distinct newtype indices.

use super::{Const, ConstId, NatureId, Net, NatureInfo, Ty, ValueId, NetId};

/// One IR module: the arena storage for every concept. T2 fills in values
/// and constants; nets and natures arrive in T3; cells (T4) and blocks
/// (T5) attach their own arenas to this struct as they arrive.
#[derive(Debug, Default, Clone)]
pub struct Module {
    /// The type of each value slot, indexed by [`ValueId`]. The arena is
    /// the source of truth for "what is this value's shape" — ops carry
    /// `ValueId`s, never `Ty`s inline.
    values: Vec<Ty>,
    /// The constant pool, indexed by [`ConstId`]. Deduplication is the
    /// caller's concern — the arena only stores; the IR has no global
    /// constant table because each module owns its own.
    consts: Vec<Const>,
    /// The nature table, indexed by [`NatureId`]. A discipline (Electrical,
    /// Thermal, Magnetic, …) names its own natures; the IR carries only
    /// the ids and this table is for diagnostics plus the discipline's own
    /// bookkeeping. The IR never hardcodes `V`/`I`.
    natures: Vec<NatureInfo>,
    /// The declared nets, indexed by [`NetId`].
    nets: Vec<Net>,
}

impl Module {
    /// An empty module — the start of every IR build.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a value slot of type `ty`, returning its id. The id is
    /// opaque — the only operations on it are passing it back to this
    /// module (to read the type) or recording it in an op (T5).
    pub fn push_value(&mut self, ty: Ty) -> ValueId {
        let id = ValueId(self.values.len() as u32);
        self.values.push(ty);
        id
    }

    /// The type of value `id`. The id was produced by [`Self::push_value`];
    /// a `ValueId` not produced by this module is an internal-consistency
    /// violation, and the verifier (T7) catches it before any backend sees
    /// it.
    pub fn value_ty(&self, id: ValueId) -> &Ty {
        &self.values[id.0 as usize]
    }

    /// The total number of value slots — used by passes that pre-allocate
    /// side tables indexed by `ValueId`.
    pub fn value_count(&self) -> usize {
        self.values.len()
    }

    /// All value ids in allocation order — for passes that walk every value
    /// (the verifier, the eventual diff pass).
    pub fn values(&self) -> impl Iterator<Item = (ValueId, &Ty)> {
        self.values
            .iter()
            .enumerate()
            .map(|(i, ty)| (ValueId(i as u32), ty))
    }

    /// Intern a constant, returning its id.
    pub fn push_const(&mut self, c: Const) -> ConstId {
        let id = ConstId(self.consts.len() as u32);
        self.consts.push(c);
        id
    }

    /// The constant at `id`.
    pub fn const_at(&self, id: ConstId) -> &Const {
        &self.consts[id.0 as usize]
    }

    /// The total number of constants.
    pub fn const_count(&self) -> usize {
        self.consts.len()
    }

    /// Intern a nature, returning its id. Natures are tied 1:1 to a
    /// discipline's access name; the IR is name-agnostic, but every
    /// `Node { across, through }` carries two ids that resolve through
    /// this table for diagnostics.
    pub fn push_nature(&mut self, info: NatureInfo) -> NatureId {
        let id = NatureId(self.natures.len() as u32);
        self.natures.push(info);
        id
    }

    /// The nature at `id`.
    pub fn nature_at(&self, id: NatureId) -> &NatureInfo {
        &self.natures[id.0 as usize]
    }

    /// Declare a net, returning its id.
    pub fn push_net(&mut self, net: Net) -> NetId {
        let id = NetId(self.nets.len() as u32);
        self.nets.push(net);
        id
    }

    /// The net at `id`.
    pub fn net_at(&self, id: NetId) -> &Net {
        &self.nets[id.0 as usize]
    }

    /// All net ids in declaration order, for passes that walk every net.
    pub fn nets(&self) -> impl Iterator<Item = (NetId, &Net)> {
        self.nets
            .iter()
            .enumerate()
            .map(|(i, net)| (NetId(i as u32), net))
    }

    /// The total number of nets.
    pub fn net_count(&self) -> usize {
        self.nets.len()
    }
}

/// A whole IR program: one or more modules. The shape mirrors the POM's
/// `Design::modules` (MD-25): the IR is a SEPARATE artifact built FROM the
/// POM, never mutating it.
#[derive(Debug, Default, Clone)]
pub struct Program {
    /// One [`Module`] per POM module, in elaboration order.
    pub modules: Vec<Module>,
}

impl Program {
    /// An empty program.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a module, returning its index in `modules`.
    pub fn push_module(&mut self, module: Module) -> usize {
        let i = self.modules.len();
        self.modules.push(module);
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_module_has_no_values_or_consts() {
        let m = Module::new();
        assert_eq!(m.value_count(), 0);
        assert_eq!(m.const_count(), 0);
    }

    #[test]
    fn push_value_returns_distinct_ids_in_allocation_order() {
        let mut m = Module::new();
        let a = m.push_value(Ty::Real);
        let b = m.push_value(Ty::Bits(8));
        let c = m.push_value(Ty::Real);
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
        // Allocation order is preserved (LOV-07's spirit: ordering is
        // explicit, never implied by traversal).
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn value_ty_round_trips_through_push() {
        let mut m = Module::new();
        let id = m.push_value(Ty::Array(Box::new(Ty::Bits(8)), 4));
        assert_eq!(m.value_ty(id), &Ty::Array(Box::new(Ty::Bits(8)), 4));
    }

    #[test]
    fn values_iterates_in_allocation_order_with_ids() {
        let mut m = Module::new();
        let _ = m.push_value(Ty::Real);
        let _ = m.push_value(Ty::Bits(16));

        let collected: Vec<_> = m.values().collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].0, ValueId(0));
        assert_eq!(collected[0].1, &Ty::Real);
        assert_eq!(collected[1].0, ValueId(1));
        assert_eq!(collected[1].1, &Ty::Bits(16));
    }

    #[test]
    fn push_const_returns_distinct_ids_and_round_trips() {
        let mut m = Module::new();
        let a = m.push_const(Const::Real(1.0));
        let b = m.push_const(Const::Bits(7));
        assert_ne!(a, b);
        assert_eq!(m.const_at(a), &Const::Real(1.0));
        assert_eq!(m.const_at(b), &Const::Bits(7));
    }

    #[test]
    fn program_holds_modules_in_elaboration_order() {
        let mut p = Program::new();
        let mut m0 = Module::new();
        m0.push_value(Ty::Real);
        let mut m1 = Module::new();
        m1.push_value(Ty::Bits(1));
        let i0 = p.push_module(m0);
        let i1 = p.push_module(m1);
        assert_eq!(i0, 0);
        assert_eq!(i1, 1);
        assert_eq!(p.modules.len(), 2);
    }
}
