# Piperine ↔ ngspice Integration Plan

Full coverage of every documented ngspice feature. Organized by phase (maps onto DESIGN.md §10 roadmap). Each item: what ngspice provides → what Piperine needs → implementation area.

---

## Phase 3 — "Working System" Completions (P0/P1 Gaps)

These block any real testbench. Must ship before Phase 3 milestone.

### 3A. Analyses

| Task | ngspice | Piperine syntax | Area |
|------|---------|-----------------|------|
| `$dc` | `dc src start stop step [src2 ...]` | `$dc("v1", 0, 5, 0.01)` | ngspice-tasks |
| `$ac` | `ac dec\|oct\|lin np fstart fstop` | `$ac("dec", 20, 1, 1e9)` | ngspice-tasks |
| `$tran` full | `tran tstep tstop [tstart [tmax]] [uic]` | `$tran(1n, 5m)` — emit `.tran` into netlist pre-load, or use `tran tstep tstop` command | ngspice-tasks + elaborator |
| `$noise` | `noise v(out) src dec np f1 f2` | `$noise("v(out)", "v1", "dec", 20, 1, 1e9)` | ngspice-tasks |
| `$tf` | `tf outvar inputsrc` | `$tf("v(out)", "v1")` | ngspice-tasks |
| `$sens` | `sens outvar` / `sens outvar ac ...` | `$sens("v(out)")` / `$sens_ac(...)` | ngspice-tasks |

**Note on `$tran`:** current impl sends `run` against a pre-declared `.tran` card. Long-term: `$tran(tstep, tstop)` dynamically replaces the `.tran` card via `alter` or re-emits netlist. Short-term: document the constraint.

### 3B. Waveform Sources on V/I

Current `spice_vsource` only emits `DC val`. Need:

```verilog
// AC stimulus (required for any $ac test)
extern module spice_vsource(
    inout p, inout n;
    parameter real dc    = 0.0;
    parameter real acmag = 0.0;
    parameter real acphase = 0.0;
    // transient waveform — one of these (all default to none):
    parameter string waveform = "";   // "PULSE", "SIN", "EXP", "PWL", "SFFM", "AM"
    parameter real v1 = 0;   // waveform param 1
    parameter real v2 = 0;   // waveform param 2
    parameter real td = 0;
    parameter real tr = 0;
    parameter real tf = 0;
    parameter real pw = 0;
    parameter real per = 0;
    // SIN extras
    parameter real freq = 0;
    parameter real theta = 0;
    parameter real phase = 0;
);
```

Elaborator serializes to ngspice SPICE line:
```
V1 n+ n- DC <dc> AC <acmag> <acphase> PULSE(<v1> <v2> <td> <tr> <tf> <pw> <per>)
```

Alternatively, separate extern modules per waveform type (`spice_vsource_pulse`, `spice_vsource_sin`, etc.) — cleaner parameter names, less coupling.

**PWL special case:** needs array parameter. Options:
- `parameter real t[] = {}; parameter real v[] = {};` (requires parser + elaborator array param support)
- Pass as string: `parameter string pwl_data = "0 0 1n 5 2n 0"` (hack but works short-term)

### 3C. Device Components

All require `.model` card emission from elaborator when `model` parameter is present.

#### Inductor + Mutual
```verilog
extern module spice_ind(inout p, inout n; parameter real l = 1e-6; parameter real ic = 0.0);
extern module spice_mutual(parameter string l1; parameter string l2; parameter real k = 0.5);
```
SPICE: `L1 p n 1e-6` / `K1 L1 L2 0.5`

#### Diode
```verilog
extern module spice_diode(inout a, inout c; parameter string model; parameter real area = 1.0; parameter real m = 1.0);
```
SPICE: `D1 a c modelname area=1.0` + `.model modelname D [params]`

Model params forwarded via `paramset`:
```verilog
paramset d1n4148 spice_diode;
  .model = "d1n4148_model";
  .is = 2.52e-9; .n = 1.752; .bv = 75; .ibv = 1e-8; ...
endparamset
```
Elaborator emits `.model d1n4148_model D(is=2.52e-9 n=1.752 bv=75 ibv=1e-8 ...)`.

