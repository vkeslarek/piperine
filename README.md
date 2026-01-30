# Piperine

> **⚠️ WARNING: PRE-ALPHA SOFTWARE**
> Piperine is currently in active development. It is **unstable**, features are incomplete, and the API is subject to
> breaking changes at any moment. Do not use in production.

**Piperine** is a modern circuit simulator written in Rust. It is designed to be used primarily as a library—first for
Rust, and eventually for Python—focusing on circuit simulation, design, and optimization.

Piperine is optimized for **medium-sized networks** (efficiently solving grids of ~40,000 nodes). It targets the
complexity range typical of analog design blocks, switching power supplies, and educational contexts, rather than
massive billion-transistor digital verification.

---

## 🚫 Piperine is NOT SPICE

For decades, "SPICE" has evoked a feeling of dread among students and engineers—arcane syntax, cryptic error messages,
and the feeling of "coding in the dark."

**Piperine proposes to change that.**

* **Code, don't list:** Instead of writing text-based netlists, you define circuits using code. This means you get **IDE
  autocompletion**, **compile-time error checking**, and **inline documentation**.
* **Design as Programming:** We aim to make circuit design and optimization feel like modern programming. Loop through
  parameters, apply optimization algorithms directly to your circuit structs, and debug with standard tools.
* **Simple Interface:** Our objective is to make simulation **simple, powerful, and flexible**.

By treating circuits as data structures rather than text files, we aim to win over both professionals needing
optimization loops and students needing clarity.

---

## 🚀 Basic Usage

Piperine allows you to define circuits programmatically using a clean, type-safe builder pattern. Below is a complete example of simulating an RC Low Pass Filter with a step input.

```rust
use piperine::prelude::*;

fn main() {
    // 1. Define the Circuit
    // We use the builder pattern to construct an RC Low Pass Filter.
    // Instead of parsing strings, we write Rust code with compile-time unit checks.
    let mut circuit: Circuit = builder("RC Filter", |b| {
        // Step Input: 0V -> 5V after 0.1ms
        b.voltage_source(
            "Vin", "n_in", GND,
            Step {
                initial: 0.0.V(),
                final_value: 5.0.V(),
                delay: 0.1.ms(),
                rise_time: 1.0.us(),
            },
        );

        // Resistor: 1 kΩ
        b.resistor("R1", "n_in", "n_out", 1.0.kOhms());

        // Capacitor: 10 nF
        b.capacitor("C1", "n_out", GND, 10.0.nF());
    })
    .into();

    // 2. Configure Analysis
    // We run a transient analysis for 1ms with a 1µs timestep.
    let options = TransientAnalysisOptions {
        stop_time: 1.0.ms(),
        dt: 1.0.us(), 
    };

    println!("Starting Transient Analysis...");

    // 3. Solve
    // The solver handles stamping, matrix factorization, and convergence checks.
    let trajectory = circuit
        .transient(options, Context::default())
        .expect("Invalid configuration")
        .solve()
        .expect("Convergence failed");

    // 4. Analyze Results
    // You can iterate through steps or jump to the end.
    let final_step = trajectory.last().unwrap();
    let v_out = final_step.get_node("n_out").unwrap_or(0.0);

    println!("Simulation Complete!");
    println!("Final Output Voltage: {:.4} V", v_out);
}
```

---

## 🗺️ Roadmap

Piperine is evolving in phases. Below is the current development plan.

### Phase 1: Core Device Physics (The "Textbook" Suite)

*Goal: Validate the solver architecture against standard SPICE primitives.*

- [ ] **MOSFET Level 1 (Shichman-Hodges)**
    - *Why:* The fundamental active device. Tests 4-terminal stamping and basic non-linear convergence.
- [ ] **Transmission Lines (Lossless T-Line)**
    - *Why:* Introduces Time Delay. Tests the transient history buffer (looking back at `t - delay`).
- [ ] **Behavioral Sources (B-Sources / Expressions)**
    - *Why:* Allows arbitrary math (e.g., `V = sin(time) * V(1)`). Requires implementing a math expression parser.

### Phase 2: Solver Hardening (The "Robustness" Suite)

*Goal: Make the simulator capable of handling switching circuits and stiff systems.*

- [ ] **Adaptive Timestepping (LTE Control)**
    - *Why:* Essential for speed and accuracy. Takes large steps when idle, tiny steps during transients.
- [ ] **Switches (Voltage/Current Controlled)**
    - *Why:* Introduces discontinuities. Requires "Breakpoint" handling so the solver hits the exact switching moment
      without stepping over it.

### Phase 3: Linear & Frequency Analysis (The "Small Signal" Suite)

*Goal: Implement analyses that require linearizing the circuit around an operating point.*

- [ ] **Transfer Function (TF)**
    - *Why:* Calculates DC small-signal gain, input resistance, and output resistance.
- [ ] **Pole-Zero Analysis (PZ)**
    - *Why:* Stability analysis (Control theory). Finds the roots of the network determinant.
- [ ] **Noise Analysis**
    - *Why:* Sums thermal, shot, and flicker noise contributions from all devices to find the noise floor.

### Phase 4: Advanced Analysis Loops (The "Expert" Suite)

*Goal: Wrappers that run the standard solver multiple times.*

- [ ] **Fourier Analysis (FFT)**
    - *Why:* Measure THD (Total Harmonic Distortion). A post-processing step on transient data.
- [ ] **Sensitivity & Monte Carlo**
    - *Why:* Manufacturing tolerances. "How does V(out) change if R1 varies by 1%?"
- [ ] **Periodic Steady State (PSS)**
    - *Why:* RF and Switching Power Supplies. Uses the Shooting Method to find the steady state of periodic signals.

### Phase 5: Advanced Device Library (The "Industry" Suite)

*Goal: Implement the massive, complex models used in real chip design.*

- [ ] **BSIM Models (BSIM3 / BSIM4)**
    - *Why:* The industry standard for deep-submicron MOSFET simulation.
- [ ] **Advanced BJT Models (VBIC / HICUM / Mextram)**
    - *Why:* Necessary for SiGe and RF BJT design where Gummel-Poon fails.
- [ ] **Compound Devices**
    - *URC:* Uniform Distributed RC for on-chip interconnects.
    - *Coupled Transmission Lines:* For crosstalk modeling.

### Phase 6: Interface

- [ ] **Python Bindings (PyO3)**
    - *Why:* User scripting and GUI integration. `import piperine`.

---

## 🤝 Contributing & Future

Piperine is an ambitious project. Its future is not yet certain, but the goal is clear: to modernize the toolchain for
circuit simulation. If you are interested in Rust, numerical methods, or analog design, feedback and contributions are
welcome.

---

## 📜 License

Piperine is intentionally released under the **MIT License**.

This is a permissive license that allows you to do almost anything with this code:

* **Use it** for private, educational, or commercial projects.
* **Modify it** to suit your needs.
* **Distribute it** in your own applications (even closed-source ones).
* **Sub-license it** as part of a larger work.

**My Intent:**
I chose this license because I want to remove barriers to entry. Whether you are a student exploring numerical analysis,
a researcher testing new algorithms, or a developer building a commercial tool, you should be able to use Piperine
freely without legal friction. I believe open tools foster better engineering for everyone.

**Author:** Vinicius Blasio Keslarek
