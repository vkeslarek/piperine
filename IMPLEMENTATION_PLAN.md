# Piperine - Plano de Implementacao

> NgSpice wrapper em Rust: circuitos como codigo, workers paralelos, API ergonomica.

## Visao Geral da Arquitetura

```text
                          piperine (bin/lib)
                               |
               +---------------+----------------+
               |               |                |
          piperine-core   piperine-ngspice   piperine-pool
          (circuit DSL,   (FFI bindings,     (worker pool,
           netlist gen,    safe wrapper)      job dispatch)
           units, etc.)
```

**Decisao chave**: Usar `libngspice.so` via FFI (nao processo externo).
Cada worker e um **processo separado** (re-exec com `--worker`) porque ngspice
usa globals internos e nao e thread-safe. Comunicacao via stdin/stdout JSON.

---

## Fase 0 - Fundacao: FFI Bindings + Build System

**Objetivo**: Compilar, linkar com libngspice, gerar bindings, ter um "hello world" que roda `op`.

### 0.1 Estrutura do workspace

```
piperine/
  Cargo.toml              (workspace root)
  header/
    sharedspice.h         (copiado do ngspice source)
    wrapper.h             (#include "sharedspice.h")
  crates/
    piperine-ngspice/     (FFI bindings crate - -sys style)
      Cargo.toml
      build.rs            (bindgen + link ngspice)
      src/lib.rs          (raw bindings re-export)
    piperine-core/        (circuit builder, netlist, units)
      Cargo.toml
      src/lib.rs
    piperine-pool/        (worker pool + IPC)
      Cargo.toml
      src/lib.rs
  src/
    main.rs               (CLI / demo binary)
```

### 0.2 build.rs (piperine-ngspice)

- Usar `bindgen` para gerar bindings de `sharedspice.h`
- `cargo:rustc-link-lib=ngspice`
- Allowlist: `ngSpice_*`, `ngGet_*`, `ngCM_*`, structs `vecvalues*`, `vector_info*`, `vecinfo*`, `vecinfoall*`
- Ja existe referencia no `best_so_far` branch

### 0.3 Safe wrapper sobre o FFI

```rust
// crates/piperine-ngspice/src/lib.rs
pub mod ffi;  // raw bindings

// crates/piperine-ngspice/src/instance.rs
pub struct NgspiceInstance {
    // Callbacks registrados, estado de init
}

impl NgspiceInstance {
    pub fn new() -> Result<Self>;           // chama ngSpice_Init
    pub fn command(&self, cmd: &str) -> Result<()>;  // ngSpice_Command
    pub fn load_netlist(&self, lines: &[&str]) -> Result<()>;  // ngSpice_Circ
    pub fn is_running(&self) -> bool;       // ngSpice_running
    pub fn get_vector(&self, name: &str) -> Result<VectorData>;  // ngGet_Vec_Info
    pub fn current_plot(&self) -> String;   // ngSpice_CurPlot
    pub fn all_plots(&self) -> Vec<String>; // ngSpice_AllPlots
    pub fn all_vecs(&self, plot: &str) -> Vec<String>;  // ngSpice_AllVecs
    pub fn reset(&self) -> Result<()>;      // ngSpice_Reset (novo init necessario)
}
```

**Callbacks a implementar** (Ch. 15.3.3 do manual):
- `SendChar` - captura stdout/stderr do ngspice
- `SendStat` - progresso da simulacao (ex: "tran 34.5%")
- `ControlledExit` - ngspice quer sair (obrigatorio)
- `SendData` - dados de simulacao em tempo real
- `SendInitData` - metadados dos vetores antes da simulacao
- `BGThreadRunning` - flag de thread rodando

### 0.4 Entregavel

```rust
fn main() {
    let ng = NgspiceInstance::new().unwrap();
    ng.load_netlist(&[
        "Test",
        "V1 in 0 DC 5",
        "R1 in out 1k",
        "R2 out 0 1k",
        ".op",
        ".end",
    ]).unwrap();
    ng.command("run").unwrap();
    let v = ng.get_vector("out").unwrap();
    println!("V(out) = {}", v.real_data()[0]); // 2.5
}
```