#### BJT
```verilog
extern module spice_bjt(inout c, inout b, inout e; parameter string model; parameter string type = "npn"; parameter real area = 1.0);
```
SPICE: `Q1 c b e modelname area=1.0`

#### MOSFET
```verilog
extern module spice_mos(
    inout d, inout g, inout s, inout b;
    parameter string model;
    parameter real l = 1e-6;
    parameter real w = 1e-6;
    parameter real ad = 0; parameter real as = 0;
    parameter real pd = 0; parameter real ps = 0;
    parameter real nrd = 1; parameter real nrs = 1;
    parameter real m = 1;
    // long tail forwarded as-is to ngspice
);
```
SPICE: `M1 d g s b modelname L=1e-6 W=1e-6 ...`

#### VCVS / VCCS (E/G)
```verilog
extern module spice_vcvs(inout p, inout n, inout cp, inout cn; parameter real gain = 1.0);
extern module spice_vccs(inout p, inout n, inout cp, inout cn; parameter real gm = 1.0);
```
SPICE: `E1 p n cp cn gain` / `G1 p n cp cn gm`

#### CCCS / CCVS (F/H)
```verilog
extern module spice_cccs(inout p, inout n; parameter string vsrc; parameter real gain = 1.0);
extern module spice_ccvs(inout p, inout n; parameter string vsrc; parameter real transres = 1.0);
```

#### Behavioral Source (B)
```verilog
// AST-passthrough: expression lowered to ngspice B-source V=... or I=... string
extern module spice_bsource_v(inout p, inout n; parameter expr V);
extern module spice_bsource_i(inout p, inout n; parameter expr I);
```
Requires **AST serializer**: Piperine expression → ngspice expression string (v(n), i(v1), temper, time, ddt(), idt(), laplace()...).

#### JFET (lower priority but complete)
```verilog
extern module spice_jfet(inout d, inout g, inout s; parameter string model; parameter real area = 1.0);
```

### 3D. `.model` Card Elaboration

When any SPICE primitive device has a `model` parameter, elaborator must:
1. Collect all `.model` cards from `paramset` bindings in the document
2. Emit them before device instances in the netlist
3. Deduplicate by model name

```
* emitted by elaborator for paramset d1n4148
.model d1n4148_model D (is=2.52e-9 n=1.752 bv=75 ibv=1e-8 cjo=4e-12 m=0.333 tt=5.76e-9)

D1 anode cathode d1n4148_model
```

### 3E. Differential Voltage and Extended `$V` / `$I`

```verilog
// current: $V("out") → v(out)
// needed:
$V("out", "in")   // v(out,in) = v(out) - v(in)
$V_mag("out")     // magnitude after AC: mag(v(out))
$V_phase("out")   // phase in degrees
$V_db("out")      // 20*log10(mag(v(out)))
$V_real("out")    // real part
$V_imag("out")    // imaginary part
```

AC vector access requires: backend returns complex pairs; interpreter has `complex` value type or returns `real[$]` of [mag, phase].

### 3F. `$meas` — All 16 Measurement Types

```verilog
// system function returning real
real bw = $meas("ac", "bandwidth", "TRIG v(out) VAL='-3db(v(out))' RISE=1 TARG v(out) VAL='-3db(v(out))' FALL=1");
// or structured:
real bw = $meas_trig_targ("ac", "bw", "v(out)", -3.0, 1, 0, "v(out)", -3.0, 1, 0);
```

Simplest impl: `$meas(analysis, name, spec_string)` — passes `spec_string` as-is to ngspice `.meas` injection, reads result vector `name`.

### 3G. `$get_vec` — Full Vector Retrieval

```verilog
real v_out[$] = $get_vec("v(out)");   // returns dynamic array
real t[$]     = $get_vec("time");
```

