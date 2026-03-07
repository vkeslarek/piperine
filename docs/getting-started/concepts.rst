=============
Core Concepts
=============

Piperine introduces a new way of thinking about circuit simulation. Instead of writing text-based netlists, you define circuits as **code**—leveraging the full power of Rust's type system, IDE support, and modern tooling.

Circuits as Data Structures
============================

Traditional SPICE Approach
--------------------------

In SPICE, circuits are text files:

.. code-block:: spice

   * Voltage Divider
   V1 n1 0 DC 10
   R1 n1 n2 1k
   R2 n2 0 1k
   .dc V1
   .end

**Problems:**

* No IDE autocompletion
* Cryptic error messages
* Hard to parameterize
* Can't use loops, variables, or functions
* Text parsing required

Piperine Approach
-----------------

In Piperine, circuits are Rust structs:

.. code-block:: rust

   use piperine::prelude::*;

   let circuit: CircuitInstance = Circuit::builder("Voltage Divider", |b| {
       let n1 = b.port();
       let n2 = b.port();
       
       b.voltage_source("V1", n1.clone(), GND, 10.0.V());
       b.resistor("R1", n1, n2.clone(), 1.0.kOhms());
       b.resistor("R2", n2.clone(), GND, 1.0.kOhms());
   }).into();

**Advantages:**