---

## Fase 1 - Worker Pool

**Objetivo**: Pool de N processos ngspice, despacho de jobs, comunicacao JSON.

### 1.1 Protocolo IPC (JSON via stdin/stdout)

```rust
// crates/piperine-pool/src/protocol.rs
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Request {
    RunSimulation { netlist: Vec<String>, command: String },
    LoadNetlist { lines: Vec<String> },
    RunCommand { command: String },
    GetResults,
    GetVector { name: String },
    Reset,
    Shutdown,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    SimulationComplete { plots: HashMap<String, PlotData> },
    VectorData { name: String, data: VectorPayload },
    Error { message: String },
    Ok,
}

pub struct PlotData {
    pub name: String,
    pub title: String,
    pub plot_type: String,
    pub vectors: HashMap<String, VectorPayload>,
}

pub enum VectorPayload {
    Real(Vec<f64>),
    Complex(Vec<(f64, f64)>),  // (real, imag)
}
```

### 1.2 Worker Process

```rust
// Binario se re-executa com --worker
// crates/piperine-pool/src/worker.rs
pub fn worker_main() -> Result<()> {
    let ng = NgspiceInstance::new()?;
    // Loop: ler Request de stdin, processar, escrever Response em stdout
    // stderr usado para logs (tracing)
}
```

### 1.3 Pool Manager

```rust
// crates/piperine-pool/src/pool.rs
pub struct WorkerPool {
    workers: Vec<WorkerHandle>,
    // Round-robin ou least-busy dispatch
}

impl WorkerPool {
    pub fn new(size: usize) -> Result<Self>;
    pub fn with_default_size() -> Result<Self>;  // num_cpus

    /// Submete uma simulacao e retorna resultado
    pub async fn submit(&self, job: SimulationJob) -> Result<SimulationResult>;

    /// Submete batch de simulacoes em paralelo
    pub async fn submit_batch(&self, jobs: Vec<SimulationJob>) -> Vec<Result<SimulationResult>>;

    pub fn shutdown(self);
}
```

### 1.4 Escolha sync vs async

- **Fase 1**: API sincrona (blocking) - mais simples, menos dependencias
- **Fase futura**: Opcionalmente adicionar feature `async` com tokio
- Workers internamente usam threads para I/O com processos filhos

---

## Fase 2 - Circuit Builder DSL (Core)

**Objetivo**: API que faz desenhar circuitos parecer programacao.

### 2.1 Sistema de Unidades

```rust
// crates/piperine-core/src/units.rs

/// Trait para valores com sufixo de engenharia
pub trait EngineeringNotation {
    fn T(self) -> f64;    // tera  1e12
    fn G(self) -> f64;    // giga  1e9
    fn M(self) -> f64;    // mega  1e6
    fn k(self) -> f64;    // kilo  1e3
    fn m(self) -> f64;    // milli 1e-3
    fn u(self) -> f64;    // micro 1e-6
    fn n(self) -> f64;    // nano  1e-9
    fn p(self) -> f64;    // pico  1e-12
    fn f(self) -> f64;    // femto 1e-15
}

impl EngineeringNotation for f64 { /* ... */ }
impl EngineeringNotation for i64 { /* ... */ }

// Uso: 4.7.k() => 4700.0, 100.n() => 100e-9
```

### 2.2 Nodes

```rust
// crates/piperine-core/src/node.rs

/// Um no do circuito
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum Node {
    Ground,              // "0"
    Named(String),       // "in", "out", "vcc"
    Internal(usize),     // auto-gerado para subcircuits
}

pub const GND: Node = Node::Ground;

// Qualquer &str, String, usize converte para Node
impl From<&str> for Node { /* ... */ }
impl From<usize> for Node { /* ... */ }
```

### 2.3 Circuit Builder

