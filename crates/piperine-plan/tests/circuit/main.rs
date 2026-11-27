//! What the two backends share: `BodyPlan` is one type, the shape a plan gives a body is the shape the interpreter serves, and `Netlist::circuit` binds each instance to the module of its body. Each former target is a module of this one.

mod shared;
