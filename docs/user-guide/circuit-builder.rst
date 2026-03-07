===============
Circuit Builder
===============

The builder pattern is the primary way to construct circuits in Piperine.

Basic Usage
===========

Every circuit starts with the ``Circuit::builder()`` static method:

.. code-block:: rust

   use piperine::prelude::*;

   let circuit: CircuitInstance = Circuit::builder("Circuit Name", |b| {
       // Add devices here
   }).into();

The closure receives ``&mut Circuit`` (referred to as ``b``) that provides methods for:

* Creating nodes with ``b.port()``
* Adding devices (resistors, capacitors, etc.)
* Creating subcircuits
* Defining models

The ``.into()`` call converts ``Circuit`` to ``CircuitInstance`` for analysis.

Creating Nodes
==============

Use ``b.port()`` to create circuit nodes:

.. code-block:: rust

   let circuit: CircuitInstance = Circuit::builder("Example", |b| {
       let input = b.port();
       let output = b.port();
       let internal = b.port();
       
       b.voltage_source("Vin", input.clone(), GND, 5.0.V());
       b.resistor("R1", input, internal.clone(), 1.0.kOhms());
       b.capacitor("C1", internal, output.clone(), 10.0.nF());
       b.resistor("R2", output.clone(), GND, 1.0.kOhms());
   }).into();

**Key Points:**

* Each ``b.port()`` call creates a unique node
* Use descriptive variable names
* ``GND`` is the global ground reference
* Nodes are type-safe at compile time
* **Use ``.clone()`` when a node is used in multiple devices**

Adding Devices
==============

Passive Devices
---------------

**Resistor:**

.. code-block:: rust

   b.resistor(name, node_p, node_n, resistance);

Example:

.. code-block:: rust

   let n1 = b.port();
   let n2 = b.port();
   b.resistor("R1", n1, n2.clone(), 1.0.kOhms());

**Capacitor:**

.. code-block:: rust

   b.capacitor(name, node_p, node_n, capacitance);

Example:

.. code-block:: rust

   let n1 = b.port();
   b.capacitor("C1", n1.clone(), GND, 10.0.nF());

**Inductor:**

.. code-block:: rust

   b.inductor(name, node_p, node_n, inductance);

Example:

.. code-block:: rust

   let n1 = b.port();
   let n2 = b.port();
   b.inductor("L1", n1, n2.clone(), 1.0.mH());

Sources
-------

**Voltage Source:**

.. code-block:: rust

   b.voltage_source(name, node_p, node_n, waveform);

DC Example:

.. code-block:: rust

   let vin = b.port();
   b.voltage_source("V1", vin.clone(), GND, 5.0.V());

Step Example:

.. code-block:: rust

   let vin = b.port();
   b.voltage_source("V1", vin.clone(), GND, Step {
       initial: 0.0.V(),
       final_value: 5.0.V(),
       delay: 0.1.ms(),
       rise_time: 1.0.us(),
   });

**Current Source:**

.. code-block:: rust

   b.current_source(name, node_p, node_n, waveform);

Example:

.. code-block:: rust

   let n1 = b.port();
   b.current_source("I1", n1.clone(), GND, 1.0.mA());

Non-Linear Devices
------------------

**Diode:**

.. code-block:: rust

   b.diode(name, anode, cathode);

Example:

.. code-block:: rust

   let anode = b.port();
   let cathode = b.port();
   b.diode("D1", anode.clone(), cathode.clone());

Pattern: Parameterized Circuits
================================

Use Rust variables to parameterize circuits:

.. code-block:: rust

   fn create_rc_filter(cutoff_freq: f64) -> CircuitInstance {
       let r = 1000.0;  // 1 kΩ
       let c = 1.0 / (2.0 * std::f64::consts::PI * cutoff_freq * r);
       
       Circuit::builder("RC Filter", |b| {
           let vin = b.port();
           let vout = b.port();
           
           b.voltage_source("Vin", vin.clone(), GND, 1.0.V());
           b.resistor("R1", vin, vout.clone(), r.Ohms());
           b.capacitor("C1", vout.clone(), GND, c.F());
       }).into()
   }
   
   // Create 1kHz filter
   let filter_1khz = create_rc_filter(1000.0);

Pattern: Component Arrays
==========================

Use loops to create repetitive structures:

.. code-block:: rust

   let circuit: CircuitInstance = Circuit::builder("Resistor Ladder", |b| {
       let vin = b.port();
       b.voltage_source("Vin", vin.clone(), GND, 10.0.V());
       
       let mut current_node = vin;
       
       // Create 10-stage ladder
       for i in 0..10 {
           let next_node = b.port();
           b.resistor(&format!("R{}", i), current_node, next_node.clone(), 1.0.kOhms());
           current_node = next_node;
       }
       
       b.resistor("Rload", current_node.clone(), GND, 1.0.kOhms());
   }).into();

Pattern: Conditional Devices
=============================

Use Rust conditionals to customize circuits:

.. code-block:: rust

   fn create_filter(filter_type: &str) -> CircuitInstance {
       Circuit::builder("Filter", |b| {
           let vin = b.port();
           let vout = b.port();
           
           b.voltage_source("Vin", vin.clone(), GND, 1.0.V());
           
           match filter_type {
               "lowpass" => {
                   b.resistor("R1", vin, vout.clone(), 1.0.kOhms());
                   b.capacitor("C1", vout.clone(), GND, 10.0.nF());
               }
               "highpass" => {
                   b.capacitor("C1", vin, vout.clone(), 10.0.nF());
                   b.resistor("R1", vout.clone(), GND, 1.0.kOhms());
               }
               _ => panic!("Unknown filter type"),
           }
       }).into()
   }

Converting to CircuitInstance
==============================

The builder returns a ``Circuit`` struct. Use ``.into()`` to convert it to ``CircuitInstance`` for analysis:

.. code-block:: rust

   let mut circuit: CircuitInstance = Circuit::builder("My Circuit", |b| {
       // ... build circuit
   }).into();
   
   // Now we can run analyses
   let result = circuit.dc(Context::default())?.solve()?;

The ``.into()`` call instantiates the circuit, assigning internal indices to all nodes and branches.

Best Practices
==============

1. **Use Descriptive Names**
   
   .. code-block:: rust
   
      // Good
      let input = b.port();
      let output = b.port();
      
      // Avoid
      let n1 = b.port();
      let n2 = b.port();

2. **Group Related Devices**
   
   .. code-block:: rust
   
      // Input stage
      let vin = b.port();
      b.voltage_source("Vin", vin.clone(), GND, 5.0.V());
      
      // Filter stage
      let vfiltered = b.port();
      b.resistor("R1", vin, vfiltered.clone(), 1.0.kOhms());
      b.capacitor("C1", vfiltered.clone(), GND, 10.0.nF());

3. **Extract Functions for Reusable Blocks**
   
   .. code-block:: rust
   
      fn add_rc_stage(b: &mut Circuit, input: NodeIdentifier, output: NodeIdentifier) {
          b.resistor("R", input, output.clone(), 1.0.kOhms());
          b.capacitor("C", output.clone(), GND, 10.0.nF());
      }

4. **Use Type-Safe Units**
   
   .. code-block:: rust
   
      // Good
      b.resistor("R1", n1, n2.clone(), 1.0.kOhms());
      
      // Compile error - wrong unit type
      b.resistor("R1", n1, n2.clone(), 1.0.nF());  // ❌

See Also
========

* :doc:`devices` - Complete device reference
* :doc:`../tutorials/index` - Hands-on examples
* :doc:`../examples/index` - Example circuits
