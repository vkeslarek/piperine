//! `Netlist::circuit`: the IR's instance table as `CircuitBuilder` calls, over whatever supplies
//! the module of each body.

use piperine_abi::raw::Module;
use piperine_ir::{BodyId, Constant, Ir, NodeRef};
use piperine_solver::abi::{Error, SolverDomain};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, ParamValue, Port, Result};

use crate::BodyPlan;
use crate::pin::PinLayout;

/// Who supplies the module of a body: the interpreter, or a compiled library.
pub trait Modules {
    /// The module of `body`; `None` for a body with nothing to run, or an id out of range.
    fn module(&self, body: BodyId) -> Option<&'static Module>;
}

/// The instance table of one design, ready to be bound to the modules of its bodies.
#[derive(Clone, Copy, Debug)]
pub struct Netlist<'a> {
    ir: &'a Ir,
    resolution: Option<f64>,
    microsteps: Option<usize>,
}

impl<'a> Netlist<'a> {
    pub fn new(ir: &'a Ir) -> Netlist<'a> {
        Netlist { ir, resolution: None, microsteps: None }
    }

    /// This netlist with the session's tick length (the `RESOLUTION` global), in seconds; the
    /// solver's own default (one femtosecond) when never given.
    pub fn with_resolution(self, seconds_per_tick: f64) -> Netlist<'a> {
        Netlist { resolution: Some(seconds_per_tick), ..self }
    }

