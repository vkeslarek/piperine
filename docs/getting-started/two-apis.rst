=============================
Two Ways to Build Circuits
=============================

Piperine provides **two APIs** for building circuits, each suited for different use cases.

Circuit::new() - Direct API
============================

**Use when:** You need to access nodes after creating the circuit.

.. code-block:: rust

   let mut circuit = Circuit::new("My Circuit");
   
   // Create nodes - you can keep references to them!
   let input = circuit.port();
   let output = circuit.port();
   
   // Add devices
   circuit.voltage_source("V1", input.clone(), GND, 5.0.V());
   circuit.resistor("R1", input, output.clone(), 1.0.kOhms());
   circuit.capacitor("C1", output.clone(), GND, 10.0.nF());
   
   // Convert to CircuitInstance
   let mut circuit_instance: CircuitInstance = circuit.into();
   
   // Run analysis - we can still use 'output' here!
   let result = circuit_instance.dc(Context::default())?.solve()?;
   let v_out = result.get_node(&output)?;

**Advantages:**

* ✅ Keep references to nodes
* ✅ Access nodes after circuit creation
* ✅ Build circuits incrementally
* ✅ Clear and explicit

**When to use:**

* Reading simulation results
* Building complex circuits
* Interactive circuit construction
* Most common use case

Circuit::builder() - Closure API
=================================

**Use when:** You don't need to access nodes afterward (self-contained circuit).

.. code-block:: rust

   let circuit: CircuitInstance = Circuit::builder("My Circuit", |b| {
       let input = b.port();
       let output = b.port();
       
       b.voltage_source("V1", input.clone(), GND, 5.0.V());
       b.resistor("R1", input, output.clone(), 1.0.kOhms());
       b.capacitor("C1", output.clone(), GND, 10.0.nF());
       // Nodes 'input' and 'output' are dropped when closure ends
   }).into();
   
   // Can't access nodes here - they're gone!
   let result = circuit.dc(Context::default())?.solve()?;
   // How do we read results? We can't! ❌

**Advantages:**

* ✅ Concise for simple circuits
* ✅ Clear scope boundaries
* ✅ Good for subcircuits (see below)

**Disadvantages:**

* ❌ Can't access nodes after creation
* ❌ Can't read simulation results easily
* ❌ Limited use cases

**When to use:**

* Creating subcircuit definitions (functions that return ``Circuit``)
* Quick prototyping when you don't need results
* Nested scopes with ``.scoped()``

Comparison
==========

Same Circuit, Both APIs
-----------------------

**Direct API (``Circuit::new()``):**

.. code-block:: rust

   let mut circuit = Circuit::new("RC Filter");
   
   let vin = circuit.port();
   let vout = circuit.port();
   
   circuit.voltage_source("V1", vin.clone(), GND, 1.0.V());
   circuit.resistor("R1", vin, vout.clone(), 1.0.kOhms());
   circuit.capacitor("C1", vout.clone(), GND, 10.0.nF());
   
   let mut circuit: CircuitInstance = circuit.into();
   let result = circuit.ac(Context::default())?.solve_sweep(options)?;
   
   // Can access vout here
   for point in result.points() {
       let v = point.get_node(&vout)?;
       println!("{:.2} Hz: {:?}", point.frequency, v);
   }

**Closure API (``Circuit::builder()``):**

.. code-block:: rust

   let circuit: CircuitInstance = Circuit::builder("RC Filter", |b| {
       let vin = b.port();
       let vout = b.port();
       
       b.voltage_source("V1", vin.clone(), GND, 1.0.V());
       b.resistor("R1", vin, vout.clone(), 1.0.kOhms());
       b.capacitor("C1", vout.clone(), GND, 10.0.nF());
       // vout drops here - can't access it later!
   }).into();
   
   let result = circuit.ac(Context::default())?.solve_sweep(options)?;
   // Can't read results by node - don't have vout reference! ❌

**Verdict:** Use ``Circuit::new()`` for almost everything!

Subcircuits
===========

Subcircuits allow you to create reusable circuit blocks.

Defining a Subcircuit
---------------------

A subcircuit is just a function that returns a ``Circuit``:

.. code-block:: rust

   fn voltage_divider(
       input: NodeIdentifier,
       output: NodeIdentifier,
       gnd: NodeIdentifier,
   ) -> Circuit {
       let mut c = Circuit::new("Voltage Divider");
       c.resistor("R1", input, output.clone(), 1.0.kOhms());
       c.resistor("R2", output, gnd, 1.0.kOhms());
       c
   }

Note: This is where ``Circuit::builder()`` could be useful:

.. code-block:: rust

   fn voltage_divider(
       input: NodeIdentifier,
       output: NodeIdentifier,
       gnd: NodeIdentifier,
   ) -> Circuit {
       Circuit::builder("Voltage Divider", |b| {
           b.resistor("R1", input.clone(), output.clone(), 1.0.kOhms());
           b.resistor("R2", output.clone(), gnd.clone(), 1.0.kOhms());
       })
   }

Using Subcircuits
-----------------

Use ``.subcircuit()`` to instantiate a subcircuit with scoped names:

.. code-block:: rust

   let mut circuit = Circuit::new("Two Stage Divider");
   
   let vin = circuit.port();
   let v_mid = circuit.port();
   let vout = circuit.port();
   
   circuit.voltage_source("V1", vin.clone(), GND, 10.0.V());
   
   // First stage - components will be named "STAGE1.R1", "STAGE1.R2"
   circuit.subcircuit("STAGE1", voltage_divider(vin, v_mid.clone(), GND));
   
   // Second stage - components will be named "STAGE2.R1", "STAGE2.R2"
   circuit.subcircuit("STAGE2", voltage_divider(v_mid, vout.clone(), GND));
   
   let mut circuit: CircuitInstance = circuit.into();
   let result = circuit.dc(Context::default())?.solve()?;
   
   println!("V_mid = {:.4} V", result.get_node(&v_mid)?);   // 5.0 V
   println!("V_out = {:.4} V", result.get_node(&vout)?);    // 2.5 V

**Benefits:**

* ✅ Automatic name scoping (no name conflicts)
* ✅ Reusable building blocks
* ✅ Hierarchical circuit structure
* ✅ Easier to understand complex circuits

Parameterized Subcircuits
--------------------------

Make subcircuits configurable:

.. code-block:: rust

   fn resistor_divider(
       input: NodeIdentifier,
       output: NodeIdentifier,
       gnd: NodeIdentifier,
       ratio: f64,  // Output voltage ratio (0.0 to 1.0)
   ) -> Circuit {
       let mut c = Circuit::new("Parameterized Divider");
       let r1 = 1000.0 * (1.0 - ratio);  // Top resistor
       let r2 = 1000.0 * ratio;          // Bottom resistor
       c.resistor("R1", input, output.clone(), r1.Ohms());
       c.resistor("R2", output, gnd, r2.Ohms());
       c
   }
   
   // Use it
   let mut circuit = Circuit::new("Custom Dividers");
   let vin = circuit.port();
   let v_25 = circuit.port();
   let v_75 = circuit.port();
   
   circuit.voltage_source("V1", vin.clone(), GND, 10.0.V());
   circuit.subcircuit("DIV_25", resistor_divider(vin.clone(), v_25.clone(), GND, 0.25));
   circuit.subcircuit("DIV_75", resistor_divider(vin, v_75.clone(), GND, 0.75));

Nested Scopes with ``scoped()``
================================

For manual scope control without subcircuits:

.. code-block:: rust

   let mut circuit = Circuit::new("Nested");
   
   let n1 = circuit.port();
   let n2 = circuit.port();
   
   circuit.voltage_source("V1", n1.clone(), GND, 5.0.V());
   
   // Create components in "BLOCK1" scope
   circuit.scoped("BLOCK1", |c| {
       c.resistor("R1", n1.clone(), n2.clone(), 1.0.kOhms());
       // This creates "BLOCK1.R1"
   });
   
   // Create components in "BLOCK2" scope
   circuit.scoped("BLOCK2", |c| {
       c.resistor("R1", n2.clone(), GND, 1.0.kOhms());
       // This creates "BLOCK2.R1" - no conflict!
   });

Best Practices
==============

1. **Use ``Circuit::new()`` by default**
   
   .. code-block:: rust
   
      // Recommended
      let mut circuit = Circuit::new("My Circuit");
      let output = circuit.port();
      // ... build circuit ...
      let circuit: CircuitInstance = circuit.into();
      // Can access 'output' here

2. **Use ``Circuit::builder()`` only for subcircuits**
   
   .. code-block:: rust
   
      fn my_subcircuit(...) -> Circuit {
          Circuit::builder("Subcircuit", |b| {
              // ... build ...
          })
      }

3. **Create subcircuits as functions**
   
   .. code-block:: rust
   
      fn low_pass_filter(input: NodeIdentifier, output: NodeIdentifier) -> Circuit {
          // ... implementation ...
      }

4. **Use ``.subcircuit()`` for reusable blocks**
   
   .. code-block:: rust
   
      circuit.subcircuit("FILTER1", low_pass_filter(n1, n2));
      circuit.subcircuit("FILTER2", low_pass_filter(n2, n3));

5. **Keep node references when needed**
   
   .. code-block:: rust
   
      let important_node = circuit.port();
      // ... use in circuit ...
      // ... read in results later

Summary
=======

+-------------------------+-------------------+------------------------+
| Feature                 | Circuit::new()    | Circuit::builder()     |
+=========================+===================+========================+
| Keep node references    | ✅ Yes            | ❌ No                  |
+-------------------------+-------------------+------------------------+
| Access nodes later      | ✅ Yes            | ❌ No                  |
+-------------------------+-------------------+------------------------+
| Read simulation results | ✅ Easy           | ❌ Difficult           |
+-------------------------+-------------------+------------------------+
| Best for                | Most use cases    | Subcircuit definitions |
+-------------------------+-------------------+------------------------+
| Recommended             | ✅ Default choice | Use sparingly          |
+-------------------------+-------------------+------------------------+

**Rule of thumb:** Use ``Circuit::new()`` unless you're defining a subcircuit function.

See Also
========

* :doc:`first-circuit` - Step-by-step first circuit
* :doc:`../user-guide/circuit-builder` - Advanced building patterns
* :doc:`concepts` - Core concepts