Requires:
- Backend: `get_vector` already returns `Vec<f64>` — expose as `real[$]` to interpreter
- Interpreter: `real[$]` as `Value::RealVec(Vec<f64>)`; assignment to dynamic array variable

### 3H. `$device_param` — Device Operating Point Read

```verilog
real id    = $device_param("m1", "id");     // @m1[id]
real vgs   = $device_param("m1", "vgs");
```

Backend issues `print @m1[id]` and parses scalar result. Or accesses vector `@m1[id]` directly via shared lib.

### 3I. Physical Constants

Predefined in testbench scope:

| Constant | Value |
|----------|-------|
| `M_PI` | 3.14159265358979... |
| `M_E` | 2.71828... |
| `BOLTZMANN` | 1.3806503e-23 |
| `ECHARGE` | 1.60217646e-19 |
| `KELVIN` | 273.15 |
| `PLANCK` | 6.62606896e-34 |
| `C_LIGHT` | 299792458.0 |
| `T_NOM` | 27.0 (°C) |

Interpreter injects these into global scope before `initial` runs.

### 3J. `$alter` / `$altermod` / `$alterparam`

```verilog
$alter("r1.r", 2e3);              // device instance parameter
$alter("@v1[pulse]", {0,5,10e-9,10e-9,10e-9,50e-9,100e-9});  // vector alter
$altermod("nmos18", "vth0", 0.42);  // model parameter
$alterparam("Rval", 2e3);           // .param variable
```

`$alter` with vector value → requires array literal expression support (see 3B PWL note).

### 3K. `$finish` / `$fatal` / `$error` / `$warning` / `$info`

```verilog
$finish;           // exit cleanly
$fatal(0, "msg");  // exit with code
$error("msg");     // print error, continue
$warning("msg");
$info("msg");
```

Interpreter: `$finish` throws a `ControlFlow::Finish` that main.rs catches. `$fatal` same + exit code.

---

## Phase 4 — Analyses Completions + Assertions

### 4A. Remaining Analyses

| Task | ngspice | Piperine syntax |
|------|---------|-----------------|
| `$pz` | `pz ni ng nj nk vol\|cur pol\|zer\|pz` | `$pz("in", "0", "out", "0", "vol", "pz")` |
| `$disto` | `disto dec np f1 f2 [f2/f1]` | `$disto("dec", 20, 1e3, 1e6, 0.9)` |
| `$sp` | S-parameter analysis | `$sp("dec", 20, 1e6, 6e9)` + port sources |

S-parameter requires `extern module spice_port(inout p, inout n; parameter real z0 = 50.0; parameter int portnum = 1)`.

### 4B. Extended Waveforms

```verilog
extern module spice_vsource_exp(inout p, inout n;
    parameter real v1=0; parameter real v2=0;
    parameter real td1=0; parameter real tau1=0;
    parameter real td2=0; parameter real tau2=0);

extern module spice_vsource_pwl(inout p, inout n;
    parameter real times[]; parameter real values[];
    parameter real td=0; parameter real r=0);

extern module spice_vsource_sffm(inout p, inout n;
    parameter real vo=0; parameter real va=0;
    parameter real fc=0; parameter real mdi=0; parameter real fs=0);

extern module spice_vsource_am(inout p, inout n;
    parameter real va=0; parameter real vo=0;
    parameter real mf=0; parameter real fc=0; parameter real td=0);

extern module spice_vsource_trnoise(inout p, inout n;
    parameter real na=0; parameter real ts=0; parameter real nalpha=0;
    parameter real namp=0; parameter real rtsam=0;
    parameter real rtscapt=0; parameter real rtsemt=0);

extern module spice_vsource_trrandom(inout p, inout n;
    parameter int type=1; parameter real ts=0; parameter real td=0;
    parameter real param1=0; parameter real param2=0);
```

(All have `spice_isource_*` analogs.)

### 4C. Switch Devices

```verilog
extern module spice_sw(inout p, inout n, inout cp, inout cn; parameter string model);
extern module spice_csw(inout p, inout n; parameter string vsrc; parameter string model);
```

