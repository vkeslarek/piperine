# Annex H — Glossary (informative)

Ported from `VAMS-LRM-2-4.md` (Accellera Verilog-AMS LRM 2.4, Annex H) as
reference material, per the `verilog-ams-support` feature's T57 (VAMS-26).
Informative, not normative — a vocabulary reference for readers of this
project's docs and of `.vams`/`.va` source, not a set of enforced rules.

**AMS** — See *Verilog-AMS*.

**behavioral description** — A mathematical mapping of inputs to outputs for
a module, including intermediate variables and control flow.

**behavioral model** — A version of a module with a unique set of parameters
designed to model a specific component.

**block** — A level within the behavioral description of a module,
delimited by `begin` and `end`.

**branch** — A relationship between two nodes and their attached quantities
within the behavioral description of a module. Each branch has two
quantities, a potential (value) and a flow, with a reference direction for
each.

**child module** — A module instantiated inside another, "parent" module. A
complete definition of the child module needs to be defined somewhere. Also
known as *submodule* or *instantiated module*.

**compact model** — A behavioral model or description of a semiconductor
device.

**component** — A fundamental unit within a system which encapsulates
behavior and/or structure. Modules and models can represent a single
component or a subcircuit with many components.

**constitutive relationships** — The essential relationships (expressions
and statements) between the outputs of a module and its inputs and
parameters, which define the nature of the module. These relationships
constitute a behavioral description.

**control flow** — The conditional and iterative statements controlling the
behavior of a module. These statements evaluate arbitrary variables
(counters, flags, tokens) to control the operation of different sections of
a behavioral description.

**flow** — One of the two fundamental quantities used to simulate the
behavior of a system. In electrical systems, the flow is the current.

**instance** — Any named occurrence of a component created from a module
definition. One module can occur in multiple instances.

**instantiation** — The process of creating an instance from a module
definition or simulator primitive and defining the connectivity and
parameters of that instance (placing the instance in the circuit or
system).

**Kirchhoff's Laws** — The physical laws defining the interconnection
relationships of nodes, branches, values, and flows. They specify a
conservation of flow in and out of a node and a conservation of value around
a loop of branches.

**level** — One block within a behavioral description, delimited by a pair
of matching keywords such as `begin`-`end` or `discipline`-`enddiscipline`.

**model** — A named instance with a unique group of parameters specifying
the behavior of one particular version of a module. Models can be used to
instantiate elements with parametric specifications different from those in
the original module definition.

**module** — A definition of the interfaces and behavior of a component.

**nesting level** — Synonym for *level* above.

**node** — A connection point in the system, with access functions for
potential and/or flow through an underlying discipline.

**node declaration** — The statement in a module definition identifying the
names of the nets associated with the module ports or local to the module.
A net declaration also identifies the discipline of the net, which in turn
identifies the access functions.

**NR method** — Newton-Raphson method. A generalized method for solving
systems of nonlinear algebraic equations by breaking them into a series of
many small linear operations ideally suited for computer processing.
Piperine's own `ConvergencePlan`/`NewtonStrategy` (`piperine-solver`,
`analyses/convergence.rs`) implements this.

**parameter** — A constant for characterizing the behavior of an instance of
a module. Parameters are defined in a module's interface declarations and
can be overridden each time the module is instantiated.

**parameter declaration** — The statement in a module definition which
defines the parameters of that module.

**port** — An external connection point for a module (also known as a
*terminal*).

**potential** — One of the two fundamental quantities used to simulate the
behavior of a system. In electrical systems, the potential is the voltage.

**primitive** — A basic component defined entirely in terms of behavior,
without reference to any other primitives. The smallest and simplest
portion of a simulated circuit or system.

**probe** — A branch in a circuit (or system) which does not alter its
behavior, but lets the simulator read out the potential or flow at that
point.

**reference direction** — A convention for determining whether the value of
a node, the flow through a branch, the value across a branch, or the flow
in or out of a terminal, is positive or negative.

**reference node** — The global node (which equals zero, `0`) against whose
potentials all node values are measured. Nets declared as `ground` are
bound to the reference node.

**run time binding** — The conditional introduction and removal of value
and flow sources during a simulation. A value source can replace a flow
source and vice versa. Binding a source to a specific node or branch
prevents it from going into an unknown state.

**scope** — The current nesting level of a block statement, which includes
all lines of code within one set of braces (or `begin`/`end`) in a module
definition.

**structural definitions** — Instantiating modules inside other modules
through module definitions and declarations, creating a hierarchical
structure in the module's behavioral description. (Piperine's own
UNBREAKABLE POM-navigability rule requires this authored hierarchy to
survive into `Design::modules` unmodified — see `CLAUDE.md`.)

**terminal** — See *port*.

**Verilog-A** — A subset of Verilog-AMS detailing the analog version of IEEE
Std 1364-2005 Verilog HDL (see Annex C, "Analog language subset"). A
language for the behavioral description of continuous-time systems, using a
syntax similar to IEEE Std 1364-2005 Verilog HDL.

**Verilog-AMS** — Mixed-signal version of IEEE Std 1364-2005 Verilog HDL. A
language for the behavioral description of continuous-time and
discrete-time systems based on the IEEE Std 1364-2005 Verilog HDL
specification. This is Piperine's sole frontend today (PHDL retired,
MD-44/MD-45).
