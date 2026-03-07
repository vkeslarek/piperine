==============
Device Library
==============

Piperine currently implements **6 device types**: 3 passive components, 2 sources, and 1 non-linear device.

Overview
========

+-------------------+------------------+------------------+
| Device            | Type             | Non-linear?      |
+===================+==================+==================+
| Resistor          | Passive          | No               |
+-------------------+------------------+------------------+
| Capacitor         | Passive          | No               |
+-------------------+------------------+------------------+
| Inductor          | Passive          | No               |
+-------------------+------------------+------------------+
| Voltage Source    | Source           | No               |
+-------------------+------------------+------------------+
| Current Source    | Source           | No               |
+-------------------+------------------+------------------+
| Diode             | Semiconductor    | Yes              |
+-------------------+------------------+------------------+

Passive Devices
===============

Resistor
--------

**Signature:**

.. code-block:: rust

   b.resistor(name, node_p, node_n, resistance)

**Parameters:**

* ``name: impl Into<String>`` - Device name
* ``node_p: NodeIdentifier`` - Positive terminal
* ``node_n: NodeIdentifier`` - Negative terminal
* ``resistance: impl Into<Dynamic<Ohm>>`` - Resistance value

**Example:**

.. code-block:: rust

   let n1 = b.port();
   let n2 = b.port();
   
   b.resistor("R1", n1, n2.clone(), 1.0.kOhms());
   b.resistor("R2", n2.clone(), GND, 470.0.Ohms());

**Available Units:**

* ``.Ohms()`` - Ohms
* ``.kOhms()`` - Kiloohms (1,000 Ω)
* ``.MOhms()`` - Megaohms (1,000,000 Ω)

**Model:**

Linear conductance: ``G = 1/R``

Stamps to matrix: ``G`` at positions ``(n+,n+)``, ``(n+,n-)``, ``(n-,n+)``, ``(n-,n-)``

Capacitor
---------

**Signature:**

.. code-block:: rust

   b.capacitor(name, node_p, node_n, capacitance)

**Parameters:**

* ``name: impl Into<String>`` - Device name
* ``node_p: NodeIdentifier`` - Positive terminal
* ``node_n: NodeIdentifier`` - Negative terminal
* ``capacitance: impl Into<Farad>`` - Capacitance value

**Example:**

.. code-block:: rust

   let n1 = b.port();
   
   b.capacitor("C1", n1.clone(), GND, 10.0.nF());
   b.capacitor("C2", n1.clone(), GND, 100.0.pF());

**Available Units:**

* ``.F()`` - Farads
* ``.uF()`` - Microfarads (10⁻⁶ F)
* ``.nF()`` - Nanofarads (10⁻⁹ F)
* ``.pF()`` - Picofarads (10⁻¹² F)

**Model:**

* **DC Analysis:** Acts as open circuit (infinite impedance)
* **AC Analysis:** Impedance ``Z = 1/(jωC)``
* **Transient:** Uses Gear 2nd-order integration with truncation error calculation

Inductor
--------

**Signature:**

.. code-block:: rust

   b.inductor(name, node_p, node_n, inductance)

**Parameters:**

* ``name: impl Into<String>`` - Device name
* ``node_p: NodeIdentifier`` - Positive terminal
* ``node_n: NodeIdentifier`` - Negative terminal
* ``inductance: impl Into<Henry>`` - Inductance value

**Example:**

.. code-block:: rust

   let n1 = b.port();
   let n2 = b.port();
   
   b.inductor("L1", n1, n2.clone(), 1.0.mH());

**Available Units:**

* ``.H()`` - Henries
* ``.mH()`` - Millihenries (10⁻³ H)
* ``.uH()`` - Microhenries (10⁻⁶ H)

**Model:**

* **DC Analysis:** Acts as short circuit (zero impedance)
* **AC Analysis:** Impedance ``Z = jωL``
* **Transient:** Uses Gear 2nd-order integration with truncation error calculation

Sources
=======

Voltage Source
--------------

**Signature:**

.. code-block:: rust

   b.voltage_source(name, node_p, node_n, waveform)

**Parameters:**

* ``name: impl Into<String>`` - Device name
* ``node_p: NodeIdentifier`` - Positive terminal
* ``node_n: NodeIdentifier`` - Negative terminal
* ``waveform: impl Into<Waveform>`` - Source waveform

**Waveform Types:**

DC Source
~~~~~~~~~

Constant voltage:

.. code-block:: rust

   let vin = b.port();
   b.voltage_source("V1", vin.clone(), GND, 5.0.V());

Step Source
~~~~~~~~~~~

Voltage step with rise time:

.. code-block:: rust

   let vin = b.port();
   b.voltage_source("V1", vin.clone(), GND, Step {
       initial: 0.0.V(),
       final_value: 5.0.V(),
       delay: 0.1.ms(),
       rise_time: 1.0.us(),
   });

