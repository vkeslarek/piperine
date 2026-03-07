===================================
Piperine Circuit Simulator
===================================

**Piperine** is a modern circuit simulator written in Rust. It provides a **code-first approach** to circuit design and simulation, replacing text-based netlists with a clean, type-safe API.

.. note::
   ⚠️ **PRE-ALPHA SOFTWARE**: Piperine is under active development. APIs may change.

Quick Example
=============

.. code-block:: rust

   use piperine::prelude::*;

   fn main() {
       let mut vout = GND;
       
       let mut circuit: CircuitInstance = Circuit::builder("Voltage Divider", |b| {
           let vin = b.port();
           vout = b.port();
           
           b.voltage_source("V1", vin.clone(), GND, 10.0.V());
           b.resistor("R1", vin, vout.clone(), 1.0.kOhms());
           b.resistor("R2", vout.clone(), GND, 1.0.kOhms());
       }).into();

       let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
       println!("V_out = {:.4} V", result.get_node(&vout).unwrap());  // 5.0000 V
   }

Features
========

✅ **5 Analysis Types**

* DC Operating Point
* AC Frequency Response
* Transient (with adaptive timestep)
* Noise Analysis
* Transfer Function

✅ **Modern API**

* Builder pattern with IDE autocompletion
* Compile-time type checking
* Unit-safe (``.V()``, ``.kOhms()``, ``.nF()``)

✅ **Robust Solver**

* Newton-Raphson with damping
* Adaptive timestep control (43% fewer steps)
* Sparse matrix solver (handles ~40,000 nodes)

✅ **Validated**

* Transfer Function: <0.1% error vs ngspice
* 18 passing tests at 100% coverage

Documentation Sections
======================

.. toctree::
   :maxdepth: 2
   :caption: Getting Started

   getting-started/index
   getting-started/installation
   getting-started/first-circuit
   getting-started/concepts

.. toctree::
   :maxdepth: 2
   :caption: User Guide

   user-guide/index
   user-guide/circuit-builder
   user-guide/devices
   user-guide/analyses
   user-guide/units
   user-guide/solver-context

.. toctree::
   :maxdepth: 2
   :caption: API Reference

   docs/crates/piperine/lib

Current Capabilities
====================

**Implemented Devices:**

* Resistor, Capacitor, Inductor
* Voltage Source, Current Source
* Diode (Shockley equation)

**Implemented Analyses:**

* **DC Analysis** - Operating point calculation
* **AC Analysis** - Small-signal frequency response
* **Transient Analysis** - Time-domain with adaptive timestep
* **Noise Analysis** - Thermal noise calculation
* **Transfer Function** - DC gain, R_in, R_out

**Infrastructure:**

* Modified Nodal Analysis (MNA)
* Sparse matrix solver (faer)
* Gear 2nd-order integration
* Truncation error control
* Breakpoint system

Why Piperine?
=============

**Traditional SPICE:**

.. code-block:: spice

   * Voltage Divider
   V1 n1 0 DC 10
   R1 n1 n2 1k
   R2 n2 0 1k
   .dc V1
   .end

Problems: Cryptic syntax, no IDE support, hard to parameterize

**Piperine:**

.. code-block:: rust

   let circuit: CircuitInstance = Circuit::builder("Divider", |b| {
       let n1 = b.port();
       let n2 = b.port();
       b.voltage_source("V1", n1.clone(), GND, 10.0.V());
       b.resistor("R1", n1, n2.clone(), 1.0.kOhms());
       b.resistor("R2", n2.clone(), GND, 1.0.kOhms());
   }).into();

Advantages: Type-safe, IDE autocompletion, use loops/functions, clear errors

Target Applications
===================

Piperine is optimized for **medium-sized circuits** (~40,000 nodes):

* Analog design blocks
* Switching power supplies
* RF amplifiers and filters
* Educational contexts
* Design optimization loops

**Not intended for:** Billion-transistor digital verification (use specialized tools)

License
=======

**MIT License** - Use freely in private, educational, or commercial projects.

Indices and Tables
==================

* :ref:`genindex`
* :ref:`search`