    /// This netlist with the bound on the microsteps of one instant of the digital engine; the
    /// engine's own default (1000) when never given.
    pub fn with_microsteps(self, bound: usize) -> Netlist<'a> {
        Netlist { microsteps: Some(bound), ..self }
    }

    /// The design as a solver circuit: one solver node per design node, in the IR's node order
    /// (so node `i` is `NodeIdentifier::Anonymous(i)`), named by its path to the builder; one
    /// `CircuitBuilder::device` per instance whose body has a module, under the instance's path.
    /// A body node is bound by position to the instance's design node (`NodeRef::Ground` to the
    /// reference). Each param is passed by name, except one that holds its body's default: it
    /// is given no value and its pin keeps the default.
    ///
    /// A carrier that is a `PORT` pin of its shape (an `in` or `out` carrier, or a `signal` a
    /// child's carrier port is bound to: `BodyPlan::carrier_is_port`) is bound, after the node
    /// ports and in the body's order, to the design net the instance gives it (`Port::Net`
    /// named by the net's path, so the net is read by that name); a carrier the instance gives
    /// no net is left open, its pin private to the instance. So a parent's `signal` and its
    /// children's ports are one solver net, and the parent's drives and commits reach the
    /// children (part II §5; `ir-rust` F-B1). A carrier's initial value is its pin's
    /// `HAS_INIT`, in the shape: it reaches the solver with the pin, and a net's own `init` is
    /// the initial value of the carrier that declares it (msl-digital T28), so it reaches the
    /// solver the same way (F-B2).
    ///
    /// Refused by name, as an IR the elaborator never gives: a net whose `init` no carrier of
    /// its declaring instance bound to it holds (dropping it would start the net unknown), and
    /// a `signal` other instances' carriers meet on that its body binds to no child (the
    /// signal's pin is private, so they would never meet).
    pub fn circuit(&self, modules: &impl Modules) -> Result<CircuitInstance> {
        self.refuse_unbound_forms()?;
        let ir = self.ir;
        let mut builder = CircuitBuilder::new(ir.top().path());
        let nodes: Vec<_> = ir.nodes().map(|n| builder.node(n.path())).collect();
        let ground = builder.ground();
        if let Some(seconds) = self.resolution {
            builder.resolution(seconds);
        }
        if let Some(bound) = self.microsteps {
            builder.microsteps(bound);
        }
        for instance in ir.instances() {
            let body = instance.body();
            let Some(module) = modules.module(body.id()) else { continue };
            let mut ports: Vec<Port> = instance
                .bindings()
                .iter()
                .map(|r| match r {
                    NodeRef::Node(id) => Port::Node(nodes[id.index()].clone()),
                    NodeRef::Ground => Port::Node(ground.clone()),
                })
                .collect();
            for c in 0..body.carriers().len() {
                if !BodyPlan::carrier_is_port(body, c as u32) {
                    continue;
                }
                ports.push(match instance.net(c) {
                    Some(net) => Port::Net(net.path().to_string()),
                    None => Port::Open,
                });
            }
            let mut values: Vec<(&str, ParamValue)> = Vec::new();
            for (param, given) in body.params().iter().zip(instance.params()).filter(|(param, given)| param.default.as_ref() != Some(*given)) {
                let value = match given {
                    Constant::Real(r) => ParamValue::Real(r.get()),
                    Constant::Bool(b) => ParamValue::Bool(*b),
                    // The solver's integer leaf; an `N`-bit value fits its 64 bits.
                    Constant::Int(v) => ParamValue::Int(*v as i64),
                    // A composite is exactly the pin's bytes (`CompositeLayout::bytes`).
                    Constant::Aggregate(_) => ParamValue::Bytes(PinLayout::bytes_in(ir, &param.ty, *given).ok_or_else(|| {
                        Error::simple(SolverDomain::Device, format!("instance `{}`: param `{}` holds a composite constant that is no value of {}", instance.path(), param.name, param.ty))
                    })?),
                };
                values.push((param.name.as_str(), value));
            }
            builder.device(module, instance.path(), &values, &ports);
        }
        builder.build()
    }

    /// The wiring this netlist cannot give the solver, as an error naming it (see `circuit`).
    fn refuse_unbound_forms(&self) -> Result<()> {
        for net in self.ir.nets() {
            let Some(init) = net.init() else { continue };
            let declarer = net.declared_by();
            let carried = declarer.body().carriers().iter().enumerate().any(|(c, carrier)| {
                carrier.init == Some(init) && declarer.net(c).is_some_and(|n| n.path() == net.path())
            });
            if !carried {
                return Err(Error::simple(
                    SolverDomain::Digital,
                    format!(
                        "{}: the net `{}` has an initial value that no carrier of `{}` bound to it holds; \
                         a net's `init` reaches the solver as the initial value of the carrier that declares it",
                        net.origin(),
                        net.path(),
                        declarer.path()
                    ),
                ));
            }
        }
        for instance in self.ir.instances() {
            let body = instance.body();
            for (c, carrier) in body.carriers().iter().enumerate() {
                if BodyPlan::carrier_is_port(body, c as u32) {
                    continue;
                }
                let Some(net) = instance.net(c) else { continue };
                if let Some(other) = net.instances().into_iter().find(|i| i.path() != instance.path()) {
                    return Err(Error::simple(
                        SolverDomain::Digital,
                        format!(
                            "{}: the signal `{}` of `{}` is the net `{}` that `{}` meets it on, but body `{}` binds it \
                             to no child (its placements record no carrier binding)",
                            carrier.origin,
                            carrier.name,
                            instance.path(),
                            net.path(),
                            other.path(),
                            body.name()
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{BodySpec, InstanceSpec, IrBuilder, Origin};

    use super::*;

    /// No body has a module: the circuit is the design's empty top.
    struct Nothing;

    impl Modules for Nothing {
        fn module(&self, _: BodyId) -> Option<&'static Module> {
            None
        }
    }

    fn empty_design() -> Ir {
        let origin = || Origin { file: "netlist.rs".into(), line: 1, col: 1 };
        let mut b = IrBuilder::new();
        let body = b.body(BodySpec {
            name: "Top".into(),
            origin: origin(),
            params: Vec::new(),
            nodes: Vec::new(),
            branches: Vec::new(),
            limits: Vec::new(),
            stims: Vec::new(),
            locals: Vec::new(),
            analog: None,
            children: Vec::new(),
        });
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes: Vec::new(), params: Vec::new(), origin: origin() });
        b.finish(top)
    }

    #[test]
    fn with_microsteps_reaches_the_builder() {
        // HII-02 AC 7: the bound a netlist carries is the bound the circuit binds the digital
        // engine with; a netlist that was never given one keeps the engine's 1000
        let ir = empty_design();
        let given = Netlist::new(&ir).with_microsteps(7).circuit(&Nothing).expect("binds");
        assert_eq!(given.bound().expect("bound").microsteps, 7);
        let default = Netlist::new(&ir).circuit(&Nothing).expect("binds");
        assert_eq!(default.bound().expect("bound").microsteps, 1000);
    }
}
