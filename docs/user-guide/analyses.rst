=============
Analysis Types
=============

Piperine implements **5 analysis types** for circuit simulation. All analyses operate on ``CircuitInstance``.

Overview
========

+----------------------+------------------+-------------------+
| Analysis             | Domain           | Output            |
+======================+==================+===================+
| DC                   | Steady-state     | Single point      |
+----------------------+------------------+-------------------+
| AC                   | Frequency        | Frequency sweep   |
+----------------------+------------------+-------------------+
| Transient            | Time             | Time trajectory   |
+----------------------+------------------+-------------------+
| Noise                | Frequency        | Noise spectrum    |
+----------------------+------------------+-------------------+
| Transfer Function    | DC small-signal  | Gain & impedances |
+----------------------+------------------+-------------------+

DC Analysis
===========

**Purpose:** Find the DC operating point (steady-state voltages and currents).

**When to use:**

* Calculate bias points
* Verify power supply distribution
* Check diode forward voltages
* Find equilibrium before AC or transient analysis

**Usage:**

.. code-block:: rust

   let mut circuit: CircuitInstance = Circuit::builder("DC Circuit", |b| {
       // ... build circuit
   }).into();
   
   let result = circuit.dc(Context::default())
       .expect("Invalid configuration")
       .solve()
       .expect("Convergence failed");

**Reading Results:**

.. code-block:: rust

   let voltage = result.get_node(&node_id).unwrap();
   let current = result.get_branch(&branch_id).unwrap();

**Algorithm:**

1. Capacitors become open circuits
2. Inductors become short circuits
3. Newton-Raphson iteration until convergence
4. Returns single ``DcOperatingPoint``

**Example:**

.. code-block:: rust

   let mut vout = GND;
   
   let mut circuit: CircuitInstance = Circuit::builder("Divider", |b| {
       let vin = b.port();
       vout = b.port();
       
       b.voltage_source("V1", vin.clone(), GND, 10.0.V());
       b.resistor("R1", vin, vout.clone(), 1.0.kOhms());
       b.resistor("R2", vout.clone(), GND, 1.0.kOhms());
   }).into();
   
   let result = circuit.dc(Context::default())?.solve()?;
   let v = result.get_node(&vout)?;
   
   println!("V_out = {:.4} V", v);  // 5.0000 V

AC Analysis
===========

**Purpose:** Calculate small-signal frequency response.

**When to use:**

* Design filters (low-pass, high-pass, band-pass)
* Find -3dB cutoff frequencies
* Analyze gain and phase vs. frequency
* Verify frequency-domain specifications

**Usage:**

.. code-block:: rust

   let sweep_options = AcSweepOptions::logarithmic(
       1.0.Hz(),      // start frequency
       1.0.MHz(),     // stop frequency
       100            // points per decade
   );
   
   let sweep = circuit.ac(Context::default())?
       .solve_sweep(sweep_options)?;

**Sweep Types:**

* ``AcSweepOptions::logarithmic(start, stop, points_per_decade)`` - Logarithmic sweep
* ``AcSweepOptions::linear(start, stop, num_points)`` - Linear sweep

**Reading Results:**

.. code-block:: rust

   for point in sweep.points() {
       let freq = point.frequency;
       let v_out = point.get_node(&node_id)?;
       
       let magnitude = v_out.norm();  // |V|
       let phase = v_out.arg();        // ∠V in radians
       let db = 20.0 * magnitude.log10();  // dB
   }

**Algorithm:**

1. Find DC operating point
2. Linearize circuit around operating point
3. For each frequency: solve ``(G + jωC)V = I``
4. Returns complex phasors

**Example: RC Low-Pass Filter:**

