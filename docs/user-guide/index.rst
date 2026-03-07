==========
User Guide
==========

This guide covers all aspects of using Piperine for circuit simulation.

.. toctree::
   :maxdepth: 2
   
   circuit-builder
   devices
   analyses
   units
   solver-context

Overview
========

Piperine provides a complete circuit simulation environment with:

* **5 Analysis Types**: DC, AC, Transient, Noise, Transfer Function
* **6 Device Types**: Resistor, Capacitor, Inductor, Diode, Voltage/Current Sources
* **Type-Safe Units**: Compile-time unit checking
* **Modern API**: Builder pattern with IDE support
* **Robust Solver**: Newton-Raphson with adaptive timestep control

Quick Reference
===============

Circuit Construction
--------------------

.. code-block:: rust

   use piperine::prelude::*;

   let circuit: CircuitInstance = Circuit::builder("My Circuit", |b| {
       // Create nodes
       let vin = b.port();
       let vout = b.port();
       
       // Add devices
       b.voltage_source("V1", vin.clone(), GND, 5.0.V());
       b.resistor("R1", vin, vout.clone(), 1.0.kOhms());
       b.capacitor("C1", vout.clone(), GND, 10.0.nF());
   }).into();

Running Analyses
----------------

.. code-block:: rust

   // DC Operating Point
   let dc_result = circuit.dc(Context::default())?.solve()?;
   
   // Transient Analysis
   let tran_result = circuit.transient(
       TransientAnalysisOptions {
           stop_time: 1.0.ms(),
           dt: 1.0.us(),
       },
       Context::default()
   )?.solve()?;
   
   // AC Analysis
   let ac_result = circuit.ac(Context::default())?.solve_sweep(
       AcSweepOptions::logarithmic(1.0.Hz(), 1.0.MHz(), 100)
   )?;

Reading Results
---------------

.. code-block:: rust

   // Node voltage (pass by reference)
   let voltage = result.get_node(&node_id)?;
   
   // Branch current
   let current = result.get_branch(&branch_id)?;

What's Next?
============

* :doc:`circuit-builder` - Learn the builder pattern in depth
* :doc:`devices` - Complete device library reference
* :doc:`analyses` - All 5 analysis types explained
* :doc:`units` - Type-safe unit system
* :doc:`solver-context` - Configure solver behavior