* ✅ IDE autocompletion and type checking
* ✅ Compile-time error detection
* ✅ Use loops, variables, functions
* ✅ Easy parameterization
* ✅ Unit-safe (can't mix volts and amps!)

The Builder Pattern
===================

Piperine uses the **builder pattern** for circuit construction:

.. code-block:: rust

   let circuit: CircuitInstance = Circuit::builder("Circuit Name", |b| {
       // Create nodes
       let n1 = b.port();
       let n2 = b.port();
       
       // Add devices using 'b'
       b.resistor("R1", n1, n2.clone(), 1.0.kOhms());
       b.capacitor("C1", n2.clone(), GND, 10.0.nF());
   }).into();

The closure receives ``&mut Circuit`` (aliased as ``b``) that provides methods for adding devices.

Why a closure?
--------------

The closure pattern:

1. Provides a clean scope for device definitions
2. Automatically handles internal bookkeeping
3. Ensures all devices are added before circuit instantiation
4. Makes code more readable
5. Allows capturing variables from outer scope

Type-Safe Units
===============

Piperine uses **unit extensions** to prevent unit errors:

.. code-block:: rust

   use piperine::prelude::*;

   let voltage = 5.0.V();       // 5 volts
   let resistance = 1.0.kOhms(); // 1000 ohms
   let time = 10.0.us();        // 10 microseconds

Available Unit Extensions
-------------------------

**Voltage:**

* ``.V()`` - Volts
* ``.mV()`` - Millivolts

**Resistance:**

* ``.Ohms()`` - Ohms
* ``.kOhms()`` - Kiloohms
* ``.MOhms()`` - Megaohms

**Capacitance:**

* ``.F()`` - Farads
* ``.uF()`` - Microfarads
* ``.nF()`` - Nanofarads
* ``.pF()`` - Picofarads

**Inductance:**

* ``.H()`` - Henries
* ``.mH()`` - Millihenries
* ``.uH()`` - Microhenries

**Time:**

* ``.s()`` - Seconds
* ``.ms()`` - Milliseconds
* ``.us()`` - Microseconds
* ``.ns()`` - Nanoseconds

**Frequency:**

* ``.Hz()`` - Hertz
* ``.kHz()`` - Kilohertz
* ``.MHz()`` - Megahertz
* ``.GHz()`` - Gigahertz

Nodes and References
====================

Creating Nodes
--------------

Nodes are created using the ``port()`` method:

.. code-block:: rust

   let input = b.port();
   let output = b.port();
   
   b.resistor("R1", input, output.clone(), 1.0.kOhms());
   //               ^^^^^  ^^^^^^^^^^^^^^
   //               node1  node2 (cloned for reuse)

* Each call to ``b.port()`` creates a unique node
* Use descriptive variable names: ``input``, ``output``, ``vcc``
* Ground is always ``GND`` (built-in constant)
* Nodes are type-safe and checked at compile time
* **Use ``.clone()`` when a node is used multiple times**

Accessing Nodes Outside the Builder
------------------------------------

To access nodes after circuit creation, declare them outside:

.. code-block:: rust

   let mut output = GND;  // Declare outside
   
   let circuit: CircuitInstance = Circuit::builder("Example", |b| {
       let input = b.port();
       output = b.port();  // Assign inside
       
       b.resistor("R1", input, output.clone(), 1.0.kOhms());
       b.capacitor("C1", output.clone(), GND, 10.0.nF());
   }).into();
   
   // Now we can use 'output' to read results
   let result = circuit.dc(Context::default())?.solve()?;
   let v_out = result.get_node(&output)?;

CircuitVariable
---------------

``CircuitVariable`` represents any measurable quantity:

* ``CircuitVariable::Node(node_id)`` - Voltage at a node
* ``CircuitVariable::Branch(branch_id)`` - Current through a device
* ``CircuitVariable::Time`` - Time (transient analysis)
* ``CircuitVariable::Frequency`` - Frequency (AC analysis)

Analysis Types
==============

Piperine implements **5 analysis types**:

1. **DC Analysis** - Operating point
   
   Finds steady-state DC voltages and currents.
   
   .. code-block:: rust
   
      let result = circuit.dc(Context::default())?.solve()?;

2. **AC Analysis** - Frequency response
   
   Small-signal frequency sweep.
   
   .. code-block:: rust
   
      let sweep = circuit.ac(Context::default())?.solve_sweep(options)?;

3. **Transient Analysis** - Time-domain simulation
   
   Simulates circuit behavior over time.
   
   .. code-block:: rust
   
      let trajectory = circuit.transient(options, Context::default())?.solve()?;

4. **Noise Analysis** - Noise floor calculation
   
   Calculates total output noise.
   
   .. code-block:: rust
   
      let noise = circuit.noise(options, Context::default())?.solve()?;

5. **Transfer Function Analysis** - DC gain and impedances
   
   Calculates small-signal DC gain, input resistance, and output resistance.
   
   .. code-block:: rust
   
      let tf = circuit.transfer_function(options, Context::default())?.solve()?;

Device Library
==============

Currently Implemented Devices
------------------------------

**Passive Devices:**

* ``resistor(name, n1, n2, resistance)``
* ``capacitor(name, n1, n2, capacitance)``
* ``inductor(name, n1, n2, inductance)``

**Sources:**

* ``voltage_source(name, pos, neg, waveform)``
* ``current_source(name, pos, neg, waveform)``

**Non-linear:**

* ``diode(name, anode, cathode)``

Waveforms
---------

**DC:**

.. code-block:: rust

   DC(5.0.V())

**Step:**

.. code-block:: rust

   Step {
       initial: 0.0.V(),
       final_value: 5.0.V(),
       delay: 0.1.ms(),
       rise_time: 1.0.us(),
   }

Solver Context
==============

``Context`` provides solver configuration:

.. code-block:: rust

   let context = Context::default();

Contains settings for:

* Convergence tolerance
* Maximum iterations
* Damping parameters
* SOA (Safe Operating Area) checking

Most users can use ``Context::default()``.

Modified Nodal Analysis (MNA)
==============================

Piperine uses **Modified Nodal Analysis**, a matrix-based method:

1. **Build system matrix**: Each device "stamps" its contribution
2. **Solve linear system**: ``A × x = b``
3. **For non-linear devices**: Use Newton-Raphson iteration
4. **For transient**: Use Gear 2nd-order integration

This is the same approach used by SPICE and ngspice.

Next Steps
==========

* :doc:`../user-guide/circuit-builder` - Advanced builder techniques
* :doc:`../user-guide/devices` - Complete device library reference
* :doc:`../user-guide/analyses` - Deep dive into all 5 analyses
* :doc:`../tutorials/index` - Hands-on tutorials
