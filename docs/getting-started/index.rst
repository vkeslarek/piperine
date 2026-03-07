===============
Getting Started
===============

Welcome to Piperine! This guide will help you get up and running with circuit simulation in Rust.

Piperine is a modern circuit simulator that lets you define circuits as **code** instead of text-based netlists. You get IDE autocompletion, compile-time type checking, and a clean, expressive API.

.. toctree::
   :maxdepth: 2
   
   installation
   first-circuit
   concepts

Quick Start
===========

1. **Install Rust** (if you haven't already)

   .. code-block:: bash

      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

2. **Add Piperine to your project**

   .. code-block:: bash

      cargo add piperine

3. **Create your first circuit** (voltage divider)

   .. code-block:: rust

      use piperine::prelude::*;

      fn main() {
          let mut n_out = GND;
          
          let mut circuit: CircuitInstance = Circuit::builder("Voltage Divider", |b| {
              let n_in = b.port();
              n_out = b.port();
              
              b.voltage_source("Vin", n_in.clone(), GND, 10.0.V());
              b.resistor("R1", n_in, n_out.clone(), 1.0.kOhms());
              b.resistor("R2", n_out.clone(), GND, 1.0.kOhms());
          }).into();

          let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
          let v_out = result.get_node(&n_out).unwrap();
          
          println!("Output: {:.4} V", v_out);  // 5.0000 V
      }

4. **Run it**

   .. code-block:: bash

      cargo run

What Makes Piperine Different?
===============================

Traditional SPICE
-----------------

.. code-block:: spice

   * Voltage Divider
   V1 n1 0 DC 10
   R1 n1 n2 1k
   R2 n2 0 1k
   .dc V1
   .end

Problems:

* Cryptic syntax
* No IDE support
* Hard to parameterize
* Error messages are unclear

Piperine
--------

.. code-block:: rust

   let circuit: CircuitInstance = Circuit::builder("Voltage Divider", |b| {
       let n1 = b.port();
       let n2 = b.port();
       
       b.voltage_source("V1", n1.clone(), GND, 10.0.V());
       b.resistor("R1", n1, n2.clone(), 1.0.kOhms());
       b.resistor("R2", n2.clone(), GND, 1.0.kOhms());
   }).into();

Advantages:

* ✅ IDE autocompletion
* ✅ Compile-time type checking
* ✅ Use variables, loops, functions
* ✅ Clear error messages
* ✅ Unit-safe (``.V()``, ``.kOhms()``)

Where to Go Next
================

* :doc:`installation` - Detailed installation instructions
* :doc:`first-circuit` - Step-by-step first circuit guide
* :doc:`concepts` - Core concepts and design philosophy
* :doc:`../tutorials/index` - Hands-on tutorials
* :doc:`../user-guide/index` - Complete user guide