`.model` for switches: `spice_sw_model` → `.model SW (ron=1 roff=1e6 vt=0 vh=0)`

### 4D. Transmission Lines

```verilog
extern module spice_tline(inout in_p, inout in_n, inout out_p, inout out_n;
    parameter real z0=50; parameter real td=0; parameter real f=0; parameter real nl=0);

extern module spice_ltra(inout p1, inout n1, inout p2, inout n2;
    parameter string model);  // .model LTRA lossy TL
```

### 4E. File Output

```verilog
$write("output.raw");                    // save all current vectors
$write("output.raw", "v(out)", "time");  // save specific vectors
$wrdata("output.txt", "v(out)");         // ASCII two-column
$wrs2p("results.s2p");                   // after $sp analysis
```

### 4F. Solver Options

```verilog
$set_option("reltol", 1e-6);
$set_option("method", "gear");
$set_option("temp", 85.0);
$set_temp(85.0);    // shorthand for $set_option("temp", ...)
```

### 4G. `.lib` / `.include` Interop

```verilog
// At module level (structural context):
`include_lib "path/to/models.lib" "tt"   // process corner

// Or as task in initial block:
$include_lib("path/to/models.lib", "tt");
```

Elaborator inserts `.lib "file" section` card into netlist before `.end`.

### 4H. SOA Monitoring (Callback Pathway)

Requires wiring libngspice per-timepoint callback to interpreter:

```verilog
always @(step) begin
    assert ($V("d","s") <= 35.0) else $error("Vds overvoltage at t=%g", $time);
end

always @(above($V("d","s") - 25.0)) $warning("approaching Vds breakdown");
```

Implementation:
1. Worker thread runs `always @(step)` body on each `pvec_data` callback from libngspice
2. `@(above(expr))` — evaluate expr each step, fire once on positive crossing
3. `$error` inside SOA block: log + halt; `$warning`: log + continue

### 4I. Multi-Run Plot Management (Monte Carlo)

```verilog
// After multiple $tran/$ac runs, each creates tran1, tran2, ...
string[$] plots = $get_plots();     // ["tran1", "tran2", ...]
real[$]   wc_out = $get_vec_from_plot("tran3", "v(out)");
$destroy_plot("tran1");             // free memory
$cur_plot();                        // returns current plot name string
```

### 4J. FFT / Fourier

```verilog
$fft("v(out)");                          // in-place FFT, creates freq-domain vectors
$fourier(1e6, "v(out)");                 // .fourier equiv: fund freq + harmonics
$linearize("v(out)");                    // resample to uniform time grid before FFT
$psd("v(out)", 1024, "hanning");         // power spectral density
```

### 4K. `.save` / `.probe` Control

```verilog
$save("v(out)", "i(v1)", "@m1[id]");   // limit saved vectors (emit .save before load)
// or at elaboration time via a pragma/attribute
```

### 4L. `.global` Nets

```verilog
// Language keyword in module body:
module my_cell(inout d, inout g, inout s, inout b);
    global wire vdd, vss;   // elaborator emits .global vdd vss
    ...
endmodule
```

Or via elaborator convention: nets named `vdd`, `vss`, `gnd` are automatically `.global`.

---

## Phase 5 — Refinement, OO Results, Statistical

### 5A. Monte Carlo / Statistical

```verilog
// Strategy A (recommended): Piperine-side RNG + $alter loop
$setseed(42);
for (int mc = 0; mc < 200; mc++) begin
    real r_val = $normal(1e3, 50.0);     // mean=1k, stddev=50R
    real c_val = $uniform(9e-12, 11e-12);
    $alter("r1.r", r_val);
    $alter("c1.c", c_val);
    $tran(1n, 100n);
    real v_peak = $vecmax($get_vec("v(out)"));
    results.push_back(v_peak);