.. code-block:: rust

   let mut vout = GND;
   
   let mut circuit: CircuitInstance = Circuit::builder("RC Filter", |b| {
       let vin = b.port();
       vout = b.port();
       
       b.voltage_source("Vin", vin.clone(), GND, 1.0.V());
       b.resistor("R1", vin, vout.clone(), 1.0.kOhms());
       b.capacitor("C1", vout.clone(), GND, 10.0.nF());
   }).into();
   
   let options = AcSweepOptions::logarithmic(1.0.Hz(), 1.0.MHz(), 50);
   let sweep = circuit.ac(Context::default())?.solve_sweep(options)?;
   
   for point in sweep.points() {
       let v = point.get_node(&vout)?;
       let gain_db = 20.0 * v.norm().log10();
       println!("{:.2} Hz: {:.2} dB", point.frequency, gain_db);
   }

Transient Analysis
==================

**Purpose:** Simulate circuit behavior over time.

**When to use:**

* Analyze step responses
* Verify rise/fall times
* Simulate switching circuits
* Observe transient behavior

**Usage:**

.. code-block:: rust

   let options = TransientAnalysisOptions {
       stop_time: 1.0.ms(),
       dt: 1.0.us(),
   };
   
   let trajectory = circuit.transient(options, Context::default())?
       .solve()?;

**Reading Results:**

.. code-block:: rust

   // Iterate through time steps
   for step in trajectory.iter() {
       let time = step.time;
       let voltage = step.get_node(&node_id)?;
       println!("{:.6} ms: {:.4} V", time * 1e3, voltage);
   }
   
   // Or get last value
   let final_step = trajectory.last().unwrap();
   let v_final = final_step.get_node(&node_id)?;

**Algorithm:**

1. Uses **adaptive timestep control** with Gear 2nd-order integration
2. Calculates truncation error for each energy-storage element
3. Automatically adjusts timestep to maintain accuracy
4. Includes **breakpoint system** for voltage source transitions

**Features:**

* ✅ Adaptive timestep (43% fewer steps than fixed)
* ✅ Breakpoint capture for Step waveforms
* ✅ Truncation error control (LTE-based)
* ✅ Gear order 2 integration

**Example: RC Step Response:**

.. code-block:: rust

   let mut vout = GND;
   
   let mut circuit: CircuitInstance = Circuit::builder("RC Step", |b| {
       let vin = b.port();
       vout = b.port();
       
       b.voltage_source("Vin", vin.clone(), GND, Step {
           initial: 0.0.V(),
           final_value: 5.0.V(),
           delay: 0.1.ms(),
           rise_time: 1.0.us(),
       });
       b.resistor("R1", vin, vout.clone(), 1.0.kOhms());
       b.capacitor("C1", vout.clone(), GND, 10.0.nF());
   }).into();
   
   let options = TransientAnalysisOptions {
       stop_time: 1.0.ms(),
       dt: 1.0.us(),
   };
   
   let trajectory = circuit.transient(options, Context::default())?.solve()?;
   
   let final_v = trajectory.last().unwrap().get_node(&vout)?;
   println!("Final voltage: {:.4} V", final_v);  // ≈ 5.0 V

Noise Analysis
==============

**Purpose:** Calculate total output noise from all noise sources.

**When to use:**

* Determine circuit noise floor
* Optimize for low-noise design
* Calculate signal-to-noise ratio (SNR)

**Usage:**

.. code-block:: rust

   let options = NoiseAnalysisOptions {
       output: CircuitVariable::Node(output_node),
       input_source: BranchIdentifier::new("Vin"),
   };
   
   let result = circuit.noise(options, Context::default())?
       .solve()?;

**Reading Results:**

.. code-block:: rust

   println!("Total output noise: {:.4e} V/√Hz", result.total_output_noise);
   println!("Total input noise: {:.4e} V/√Hz", result.total_input_noise);

**Algorithm:**

Uses the **adjoint method**:

1. Find DC operating point
2. Linearize circuit
3. Calculate adjoint network
4. Sum noise contributions from all resistors
5. Thermal noise: ``v_n² = 4kTRΔf``

**Example:**

