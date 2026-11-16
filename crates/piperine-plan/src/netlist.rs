//! `Netlist::circuit`: the IR's instance table as `CircuitBuilder` calls, over whatever supplies
//! the module of each body.

use piperine_abi::raw::Module;
use piperine_ir::{BodyId, CarrierRole, Constant, Ir, NodeRef};
use piperine_solver::abi::{Error, SolverDomain};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, ParamValue, Port, Result};

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
}

impl<'a> Netlist<'a> {
    pub fn new(ir: &'a Ir) -> Netlist<'a> {
        Netlist { ir, resolution: None }
    }

    /// This netlist with the session's tick length (the `RESOLUTION` global), in seconds; the
    /// solver's own default (one femtosecond) when never given.
    pub fn with_resolution(self, seconds_per_tick: f64) -> Netlist<'a> {
        Netlist { resolution: Some(seconds_per_tick), ..self }
    }

    /// The design as a solver circuit: one solver node per design node, in the IR's node order
    /// (so node `i` is `NodeIdentifier::Anonymous(i)`), named by its path to the builder; one
    /// `CircuitBuilder::device` per instance whose body has a module, under the instance's path.
    /// A body node is bound by position to the instance's design node (`NodeRef::Ground` to the
    /// reference). Each param is passed by name, except one that holds its body's default: it
    /// is given no value and its pin keeps the default.
    ///
    /// A carrier port (an `in` or `out` carrier, never a `signal`) is bound, after the node
    /// ports and in the body's order, to the design net the instance gives it
    /// (`Port::Net` named by the net's path, so the net is read by that name); a carrier the
    /// instance gives no net is left open, its pin private to the instance. A carrier's initial
    /// value is its pin's `HAS_INIT`, in the shape: it reaches the solver with the pin.
    ///
    /// Refused, naming the carrier and the net, until msl-digital binds them
    /// (`ir-rust` verification F-B1): a parent's `signal` that its children's carrier ports meet
    /// on, since the builder binds no `PRIVATE` pin to a named net and the parent's drives and
    /// commits would never reach the children.
    pub fn circuit(&self, modules: &impl Modules) -> Result<CircuitInstance> {
        self.refuse_unbound_forms()?;
        let ir = self.ir;
        let mut builder = CircuitBuilder::new(ir.top().path());
        let nodes: Vec<_> = ir.nodes().map(|n| builder.node(n.path())).collect();
        let ground = builder.ground();
        if let Some(seconds) = self.resolution {
            builder.resolution(seconds);
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
            for (c, carrier) in body.carriers().iter().enumerate() {
                if carrier.role == CarrierRole::Signal {
                    continue;
                }
                ports.push(match instance.net(c) {
                    Some(net) => Port::Net(net.path().to_string()),
                    None => Port::Open,
                });
            }
            let values: Vec<(&str, ParamValue)> = body
                .params()
                .iter()
                .zip(instance.params())
                .filter(|(param, given)| param.default.as_ref() != Some(*given))
                .map(|(param, given)| {
                    let value = match given {
                        Constant::Real(r) => ParamValue::Real(r.get()),
                        Constant::Bool(b) => ParamValue::Bool(*b),
                        // The solver's integer leaf; an `N`-bit value fits its 64 bits.
                        Constant::Int(v) => ParamValue::Int(*v as i64),
                    };
                    (param.name.as_str(), value)
                })
                .collect();
            builder.device(module, instance.path(), &values, &ports);
        }
        builder.build()
    }

    /// The wiring this netlist cannot yet give the solver, as an error naming it (see `circuit`).
    fn refuse_unbound_forms(&self) -> Result<()> {
        for instance in self.ir.instances() {
            let body = instance.body();
            for (c, carrier) in body.carriers().iter().enumerate() {
                if carrier.role != CarrierRole::Signal {
                    continue;
                }
                let Some(net) = instance.net(c) else { continue };
                if let Some(child) = net.instances().first() {
                    return Err(Error::simple(
                        SolverDomain::Digital,
                        format!(
                            "{}: the signal `{}` of `{}` is the net `{}` that the carrier ports of `{}` meet on; \
                             a parent's signal bound to its children's ports is not supported yet (msl-digital)",
                            carrier.origin,
                            carrier.name,
                            instance.path(),
                            net.path(),
                            child.path()
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}
