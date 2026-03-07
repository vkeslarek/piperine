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
       // Create circuit with Circuit::new() to access nodes later
       let mut circuit = Circuit::new("Voltage Divider");
       
       // Create nodes
       let n_in = circuit.port();
       let n_out = circuit.port();
       
       // 10V DC voltage source
       circuit.voltage_source("Vin", n_in.clone(), GND, 10.0.V());
       
       // R1: 1 kΩ
       circuit.resistor("R1", n_in, n_out.clone(), 1.0.kOhms());
       
       // R2: 1 kΩ
       circuit.resistor("R2", n_out.clone(), GND, 1.0.kOhms());
       
       // Convert to CircuitInstance
       let mut circuit: CircuitInstance = circuit.into();

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

2. **Create circuit with ``Circuit::new()``**
   
   .. code-block:: rust
   
      let mut circuit = Circuit::new("Voltage Divider");
   
   Use ``Circuit::new()`` when you need to access nodes after creating them.

3. **Create nodes with ``port()``**
   
   .. code-block:: rust
   
      let n_in = circuit.port();
      let n_out = circuit.port();
   
   Each call to ``circuit.port()`` creates a unique node identifier that you can use later.

4. **Add voltage source**
   
   .. code-block:: rust
   
      circuit.voltage_source("Vin", n_in.clone(), GND, 10.0.V());
   
   * ``"Vin"`` - device name
   * ``n_in.clone()`` - positive node (clone when reused)
   * ``GND`` - negative node (ground reference)
   * ``10.0.V()`` - 10 volts with unit extension

5. **Add resistors**
   
   .. code-block:: rust
   
      circuit.resistor("R1", n_in, n_out.clone(), 1.0.kOhms());
   
   * Connects node ``n_in`` to ``n_out``
   * Use ``.clone()`` when a node is used multiple times
   * Resistance: 1kΩ (using ``.kOhms()`` unit extension)

6. **Convert to ``CircuitInstance``**
   
   .. code-block:: rust
   
      let mut circuit: CircuitInstance = circuit.into();
   
   Convert the ``Circuit`` to ``CircuitInstance`` before running analyses.

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

   let mut circuit = Circuit::new("2:1 Divider");
   
   let n_in = circuit.port();
   let n_out = circuit.port();
   
   circuit.voltage_source("Vin", n_in.clone(), GND, 10.0.V());
   circuit.resistor("R1", n_in, n_out.clone(), 2.0.kOhms());
   circuit.resistor("R2", n_out.clone(), GND, 1.0.kOhms());
   
   let circuit: CircuitInstance = circuit.into();
   // Expected: V_out = 10V × (1kΩ / 3kΩ) = 3.333V