.. code-block:: rust

   let mut vout = GND;
   
   let mut circuit: CircuitInstance = Circuit::builder("Noisy Amplifier", |b| {
       let vin = b.port();
       vout = b.port();
       
       b.voltage_source("Vin", vin.clone(), GND, 1.0.V());
       b.resistor("R1", vin, vout.clone(), 1.0.kOhms());
       b.resistor("R2", vout.clone(), GND, 1.0.kOhms());
   }).into();
   
   let options = NoiseAnalysisOptions {
       output: CircuitVariable::Node(vout),
       input_source: BranchIdentifier::new("Vin"),
   };
   
   let result = circuit.noise(options, Context::default())?.solve()?;
   println!("Output noise: {:.4e} V/√Hz", result.total_output_noise);

Transfer Function Analysis
==========================

**Purpose:** Calculate DC small-signal gain, input resistance, and output resistance.

**When to use:**

* Determine amplifier gain
* Find input/output impedances
* Verify Thévenin/Norton equivalents
* Design impedance matching networks

**Usage:**

.. code-block:: rust

   let options = TransferFunctionAnalysisOptions {
       output: CircuitVariable::Node(output_node),
       output_ref: None,  // or Some(ref_node) for differential
       input_source: BranchIdentifier::new("Vin"),
   };
   
   let result = circuit.transfer_function(options, Context::default())?
       .solve()?;

**Reading Results:**

.. code-block:: rust

   println!("Gain: {:.6}", result.gain);
   println!("Input resistance: {:.2} Ω", result.input_resistance);
   println!("Output resistance: {:.2} Ω", result.output_resistance);
   println!("Type: {:?}", result.tf_type);  // VoltageGain, etc.

**Transfer Function Types:**

* ``VoltageGain`` - V→V (voltage amplifier)
* ``Transconductance`` - V→I (voltage-to-current)
* ``Transresistance`` - I→V (current-to-current)
* ``CurrentGain`` - I→I (current amplifier)

**Algorithm:**

1. Solve DC operating point
2. Build Jacobian matrix (linearization)
3. Apply unit excitation at input (1V or 1A)
4. Solve linearized system
5. Extract gain, R_in, R_out

**Validation:**

Validated against ngspice with <0.1% error on resistive circuits.

**Example:**

.. code-block:: rust

   let mut vout = GND;
   
   let mut circuit: CircuitInstance = Circuit::builder("Divider TF", |b| {
       let vin = b.port();
       vout = b.port();
       
       b.voltage_source("Vin", vin.clone(), GND, 10.0.V());
       b.resistor("R1", vin, vout.clone(), 1.0.kOhms());
       b.resistor("R2", vout.clone(), GND, 1.0.kOhms());
   }).into();
   
   let options = TransferFunctionAnalysisOptions {
       output: CircuitVariable::Node(vout),
       output_ref: None,
       input_source: BranchIdentifier::new("Vin"),
   };
   
   let result = circuit.transfer_function(options, Context::default())?.solve()?;
   
   println!("Gain: {:.6}", result.gain);                  // 0.500000
   println!("R_in: {:.1} Ω", result.input_resistance);    // 2000.0 Ω
   println!("R_out: {:.1} Ω", result.output_resistance);  // 500.0 Ω

Choosing the Right Analysis
============================

**Design Phase:**

1. Start with **DC analysis** to verify bias points
2. Use **Transfer Function** to calculate gain and impedances
3. Use **AC analysis** to verify frequency response
4. Use **Transient** to check step response

**Debugging:**

* Node voltages wrong? → **DC analysis**
* Frequency response off? → **AC analysis**
* Rise time issues? → **Transient analysis**
* Too much noise? → **Noise analysis**

**Optimization:**

* Maximize gain → **Transfer Function**
* Minimize noise → **Noise analysis**
* Shape frequency response → **AC analysis**
* Meet timing specs → **Transient analysis**

See Also
========

* :doc:`../tutorials/dc-analysis` - DC analysis tutorial
* :doc:`../tutorials/ac-analysis` - AC analysis tutorial
* :doc:`../tutorials/transient-analysis` - Transient tutorial
* :doc:`../tutorials/transfer-function` - TF analysis tutorial
* :doc:`solver-context` - Configuring solver behavior