**Parameters:**

* ``initial`` - Initial voltage
* ``final_value`` - Final voltage after step
* ``delay`` - Time before step begins
* ``rise_time`` - Linear transition time

**Available Voltage Units:**

* ``.V()`` - Volts
* ``.mV()`` - Millivolts (10⁻³ V)

Current Source
--------------

**Signature:**

.. code-block:: rust

   b.current_source(name, node_p, node_n, waveform)

**Parameters:**

* ``name: impl Into<String>`` - Device name
* ``node_p: NodeIdentifier`` - Positive terminal (current flows from p to n)
* ``node_n: NodeIdentifier`` - Negative terminal
* ``waveform: impl Into<Waveform>`` - Source waveform

**Example:**

.. code-block:: rust

   let n1 = b.port();
   b.current_source("I1", n1.clone(), GND, 1.0.mA());

**Available Current Units:**

* ``.A()`` - Amperes
* ``.mA()`` - Milliamperes (10⁻³ A)
* ``.uA()`` - Microamperes (10⁻⁶ A)

Non-Linear Devices
==================

Diode
-----

**Signature:**

.. code-block:: rust

   b.diode(name, anode, cathode)

**Parameters:**

* ``name: impl Into<String>`` - Device name
* ``anode: NodeIdentifier`` - Anode terminal
* ``cathode: NodeIdentifier`` - Cathode terminal

**Example:**

.. code-block:: rust

   let anode = b.port();
   let cathode = b.port();
   
   b.voltage_source("V1", anode.clone(), GND, 5.0.V());
   b.diode("D1", anode.clone(), cathode.clone());
   b.resistor("R1", cathode.clone(), GND, 1.0.kOhms());

**Model:**

Shockley diode equation:

.. math::

   I_D = I_S (e^{V_D / (n V_T)} - 1)

Where:

* ``I_S`` = Saturation current (default: 1e-14 A)
* ``n`` = Ideality factor (default: 1.0)
* ``V_T`` = Thermal voltage (≈ 26 mV at room temperature)

**Convergence:**

The diode uses Newton-Raphson iteration with:

* Conductance: ``g_d = ∂I_D / ∂V_D``
* Current source equivalent for linearization

Common Patterns
===============

RC Low-Pass Filter
------------------

.. code-block:: rust

   let circuit: CircuitInstance = Circuit::builder("RC Filter", |b| {
       let vin = b.port();
       let vout = b.port();
       
       b.voltage_source("Vin", vin.clone(), GND, 1.0.V());
       b.resistor("R1", vin, vout.clone(), 1.0.kOhms());
       b.capacitor("C1", vout.clone(), GND, 10.0.nF());
   }).into();

**Cutoff frequency:** ``f_c = 1 / (2πRC) ≈ 15.9 kHz``

Voltage Divider
---------------

.. code-block:: rust

   let circuit: CircuitInstance = Circuit::builder("Divider", |b| {
       let vin = b.port();
       let vout = b.port();
       
       b.voltage_source("V1", vin.clone(), GND, 10.0.V());
       b.resistor("R1", vin, vout.clone(), 1.0.kOhms());
       b.resistor("R2", vout.clone(), GND, 1.0.kOhms());
   }).into();

**Output:** ``V_out = V_in × R2/(R1+R2) = 5V``

LC Resonator
------------

.. code-block:: rust

   let circuit: CircuitInstance = Circuit::builder("LC Tank", |b| {
       let vin = b.port();
       let vtank = b.port();
       
       b.voltage_source("V1", vin.clone(), GND, 1.0.V());
       b.resistor("R1", vin, vtank.clone(), 50.0.Ohms());
       b.inductor("L1", vtank.clone(), GND, 1.0.mH());
       b.capacitor("C1", vtank.clone(), GND, 10.0.nF());
   }).into();

**Resonance:** ``f_0 = 1 / (2π√LC) ≈ 50.3 kHz``

Diode Rectifier
---------------

.. code-block:: rust

   let circuit: CircuitInstance = Circuit::builder("Half-Wave Rectifier", |b| {
       let vin = b.port();
       let vout = b.port();
       
       b.voltage_source("Vin", vin.clone(), GND, Step {
           initial: 0.0.V(),
           final_value: 5.0.V(),
           delay: 0.0.ms(),
           rise_time: 1.0.us(),
       });
       b.diode("D1", vin, vout.clone());
       b.resistor("Rload", vout.clone(), GND, 1.0.kOhms());
   }).into();

See Also
========

* :doc:`circuit-builder` - How to add devices to circuits
* :doc:`analyses` - Running analyses with these devices
* :doc:`../tutorials/index` - Device usage tutorials
