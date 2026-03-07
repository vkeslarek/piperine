==========
Unit System
==========

Piperine uses **type-safe unit extensions** to prevent unit errors at compile time.

Philosophy
==========

Instead of passing raw ``f64`` values:

.. code-block:: rust

   // Unsafe - what units?
   b.resistor("R1", n1, n2.clone(), 1000.0);  // ❌
   b.capacitor("C1", n2.clone(), GND, 0.00000001);  // ❌

Use **unit extensions**:

.. code-block:: rust

   // Safe - units are explicit
   b.resistor("R1", n1, n2.clone(), 1.0.kOhms());  // ✅
   b.capacitor("C1", n2.clone(), GND, 10.0.nF());  // ✅

The Rust compiler ensures you don't accidentally mix units:

.. code-block:: rust

   // Compile error - type mismatch!
   b.resistor("R1", n1, n2.clone(), 10.0.nF());  // ❌ Won't compile!

Available Units
===============

Voltage
-------

.. code-block:: rust

   use piperine::prelude::*;
   
   let v1 = 5.0.V();    // 5 volts
   let v2 = 100.0.mV(); // 100 millivolts = 0.1 V

**Methods:**

* ``.V()`` - Volts
* ``.mV()`` - Millivolts (10⁻³ V)

**Examples:**

.. code-block:: rust

   b.voltage_source("V1", vin.clone(), GND, 5.0.V());
   b.voltage_source("V2", vin.clone(), GND, 3300.0.mV());  // 3.3V

Resistance
----------

.. code-block:: rust

   let r1 = 1.0.kOhms();  // 1 kilohm = 1000 Ω
   let r2 = 470.0.Ohms(); // 470 ohms
   let r3 = 1.0.MOhms();  // 1 megohm = 1,000,000 Ω

**Methods:**

* ``.Ohms()`` - Ohms (Ω)
* ``.kOhms()`` - Kiloohms (1,000 Ω)
* ``.MOhms()`` - Megaohms (1,000,000 Ω)

**Examples:**

.. code-block:: rust

   b.resistor("R1", n1, n2.clone(), 1.0.kOhms());
   b.resistor("R2", n2.clone(), GND, 470.0.Ohms());
   b.resistor("R3", n1, n2.clone(), 10.0.MOhms());

Capacitance
-----------

.. code-block:: rust

   let c1 = 10.0.nF();  // 10 nanofarads
   let c2 = 100.0.pF(); // 100 picofarads
   let c3 = 1.0.uF();   // 1 microfarad

**Methods:**

* ``.F()`` - Farads
* ``.uF()`` - Microfarads (10⁻⁶ F)
* ``.nF()`` - Nanofarads (10⁻⁹ F)
* ``.pF()`` - Picofarads (10⁻¹² F)

**Examples:**

.. code-block:: rust

   b.capacitor("C1", n1, GND, 10.0.nF());
   b.capacitor("C2", n1, GND, 100.0.pF());
   b.capacitor("C3", n1, GND, 1.0.uF());

Inductance
----------

.. code-block:: rust

   let l1 = 1.0.mH();  // 1 millihenry
   let l2 = 100.0.uH(); // 100 microhenries

**Methods:**

* ``.H()`` - Henries
* ``.mH()`` - Millihenries (10⁻³ H)
* ``.uH()`` - Microhenries (10⁻⁶ H)

**Examples:**

.. code-block:: rust

   b.inductor("L1", n1, n2.clone(), 1.0.mH());
   b.inductor("L2", n1, n2.clone(), 100.0.uH());

Time
----

.. code-block:: rust

   let t1 = 1.0.ms();  // 1 millisecond
   let t2 = 10.0.us(); // 10 microseconds
   let t3 = 100.0.ns(); // 100 nanoseconds

**Methods:**

* ``.s()`` - Seconds
* ``.ms()`` - Milliseconds (10⁻³ s)
* ``.us()`` - Microseconds (10⁻⁶ s)
* ``.ns()`` - Nanoseconds (10⁻⁹ s)

**Examples:**

.. code-block:: rust

   // Transient analysis
   let options = TransientAnalysisOptions {
       stop_time: 1.0.ms(),
       dt: 1.0.us(),
   };
   
   // Step source
   b.voltage_source("V1", vin.clone(), GND, Step {
       initial: 0.0.V(),
       final_value: 5.0.V(),
       delay: 0.1.ms(),
       rise_time: 1.0.us(),
   });

Frequency
---------

.. code-block:: rust

   let f1 = 1.0.kHz();  // 1 kilohertz
   let f2 = 10.0.MHz(); // 10 megahertz