```rust
// crates/piperine-core/src/circuit.rs
use crate::node::Node;

pub struct Circuit {
    title: String,
    elements: Vec<Element>,
    models: Vec<Model>,
    subcircuits: Vec<SubCircuit>,
    params: Vec<(String, String)>,      // .param
    options: Vec<(String, String)>,     // .options
    analyses: Vec<Analysis>,
    saves: Vec<String>,                 // .save
    initial_conditions: Vec<(String, f64)>,  // .ic
}

impl Circuit {
    pub fn new(title: &str) -> Self;

    // === Elementos passivos ===
    pub fn resistor(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                    value: f64) -> &mut Self;
    pub fn capacitor(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                     value: f64) -> &mut Self;
    pub fn inductor(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                    value: f64) -> &mut Self;

    // === Fontes independentes ===
    pub fn vdc(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
               voltage: f64) -> &mut Self;
    pub fn idc(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
               current: f64) -> &mut Self;
    pub fn vpulse(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                  pulse: Pulse) -> &mut Self;
    pub fn vsin(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                sin: Sinusoidal) -> &mut Self;
    pub fn vpwl(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                points: &[(f64, f64)]) -> &mut Self;

    // === Fontes dependentes ===
    pub fn vcvs(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                cp: impl Into<Node>, cn: impl Into<Node>, gain: f64) -> &mut Self;
    pub fn vccs(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                cp: impl Into<Node>, cn: impl Into<Node>, gm: f64) -> &mut Self;
    pub fn ccvs(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                vsource: &str, transresistance: f64) -> &mut Self;
    pub fn cccs(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                vsource: &str, gain: f64) -> &mut Self;

    // === Behavioral sources (Ch. 5) ===
    pub fn bsource_v(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                     expr: &str) -> &mut Self;
    pub fn bsource_i(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                     expr: &str) -> &mut Self;

    // === Semicondutores ===
    pub fn diode(&mut self, name: &str, anode: impl Into<Node>, cathode: impl Into<Node>,
                 model: &str) -> &mut Self;
    pub fn bjt_npn(&mut self, name: &str, c: impl Into<Node>, b: impl Into<Node>,
                   e: impl Into<Node>, model: &str) -> &mut Self;
    pub fn bjt_pnp(&mut self, name: &str, c: impl Into<Node>, b: impl Into<Node>,
                   e: impl Into<Node>, model: &str) -> &mut Self;
    pub fn mosfet_n(&mut self, name: &str, d: impl Into<Node>, g: impl Into<Node>,
                    s: impl Into<Node>, b: impl Into<Node>, model: &str) -> &mut Self;
    pub fn mosfet_p(&mut self, name: &str, d: impl Into<Node>, g: impl Into<Node>,
                    s: impl Into<Node>, b: impl Into<Node>, model: &str) -> &mut Self;
    pub fn jfet_n(&mut self, name: &str, d: impl Into<Node>, g: impl Into<Node>,
                  s: impl Into<Node>, model: &str) -> &mut Self;

    // === Switches ===
    pub fn switch_v(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                    cp: impl Into<Node>, cn: impl Into<Node>, model: &str) -> &mut Self;
    pub fn switch_i(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>,
                    vsource: &str, model: &str) -> &mut Self;

    // === Transmission lines ===
    pub fn tline(&mut self, name: &str, p1: impl Into<Node>, n1: impl Into<Node>,
                 p2: impl Into<Node>, n2: impl Into<Node>, z0: f64, td: f64) -> &mut Self;

    // === Coupled inductors ===
    pub fn mutual_inductor(&mut self, name: &str, l1: &str, l2: &str,
                           coupling: f64) -> &mut Self;

    // === Models ===
    pub fn model(&mut self, name: &str, kind: ModelKind,
                 params: &[(&str, f64)]) -> &mut Self;

    // === Subcircuit usage ===
    pub fn subcircuit_instance(&mut self, name: &str, subckt: &str,
                               nodes: &[impl Into<Node>]) -> &mut Self;

    // === Parameters & Options ===
    pub fn param(&mut self, name: &str, value: &str) -> &mut Self;
    pub fn option(&mut self, name: &str, value: &str) -> &mut Self;
    pub fn ic(&mut self, node: &str, voltage: f64) -> &mut Self;
    pub fn save(&mut self, what: &str) -> &mut Self;
    pub fn include(&mut self, path: &str) -> &mut Self;
    pub fn lib(&mut self, path: &str, section: &str) -> &mut Self;

    // === Analises ===
    pub fn op(&mut self) -> &mut Self;
    pub fn dc_sweep(&mut self, source: &str, start: f64, stop: f64,
                    step: f64) -> &mut Self;
    pub fn ac(&mut self, variation: AcVariation, npoints: usize,
              fstart: f64, fstop: f64) -> &mut Self;
    pub fn tran(&mut self, tstep: f64, tstop: f64) -> &mut Self;
    pub fn tran_full(&mut self, tstep: f64, tstop: f64, tstart: f64,
                     tmax: f64, uic: bool) -> &mut Self;
    pub fn noise(&mut self, output: &str, src: &str, variation: AcVariation,
                 npoints: usize, fstart: f64, fstop: f64) -> &mut Self;
    pub fn tf(&mut self, output: &str, source: &str) -> &mut Self;
    pub fn pz(&mut self, /* ... */) -> &mut Self;
    pub fn sens(&mut self, output: &str) -> &mut Self;
    pub fn disto(&mut self, /* ... */) -> &mut Self;
    pub fn sp(&mut self, variation: AcVariation, npoints: usize,
              fstart: f64, fstop: f64) -> &mut Self;

    // === .meas (Ch. 11.4) ===
    pub fn meas_tran(&mut self, name: &str, expr: &str) -> &mut Self;
    pub fn meas_ac(&mut self, name: &str, expr: &str) -> &mut Self;
    pub fn meas_dc(&mut self, name: &str, expr: &str) -> &mut Self;

    // === Gerar netlist ===
    pub fn to_netlist(&self) -> Vec<String>;
    pub fn to_string(&self) -> String;
}
```

