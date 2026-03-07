====================
Your First Circuit
====================

Let's build a simple voltage divider to get familiar with Piperine.

The Circuit
===========

We'll create a voltage divider with:

* Input voltage: 10V
* R1 = 1kΩ
* R2 = 1kΩ
* Expected output: 5V

.. mermaid::

   graph LR
       V[Vin 10V] --> R1[R1 1kΩ]
       R1 --> Out[n_out]
       Out --> R2[R2 1kΩ]
       R2 --> GND[GND]

Complete Code
=============

Create a new file ``main.rs``:

.. code-block:: rust

   use piperine::prelude::*;

   fn main() {
       // Declare node to access outside builder
       let mut n_out = GND;
       
       // Define the circuit using the builder pattern
       let mut circuit: CircuitInstance = Circuit::builder("Voltage Divider", |b| {
           // Create nodes
           let n_in = b.port();
           n_out = b.port();
           
           // 10V DC voltage source
           b.voltage_source("Vin", n_in.clone(), GND, 10.0.V());
           
           // R1: 1 kΩ
           b.resistor("R1", n_in, n_out.clone(), 1.0.kOhms());
           
           // R2: 1 kΩ
           b.resistor("R2", n_out.clone(), GND, 1.0.kOhms());
       })
       .into();

       // Solve the DC operating point
       let result = circuit
           .dc(Context::default())
           .expect("Invalid configuration")
           .solve()
           .expect("Convergence failed");

       // Read the output voltage
       let v_out_value = result.get_node(&n_out).unwrap();
       
       println!("Output Voltage: {:.4} V", v_out_value);
       // Expected: 5.0000 V
   }

Run it:

.. code-block:: bash

   cargo run

Output:

.. code-block:: text

   Output Voltage: 5.0000 V

Code Walkthrough
================

1. **Import prelude**
   
   .. code-block:: rust
   
      use piperine::prelude::*;
   
   This imports all common types and traits.

2. **Declare node for external access**
   
   .. code-block:: rust
   
      let mut n_out = GND;
   
   Nodes created inside the builder are scoped to the closure. To access them later, declare them outside as mutable.

3. **Create circuit with builder**
   
   .. code-block:: rust
   
      let mut circuit: CircuitInstance = Circuit::builder("Voltage Divider", |b| {
          // ... add devices here
      }).into();
   
   The builder is a static method on ``Circuit`` that takes a closure receiving ``&mut Circuit``.

4. **Create nodes with ``port()``**
   
   .. code-block:: rust
   
      let n_in = b.port();
      n_out = b.port();  // Assign to outer variable
   
   Each call to ``b.port()`` creates a unique node identifier.

5. **Add voltage source**
   
   .. code-block:: rust
   
      b.voltage_source("Vin", n_in.clone(), GND, 10.0.V());
   
   * ``"Vin"`` - device name
   * ``n_in.clone()`` - positive node (clone when reused)
   * ``GND`` - negative node (ground reference)
   * ``10.0.V()`` - 10 volts with unit extension

6. **Add resistors**
   
   .. code-block:: rust
   
      b.resistor("R1", n_in, n_out.clone(), 1.0.kOhms());
   
   * Connects node ``n_in`` to ``n_out``
   * Use ``.clone()`` when a node is used multiple times
   * Resistance: 1kΩ (using ``.kOhms()`` unit extension)

7. **Run DC analysis**
   
   .. code-block:: rust
   
      let result = circuit.dc(Context::default())?.solve()?;
   
   Solves the DC operating point (steady-state voltages and currents).

8. **Read results**
   
   .. code-block:: rust
   
      let v_out_value = result.get_node(&n_out).unwrap();
   
   Retrieves the voltage at the ``n_out`` node (pass by reference).

Understanding the Result
========================

The voltage divider formula:

.. math::

   V_{out} = V_{in} \\times \\frac{R2}{R1 + R2} = 10V \\times \\frac{1k\\Omega}{2k\\Omega} = 5V

Piperine solves this using Modified Nodal Analysis (MNA), obtaining the exact same result!

What's Next?
============

Now that you've created your first circuit, explore:

* :doc:`concepts` - Core concepts and design philosophy
* :doc:`../user-guide/circuit-builder` - Advanced builder patterns
* :doc:`../tutorials/dc-analysis` - More DC analysis examples
* :doc:`../tutorials/ac-analysis` - Frequency response analysis
* :doc:`../tutorials/transient-analysis` - Time-domain simulation

Try It Yourself
===============

Modify the circuit:

1. **Different ratio**: Change R1 to 2kΩ, expect V_out = 3.33V
2. **Different voltage**: Change input to 5V, expect V_out = 2.5V
3. **Add more resistors**: Create a 3-resistor divider

Example with 2kΩ / 1kΩ:

.. code-block:: rust

   let mut n_out = GND;
   
   let circuit: CircuitInstance = Circuit::builder("2:1 Divider", |b| {
       let n_in = b.port();
       n_out = b.port();
       
       b.voltage_source("Vin", n_in.clone(), GND, 10.0.V());
       b.resistor("R1", n_in, n_out.clone(), 2.0.kOhms());
       b.resistor("R2", n_out.clone(), GND, 1.0.kOhms());
   }).into();
   // Expected: V_out = 10V × (1kΩ / 3kΩ) = 3.333V