**Methods:**

* ``.Hz()`` - Hertz
* ``.kHz()`` - Kilohertz (1,000 Hz)
* ``.MHz()`` - Megahertz (1,000,000 Hz)
* ``.GHz()`` - Gigahertz (1,000,000,000 Hz)

**Examples:**

.. code-block:: rust

   // AC sweep
   let options = AcSweepOptions::logarithmic(
       1.0.Hz(),
       1.0.MHz(),
       100
   );

Current
-------

.. code-block:: rust

   let i1 = 1.0.A();   // 1 ampere
   let i2 = 100.0.mA(); // 100 milliamperes
   let i3 = 50.0.uA();  // 50 microamperes

**Methods:**

* ``.A()`` - Amperes
* ``.mA()`` - Milliamperes (10⁻³ A)
* ``.uA()`` - Microamperes (10⁻⁶ A)

**Examples:**

.. code-block:: rust

   b.current_source("I1", n1.clone(), GND, 1.0.mA());

Using Units in Calculations
============================

Units can be used in mathematical expressions:

.. code-block:: rust

   // Calculate RC time constant
   let r = 1000.0;  // Ω
   let c = 10e-9;   // F
   let tau = r * c;  // = 10 µs
   
   // Use in circuit
   b.resistor("R1", n1, n2.clone(), r.Ohms());
   b.capacitor("C1", n2.clone(), GND, c.F());

**Design Example: RC Filter**

Calculate component values for 1 kHz cutoff:

.. code-block:: rust

   fn create_rc_filter(cutoff_hz: f64) -> CircuitInstance {
       let r = 1000.0;  // Choose R = 1 kΩ
       let c = 1.0 / (2.0 * std::f64::consts::PI * cutoff_hz * r);
       
       Circuit::builder("RC Filter", |b| {
           let vin = b.port();
           let vout = b.port();
           
           b.voltage_source("Vin", vin.clone(), GND, 1.0.V());
           b.resistor("R1", vin, vout.clone(), r.Ohms());
           b.capacitor("C1", vout.clone(), GND, c.F());
       }).into()
   }
   
   // Create 1 kHz low-pass filter
   let filter = create_rc_filter(1000.0);

Unit Conversions
================

No explicit conversion needed - the unit extensions handle it:

.. code-block:: rust

   // All equivalent
   b.resistor("R1", n1, n2.clone(), 1000.0.Ohms());
   b.resistor("R1", n1, n2.clone(), 1.0.kOhms());
   
   // All equivalent
   b.capacitor("C1", n1, GND, 0.00001.F());
   b.capacitor("C1", n1, GND, 10.0.uF());
   b.capacitor("C1", n1, GND, 10000.0.nF());

Common Conversions
------------------

**Resistance:**

* 1 kΩ = 1,000 Ω
* 1 MΩ = 1,000 kΩ = 1,000,000 Ω

**Capacitance:**

* 1 µF = 1,000 nF = 1,000,000 pF
* 1 nF = 1,000 pF

**Inductance:**

* 1 mH = 1,000 µH

**Time:**

* 1 ms = 1,000 µs = 1,000,000 ns

**Frequency:**

* 1 kHz = 1,000 Hz
* 1 MHz = 1,000 kHz = 1,000,000 Hz

Best Practices
==============

1. **Always use unit extensions**
   
   .. code-block:: rust
   
      // Good
      b.resistor("R1", n1, n2.clone(), 1.0.kOhms());
      
      // Bad
      b.resistor("R1", n1, n2.clone(), 1000.0);  // ❌

2. **Use readable scales**
   
   .. code-block:: rust
   
      // Good - easy to read
      10.0.nF()
      1.0.kOhms()
      
      // Avoid - hard to count zeros
      0.00000001.F()
      1000.0.Ohms()

3. **Match component datasheets**
   
   If a datasheet says "10 nF", use ``10.0.nF()`` not ``0.00001.uF()``

4. **Keep units consistent in context**
   
   .. code-block:: rust
   
      // Good - all in nF
      b.capacitor("C1", n1, GND, 10.0.nF());
      b.capacitor("C2", n2, GND, 100.0.nF());
      
      // Avoid mixing unnecessarily
      b.capacitor("C1", n1, GND, 10.0.nF());
      b.capacitor("C2", n2, GND, 0.1.uF());  // Same value, different unit

See Also
========

* :doc:`devices` - Device library using these units
* :doc:`circuit-builder` - Building circuits with units
* :doc:`../getting-started/concepts` - Unit system philosophy