### 2.4 Waveforms (Ch. 4.1)

```rust
// crates/piperine-core/src/waveform.rs

pub struct Pulse {
    pub v1: f64,        // initial value
    pub v2: f64,        // pulsed value
    pub td: f64,        // delay time
    pub tr: f64,        // rise time
    pub tf: f64,        // fall time
    pub pw: f64,        // pulse width
    pub per: f64,       // period
    pub np: Option<u32>, // number of pulses
}

pub struct Sinusoidal {
    pub offset: f64,
    pub amplitude: f64,
    pub freq: f64,
    pub delay: Option<f64>,
    pub damping: Option<f64>,
    pub phase: Option<f64>,
}

pub struct Exponential {
    pub v1: f64,
    pub v2: f64,
    pub td1: f64,
    pub tau1: f64,
    pub td2: f64,
    pub tau2: f64,
}

pub enum AcVariation {
    Dec,  // decades
    Oct,  // octaves
    Lin,  // linear
}

pub enum ModelKind {
    R, C, L,
    D,              // diode
    NPN, PNP,       // BJT
    NJF, PJF,       // JFET
    NMOS, PMOS,     // MOSFET
    VDMOS,          // power MOSFET
    SW, CSW,        // switches
}
```

### 2.5 Exemplo de uso (Circuit Builder)

```rust
use piperine_core::prelude::*;

let mut ckt = Circuit::new("Voltage Divider");
ckt.vdc("V1", "in", GND, 10.0)
   .resistor("R1", "in", "out", 1.0.k())
   .resistor("R2", "out", GND, 1.0.k())
   .op();

// Gera:
// Voltage Divider
// V1 in 0 DC 10
// R1 in out 1000
// R2 out 0 1000
// .op
// .end
```

---

## Fase 3 - SubCircuits e Reuso

**Objetivo**: Composicao de circuitos, subcircuitos como funcoes, parametrizacao.

### 3.1 SubCircuit Definition

```rust
// crates/piperine-core/src/subcircuit.rs

pub struct SubCircuit {
    name: String,
    ports: Vec<String>,           // nos externos
    params: Vec<(String, String)>, // parametros com defaults
    body: Circuit,                 // circuito interno
}

impl SubCircuit {
    pub fn new(name: &str, ports: &[&str]) -> Self;
    pub fn with_param(mut self, name: &str, default: &str) -> Self;

    // Acesso ao circuit builder interno
    pub fn circuit(&mut self) -> &mut Circuit;

    // Gera .SUBCKT ... .ENDS
    pub fn to_netlist(&self) -> Vec<String>;
}
```

