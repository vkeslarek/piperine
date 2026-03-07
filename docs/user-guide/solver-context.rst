==============
Solver Context
==============

The ``Context`` struct configures solver behavior for all analyses.

Overview
========

Every analysis requires a ``Context``:

.. code-block:: rust

   let result = circuit.dc(Context::default())?.solve()?;
   let trajectory = circuit.transient(options, Context::default())?.solve()?;

For most users, ``Context::default()`` is sufficient. Advanced users can customize:

* Convergence tolerance
* Maximum Newton-Raphson iterations
* Damping parameters
* Safe Operating Area (SOA) checking

Default Settings
================

``Context::default()`` provides conservative, production-ready settings:

.. code-block:: rust

   Context {
       reltol: 1e-3,        // Relative tolerance
       abstol: 1e-12,       // Absolute tolerance (A)
       vntol: 1e-6,         // Voltage tolerance (V)
       max_iterations: 100, // Max Newton iterations
       damping: true,       // Enable damping
       soa_check: true,     // Enable SOA checks
   }

These values provide:

* Robust convergence for most circuits
* Reasonable accuracy (0.1% relative error)
* Protection against unrealistic solutions

Convergence Parameters
======================

Relative Tolerance (reltol)
---------------------------

Controls relative error tolerance:

.. code-block:: rust

   let context = Context {
       reltol: 1e-4,  // Tighter: 0.01% error
       ..Default::default()
   };

**Default:** ``1e-3`` (0.1% error)

**When to change:**

* **Tighten** (smaller value) for high-precision requirements
* **Loosen** (larger value) if convergence is difficult

**Trade-off:** Tighter tolerance → more iterations → slower simulation

Absolute Tolerance (abstol)
---------------------------

Minimum current resolution:

.. code-block:: rust

   let context = Context {
       abstol: 1e-15,  // Picoampere resolution
       ..Default::default()
   };

**Default:** ``1e-12`` A (picoamperes)

**When to change:**

* Circuits with very small currents (femtoampere range)
* High-precision analog designs

Voltage Tolerance (vntol)
-------------------------

Minimum voltage resolution:

.. code-block:: rust

   let context = Context {
       vntol: 1e-9,  // Nanovolt resolution
       ..Default::default()
   };

**Default:** ``1e-6`` V (microvolts)

**When to change:**

* Ultra-low-voltage circuits
* Precision voltage references

Maximum Iterations
==================

Limits Newton-Raphson iterations:

.. code-block:: rust

   let context = Context {
       max_iterations: 200,  // Allow more attempts
       ..Default::default()
   };

**Default:** ``100``

**When to change:**

* **Increase:** For difficult-to-converge circuits (many diodes, tight coupling)
* **Decrease:** To fail fast during debugging

**Failure mode:** If exceeded, solver returns ``ConvergenceError``

Damping
=======

Newton-Raphson damping prevents oscillation:

.. code-block:: rust

   let context = Context {
       damping: false,  // Disable damping
       ..Default::default()
   };

**Default:** ``true`` (enabled)

**How it works:**

Instead of full Newton step ``x_new = x_old - J⁻¹f(x)``, use:

.. math::

   x_{new} = x_{old} - \\alpha \\cdot J^{-1}f(x)

where ``α ∈ (0, 1]`` is chosen to reduce residual.

**When to disable:**

* Simple linear circuits (unnecessary overhead)
* Debugging convergence issues

**When to enable:**

* Non-linear circuits (diodes, transistors)
* Circuits with tight coupling
* Default for production use

Safe Operating Area (SOA)
==========================

Checks for unrealistic values:

.. code-block:: rust

   let context = Context {
       soa_check: false,  // Disable SOA
       ..Default::default()
   };

**Default:** ``true`` (enabled)

**Checks:**

* Node voltages: ``|V| < 1e6`` V
* Branch currents: ``|I| < 1e6`` A
* Diode voltages: within reasonable forward/reverse ranges

**When to disable:**

* Debugging (to see raw solver output)
* Specialized circuits with extreme values

**Failure mode:** Returns ``SafeOperatingAreaViolation`` error

Creating Custom Contexts
=========================

Full custom context:

.. code-block:: rust

   let context = Context {
       reltol: 1e-4,
       abstol: 1e-15,
       vntol: 1e-9,
       max_iterations: 200,
       damping: true,
       soa_check: true,
   };
   
   let result = circuit.dc(context)?.solve()?;

Or modify defaults:

.. code-block:: rust

   let context = Context {
       max_iterations: 200,
       ..Context::default()
   };

Common Configurations
=====================

High-Precision Configuration
----------------------------

For research or validation:

.. code-block:: rust

   let high_precision = Context {
       reltol: 1e-6,     // 0.0001% error
       abstol: 1e-15,    // Femtoampere resolution
       vntol: 1e-9,      // Nanovolt resolution
       max_iterations: 500,
       damping: true,
       soa_check: true,
   };

**Use case:** Comparing against reference simulators (ngspice, HSPICE)

Fast Configuration
------------------

For rapid iteration during design:

.. code-block:: rust

   let fast = Context {
       reltol: 1e-2,     // Loose: 1% error
       abstol: 1e-9,     // Nanoampere resolution
       vntol: 1e-3,      // Millivolt resolution
       max_iterations: 50,
       damping: true,
       soa_check: false,
   };

**Use case:** Quick design exploration, parameter sweeps

**Warning:** May miss subtle issues!

Robust Configuration
--------------------

For difficult circuits:

.. code-block:: rust

   let robust = Context {
       reltol: 1e-2,     // Looser tolerance
       abstol: 1e-9,
       vntol: 1e-3,
       max_iterations: 500,  // Many attempts
       damping: true,        // Always on
       soa_check: false,     // Don't give up early
   };

**Use case:** Circuits with many diodes, strong non-linearities

Debugging Convergence Issues
=============================

If a circuit won't converge:

1. **Check initial guess**
   
   DC analysis provides initial conditions for AC/Transient. Verify DC solves first.

2. **Loosen tolerances**
   
   .. code-block:: rust
   
      let context = Context {
          reltol: 1e-2,
          ..Context::default()
      };

3. **Increase iterations**
   
   .. code-block:: rust
   
      let context = Context {
          max_iterations: 500,
          ..Context::default()
      };

4. **Enable damping**
   
   Always use ``damping: true`` for non-linear circuits.

5. **Simplify circuit**
   
   Comment out devices one by one to isolate the problem.

6. **Check component values**
   
   Extreme values (femtofarads, teraohms) cause numerical issues.

Performance Considerations
==========================

**Tighter tolerance = Slower:**

Each factor of 10 tighter adds ~2-3 iterations on average.

**More iterations = Longer timeout:**

Budget ~10 µs per iteration per node for large circuits.

**Damping overhead:**

Adds 1-2 extra linear solves per iteration (~30% slower).

**SOA checking:**

Negligible overhead (<1%).

**Recommendation:** Use defaults unless you have specific needs.

See Also
========

* :doc:`analyses` - Using Context with different analyses
* :doc:`../architecture/solver-design` - How the solver works internally