end
```

New system functions:
- `$normal(mean, stddev)` → real (Box-Muller or Rust `rand`)
- `$uniform(lo, hi)` → real
- `$setseed(n)` — seed Rust RNG + `set rndseed=n` in ngspice
- `$urandom()` → integer
- `$urandom_range(lo, hi)` → integer

### 5B. `$show` / `$showmod`

```verilog
real vth = $show("m1", "vth0");       // reads device operating point param
$showmod("nmos18");                    // prints model card (diagnostic)
```

### 5C. Object-Oriented Results (Deferred per DESIGN.md §5.4)

```verilog
TranResult t = $tran(1n, 5m);
real bw = t.signal("v(out)").bandwidth_3db();
Signal s = t.signal("v(out)");
real pk = s.max(); real avg = s.mean();

AcResult ac = $ac("dec", 20, 1, 1e9);
real bw3db = ac.signal("v(out)").bandwidth(-3.0);
real pm = ac.phase_margin("v(out)");
```

Requires: `TranResult`, `AcResult`, `Signal` types in interpreter; method dispatch.

### 5D. Compatibility Modes

```verilog
$set_compat("hspice");     // set ngbehavior=hs
$set_compat("ltspice");    // set ngbehavior=lt
$set_compat("pspice");     // set ngbehavior=ps
```

### 5E. MESFET / VDMOS / Advanced Devices

```verilog
extern module spice_mesfet(inout d, inout g, inout s; parameter string model; parameter real area=1.0);
extern module spice_vdmos(inout d, inout g, inout s; parameter string model; parameter string type="nmos");
```

### 5F. `$sformat` / String Functions

```verilog
string fname = $sformat("mc_run_%03d.raw", mc);
$write(fname);
int pos = $strstr("v(out)", "(");
string sub = $strslice("v(out)", 2, 5);   // "out"
```

### 5G. `$load` — Rawfile Import

```verilog
$load("golden.raw");   // load previous results into current session
real[$] golden = $get_vec("v(out)");
```

---

## Parser Changes Required

| Feature | Change |
|---------|--------|
| `paramset` declaration | New top-level construct |
| `global wire name` | New module-level statement |
| `parameter expr V` | New parameter kind (AST-passthrough) |
| `parameter real arr[]` | Array parameter type |
| `always @(step)` | New event sensitivity form |
| `always @(above(expr))` | Analog event in sensitivity list |
| `{v1, v2, v3}` as `real[$]` literal | Array literal expression |
| Physical constants (`M_PI`, `BOLTZMANN`, ...) | Predefined identifiers |

---

## Implementation Order (Recommended)

```
Sprint 1:  $dc, $ac, $tran(tstep,tstop), $alter — core parametric workflow
Sprint 2:  AC stimulus + PULSE/SIN on vsource/isource — basic transient + AC testbenches
Sprint 3:  D, Q, M + .model emission — semiconductor circuits
Sprint 4:  L, K, VCVS, VCCS — passive + controlled sources
Sprint 5:  $V("n1","n2"), $V_mag/phase/db, $get_vec, complex value type — AC measurements
Sprint 6:  $meas (all 16 types), $device_param, physical constants
Sprint 7:  $noise, $tf, $alterparam, $altermod
Sprint 8:  B-source + AST serializer — behavioral sources
Sprint 9:  Monte Carlo RNG ($normal, $uniform, $setseed, plot management)
Sprint 10: SOA callbacks (always @(step)), $pz, $disto, file I/O
Sprint 11: .lib interop, $set_option, .global, PWL waveforms
Sprint 12: OO results (TranResult, Signal, AcResult)
```

---

## Feature Count Summary

| Phase | New system tasks/funcs | New extern modules | Parser changes | Priority |
|-------|----------------------|-------------------|----------------|----------|
| Phase 3 | 18 | 8 (D, Q, M, L, K, E, G, B) | `paramset`, array literal | P0/P1 |
| Phase 4 | 22 | 6 (switch, tline, port, pwl, sffm, trnoise) | `always @(step)`, `@(above())`, `global wire` | P1/P2 |
| Phase 5 | 12 | 3 (mesfet, vdmos, xspice) | `parameter expr`, OO types | P2/P3 |
| **Total** | **~52** | **~17** | **~8 new constructs** | |