### 3.2 Exemplo - OpAmp como SubCircuit

```rust
fn opamp_ideal() -> SubCircuit {
    let mut sub = SubCircuit::new("opamp_ideal", &["inp", "inn", "out"])
        .with_param("gain", "100000")
        .with_param("rin", "1e12")
        .with_param("rout", "0.1");

    let c = sub.circuit();
    c.resistor("Rin", "inp", "inn", "{rin}")
     .bsource_v("Eout", "out_int", GND, "V(inp,inn) * {gain}")
     .resistor("Rout", "out_int", "out", "{rout}");

    sub
}

// Usar:
let mut ckt = Circuit::new("Inverting Amplifier");
ckt.register_subcircuit(opamp_ideal());
ckt.vdc("V1", "in", GND, 1.0)
   .resistor("R1", "in", "mid", 10.0.k())
   .resistor("R2", "mid", "out", 100.0.k())
   .subcircuit_instance("X1", "opamp_ideal", &["mid", GND, "out"])
   .op();
```

### 3.3 Library de Componentes

```rust
// crates/piperine-core/src/library/mod.rs
pub mod opamps;
pub mod regulators;
pub mod filters;
pub mod common_models;  // modelos de diodos, transistores comuns

// Exemplo: modelos predefinidos
pub fn model_1n4148() -> Model { /* .model 1N4148 D(Is=2.52e-9 ...) */ }
pub fn model_2n2222() -> Model { /* .model 2N2222 NPN(Is=1e-14 ...) */ }
```

---

## Fase 4 - Simulation Engine (integracao pool + circuit)

**Objetivo**: API completa run-and-get-results.

### 4.1 Simulator

```rust
// crates/piperine-pool/src/simulator.rs (ou top-level piperine)

pub struct Simulator {
    pool: WorkerPool,
}

impl Simulator {
    pub fn new() -> Result<Self>;
    pub fn with_workers(n: usize) -> Result<Self>;

    /// Roda um circuito e retorna resultados
    pub fn run(&self, circuit: &Circuit) -> Result<SimulationResult>;

    /// Roda batch de circuitos em paralelo
    pub fn run_batch(&self, circuits: &[Circuit]) -> Vec<Result<SimulationResult>>;

    /// Roda Monte Carlo: mesmo circuito N vezes com parametros variados
    pub fn monte_carlo(&self, circuit: &Circuit, variations: &[Variation],
                       runs: usize) -> Result<MonteCarloResult>;

    /// Roda parameter sweep
    pub fn sweep(&self, circuit: &Circuit, param: &str,
                 values: &[f64]) -> Result<SweepResult>;
}
```

### 4.2 Resultados Tipados

```rust
// crates/piperine-core/src/result.rs

pub struct SimulationResult {
    pub plots: HashMap<String, Plot>,
    pub measurements: HashMap<String, f64>,  // resultados de .meas
    pub log: Vec<String>,
}

pub struct Plot {
    pub name: String,
    pub plot_type: PlotType,
    pub vectors: HashMap<String, Vector>,
}

pub enum PlotType { OpPoint, DcSweep, AcAnalysis, Transient, Noise, /* ... */ }

pub enum Vector {
    Real(RealVector),
    Complex(ComplexVector),
}

pub struct RealVector {
    pub name: String,
    pub data: Vec<f64>,
    pub scale: Option<String>,  // nome do vetor de escala (ex: "time", "frequency")
}

pub struct ComplexVector {
    pub name: String,
    pub data: Vec<(f64, f64)>,  // (real, imag)
    pub scale: Option<String>,
}

impl SimulationResult {
    /// Acessa vetor por nome
    pub fn vector(&self, name: &str) -> Option<&Vector>;

    /// Acessa valor DC (para .op)
    pub fn dc_value(&self, node: &str) -> Option<f64>;

    /// Acessa vetor real
    pub fn real_vector(&self, name: &str) -> Option<&[f64]>;

    /// Magnitude (para AC)
    pub fn magnitude(&self, name: &str) -> Option<Vec<f64>>;

    /// Phase (para AC)
    pub fn phase_deg(&self, name: &str) -> Option<Vec<f64>>;
}
```

### 4.3 Exemplo Completo

```rust
use piperine::prelude::*;

fn main() -> Result<()> {
    let sim = Simulator::new()?;

    // ---- RC Low-Pass Filter ----
    let mut ckt = Circuit::new("RC Low-Pass");
    ckt.vdc("Vin", "in", GND, 1.0)
       .resistor("R1", "in", "out", 1.0.k())
       .capacitor("C1", "out", GND, 1.0.u())
       .ac(AcVariation::Dec, 100, 1.0, 1.0.M());

    let result = sim.run(&ckt)?;

    // fc = 1/(2*pi*R*C) = ~159 Hz
    let freq = result.real_vector("frequency").unwrap();
    let gain_db: Vec<f64> = result.magnitude("v(out)")
        .unwrap()
        .iter()
        .map(|m| 20.0 * m.log10())
        .collect();

    for (f, g) in freq.iter().zip(gain_db.iter()) {
        println!("{:.1} Hz -> {:.2} dB", f, g);
    }

    Ok(())
}
```

---

## Fase 5 - Monte Carlo e Otimizacao

**Objetivo**: Aproveitar os workers para simulacoes estatisticas e sweeps em paralelo.

### 5.1 Monte Carlo com Workers Paralelos

```rust
// A ideia: gerar N netlists com parametros variados,
// despachar em paralelo para os workers

pub struct Variation {
    pub param: String,
    pub distribution: Distribution,
}

pub enum Distribution {
    Gaussian { mean: f64, std_dev: f64, sigma: f64 },
    Uniform { min: f64, max: f64 },
    /// Usa as funcoes built-in do ngspice (gauss, agauss, unif, aunif, limit)
    NgspiceBuiltin(String),
}

pub struct MonteCarloResult {
    pub runs: Vec<SimulationResult>,
    pub statistics: HashMap<String, Statistics>,
}

pub struct Statistics {
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
    pub percentiles: HashMap<u8, f64>,  // p5, p50, p95 etc.
}
```

### 5.2 Parameter Sweep (paralelo)

```rust
let mut ckt = Circuit::new("Gain vs R2");
ckt.vdc("V1", "in", GND, 1.0)
   .resistor("R1", "in", "mid", 10.0.k())
   .resistor("R2", "mid", "out", "{r2_val}")
   .subcircuit_instance("X1", "opamp_ideal", &["mid", "out", "out"])
   .param("r2_val", "100k")
   .op();

let sweep = sim.sweep(
    &ckt,
    "r2_val",
    &[10.0.k(), 50.0.k(), 100.0.k(), 500.0.k(), 1.0.M()]
)?;

for (val, result) in sweep.iter() {
    println!("R2={:.0} -> Gain={:.1}", val, result.dc_value("out").unwrap());
}
```

### 5.3 Otimizacao (futuro)

```rust
// Simples hill-climbing ou integracao com crate de otimizacao
pub struct OptimizationTarget {
    pub metric: String,         // ex: "v(out)"
    pub goal: OptGoal,
}

pub enum OptGoal {
    Minimize,
    Maximize,
    Target(f64),                // atingir valor especifico
    Range(f64, f64),            // manter dentro de faixa
}
```

---

## Fase 6 - Features Avancadas

### 6.1 Suporte a .PROBE / .SAVE (Ch. 11.6.5)

```rust
// Probe: salva apenas vetores especificos (economia de memoria)
ckt.save("v(out)")
   .save("i(V1)")
   .save("@R1[p]");  // potencia do resistor
```

### 6.2 .MEAS automatico (Ch. 11.4)

```rust
// Medicoes automaticas nos resultados
ckt.tran(1.0.n(), 100.0.u())
   .meas_tran("rise_time", "TRIG v(out) VAL=0.1 RISE=1 TARG v(out) VAL=0.9 RISE=1")
   .meas_tran("overshoot", "MAX v(out)")
   .meas_tran("delay", "FIND v(out) AT=50u");

let result = sim.run(&ckt)?;
let rise = result.measurements["rise_time"];
```

### 6.3 Corner Analysis

```rust
pub struct Corner {
    pub name: String,
    pub params: HashMap<String, f64>,
    pub temp: f64,
}

pub fn typical_corners() -> Vec<Corner> {
    vec![
        Corner { name: "TT".into(), temp: 27.0, /* ... */ },
        Corner { name: "FF".into(), temp: -40.0, /* ... */ },
        Corner { name: "SS".into(), temp: 125.0, /* ... */ },
        Corner { name: "FS".into(), temp: 27.0, /* ... */ },
        Corner { name: "SF".into(), temp: 27.0, /* ... */ },
    ]
}

let results = sim.run_corners(&ckt, &typical_corners())?;
```

### 6.4 Temperatura (Ch. 1.3)

```rust
ckt.option("temp", "85")    // temperatura global
   .option("tnom", "27");    // temperatura nominal dos modelos
```

### 6.5 Convergencia helpers (Ch. 1.4)

```rust
// Presets para ajudar convergencia
ckt.options_convergence_relaxed()  // reltol=0.01, abstol=1e-10, vntol=1e-4, gmin=1e-10
   .option("method", "gear")       // integracao numerica
   .option("maxord", "3");
```

### 6.6 Importar netlists existentes

```rust
// Carregar .cir/.sp de arquivo
let ckt = Circuit::from_file("path/to/circuit.cir")?;

// Ou incluir dentro de outro circuito
ckt.include("models/transistors.lib")
   .lib("models/process.lib", "tt");
```

---

## Ordem de Implementacao

| Fase | O que | Depende de | Complexidade |
|------|-------|------------|-------------|
| **0** | FFI bindings + safe wrapper | nada | Media |
| **1** | Worker pool + IPC | Fase 0 | Media |
| **2** | Circuit builder DSL + units | nada (paralelo) | Media |
| **3** | SubCircuits + library | Fase 2 | Baixa |
| **4** | Simulator (pool + circuit) | Fase 1 + 2 | Baixa |
| **5** | Monte Carlo + sweep + otim | Fase 4 | Media |
| **6** | Features avancadas | Fase 4 | Variavel |

**Fases 0 e 2 podem ser desenvolvidas em paralelo.**

---

## Dependencias do Cargo.toml (workspace)

```toml
[workspace.dependencies]
# FFI
bindgen = "0.72"
libc = "0.2"

# Serialization (IPC)
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "2.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Utilities
num_cpus = "1.16"
```

---

## Decisoes de Design

1. **Processo separado (nao thread)**: ngspice usa globais C, nao e thread-safe. Cada worker e um processo com sua propria instancia de libngspice.

2. **JSON via stdin/stdout**: Simples, debugavel, sem dependencia de IPC complexo. Performance nao e gargalo (simulacao e ordens de magnitude mais lenta que serialization).

3. **Circuit builder retorna `&mut Self`**: Permite chaining fluente. Alternativa seria builder pattern com `.build()`, mas chaining e mais natural para circuitos.

4. **Unidades como extension traits em f64**: `1.0.k()` e limpo e Rustico. Alternativa seria newtype wrappers (`Ohm(1000.0)`) mas adiciona verbosidade sem beneficio real (ngspice so entende numeros).

5. **Netlist como Vec<String>**: O circuito gera linhas de netlist SPICE. Isso e o que ngspice consume. Nao tentamos representar o circuito de outra forma - ngspice e o motor.

6. **Parametros como strings**: `.param("res", "10k * 1.05")` permite expressoes ngspice nativas. Nao tentamos parsear/avaliar expressoes - ngspice faz isso melhor.

---

## Proximos Passos

Comecar pela **Fase 0**: copiar o `sharedspice.h` do `best_so_far`, configurar build.rs, gerar bindings, e fazer o primeiro `op` funcionar via libngspice.
