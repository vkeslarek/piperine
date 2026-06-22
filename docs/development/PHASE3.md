# Piperine Phase 3 — Complete ngspice Interface Proposal

This document proposes the full user-facing Piperine API that covers **every ngspice
feature**. It is an intent document — scope only, no implementation detail. Each section
will be expanded into its own implementation plan before work begins.

---

## 1. Analyses

### 1.1 Core Analyses (Phase 3 P0)

All analyses become system functions that return a typed result object.

```verilog
// DC operating point
OpResult  op_res = $op();

// DC sweep — source, start, stop, step [, source2, start2, stop2, step2]
DcResult  dc_res = $dc("v1", 0.0, 5.0, 0.01);
DcResult  dc2d   = $dc("v1", 0.0, 3.3, 0.01, "v2", 0.0, 1.8, 0.1);

// Transient — tstep, tstop [, tstart [, tmax]] [, uic=true]
TranResult t = $tran(1e-9, 1e-3);
TranResult t = $tran(1e-9, 1e-3, 100e-9);          // tstart
TranResult t = $tran(1e-9, 1e-3, 0.0, 100e-12);    // tstart + tmax
TranResult t = $tran(1e-9, 1e-3, 0.0, 0.0, 1);     // uic=true

// AC — spacing ("dec"|"oct"|"lin"), points, fstart, fstop
AcResult  ac = $ac("dec", 20, 1.0, 1e9);
AcResult  ac = $ac("lin", 1000, 1e3, 1e6);

// Noise — output, input_src, spacing, points, fstart, fstop [, ptspersum]
NoiseResult ns = $noise("v(out)", "v1", "dec", 20, 1.0, 1e9);
NoiseResult ns = $noise("v(out,ref)", "v1", "dec", 20, 1.0, 1e9, 5);

// Transfer function — outvar, input_source
TfResult  tf = $tf("v(out)", "v1");
TfResult  tf = $tf("i(v_load)", "v1");

// Sensitivity
SensResult sdc = $sens("v(out)");                           // DC sens
SensResult sac = $sens_ac("v(out)", "dec", 20, 1.0, 1e9);  // AC sens

// Pole-zero — in+, in-, out+, out-, vol|cur, pol|zer|pz
PzResult pz = $pz("in", "0", "out", "0", "vol", "pz");
PzResult pz = $pz("in", "0", "out", "0", "cur", "pol");

// Distortion — spacing, points, fstart, fstop [, f2overf1]
DistoResult d = $disto("dec", 20, 1e3, 1e6);
DistoResult d = $disto("dec", 20, 1e3, 1e6, 0.9);

// Periodic Steady State — fguess, stabtime, points, harmonics
PssResult pss = $pss(1e6, 10e-6, 100, 10);

// S-parameters (requires spice_port sources in netlist)
SpResult  sp = $sp("dec", 20, 1e6, 6e9);
```

### 1.2 Result Types

Result types expose signals and measurements. Detail to be defined per-type.

```verilog
// All results provide:
string plot_name = result.plot_name();      // "tran1", "ac1", etc.
bool   ok        = result.ok();             // false if run_errors occurred
RunError[$] errs = result.run_errors();     // assertions that triggered

// Signal access (indexed by vector name):
Signal s = result.signal("v(out)");
Signal t = result.signal("time");           // scale vector

// Inline measurement on Signal:
real pk   = s.max();
real avg  = s.mean();
real rms  = s.rms();
real pp   = s.peak_to_peak();
real intg = s.integral();
real f3db = s.bandwidth_3db();             // AcResult only
real pm   = s.phase_margin();              // AcResult only

// Raw vector:
real[$] data = s.values();
real[$] freq = result.scale();
```

### 1.3 Measurement — $meas

```verilog
// Passthrough: spec_string is ngspice .meas syntax verbatim
real bw = $meas("ac", "bw3db", "WHEN vdb(out)=-3 FALL=1");

// Structured measurements (all return real):
real v    = $meas_find_at("tran", "v(out)", 10e-9);       // FIND AT
real t50  = $meas_when("tran", "v(out)", 0.5);             // WHEN val
real tpd  = $meas_trig_targ("tran",
                "v(in)",  0.5, "rise", 1,
                "v(out)", 0.5, "fall", 1);                 // TRIG-TARG
real vrms = $meas_rms("tran", "v(out)", 10e-9, 100e-9);   // RMS FROM-TO
real avg  = $meas_avg("tran", "v(out)", 0.0, 0.0);        // AVG
real mn   = $meas_min("tran", "v(out)");
real mx   = $meas_max("tran", "v(out)");
real tm   = $meas_max_at("tran", "v(out)");                // time at max
real eng  = $meas_integral("tran", "v(out)", 0.0, 1e-6);
```

---

## 2. Component Library (extern modules)

### 2.1 Passives

```verilog
// Resistor
extern module spice_r(inout p, inout n;
    parameter real r = 1e3;
    parameter real tc1 = 0.0;   // first-order temp coeff (1/°C)
    parameter real tc2 = 0.0;   // second-order temp coeff (1/°C²)
    parameter real m   = 1.0;   // multiplier
    parameter real noisy = 1;   // 0 = noiseless
    parameter string model = "");

// Capacitor
extern module spice_c(inout p, inout n;
    parameter real c  = 1e-12;
    parameter real ic = 0.0;    // initial voltage condition
    parameter real m  = 1.0;
    parameter string model = "");

// Inductor
extern module spice_l(inout p, inout n;
    parameter real l  = 1e-6;
    parameter real ic = 0.0;    // initial current
    parameter real m  = 1.0;
    parameter string model = "");

// Mutual inductor (coupled inductors)
extern module spice_k(
    parameter string l1;        // instance name of first inductor
    parameter string l2;        // instance name of second inductor
    parameter real   k  = 0.5); // coupling coefficient (0 < k ≤ 1)
```

### 2.2 Sources

```verilog
// DC voltage source with AC stimulus + transient waveform
extern module spice_v(inout p, inout n;
    parameter real   dc      = 0.0;
    parameter real   acmag   = 0.0;
    parameter real   acphase = 0.0;
    // waveform — pick one (empty string = none)
    parameter string waveform = "";   // "PULSE","SIN","EXP","PWL","SFFM","AM","TRNOISE","TRRANDOM"
    // PULSE params
    parameter real v1=0; parameter real v2=0;
    parameter real td=0; parameter real tr=0; parameter real tf=0;
    parameter real pw=0; parameter real per=0; parameter int np=0;
    // SIN params
    parameter real vo=0; parameter real va=0; parameter real freq=0;
    parameter real theta=0; parameter real phase=0;
    // EXP params
    parameter real td1=0; parameter real tau1=0;
    parameter real td2=0; parameter real tau2=0;
    // PWL — passed as string "t0 v0 t1 v1 ..." (see note below)
    parameter string pwl = "";
    parameter real   pwl_td=0; parameter real pwl_r=-1;
    // SFFM
    parameter real fc=0; parameter real mdi=0; parameter real fs=0;
    // AM
    parameter real sa=0; parameter real oc=0; parameter real mf=0;
    // TRNOISE
    parameter real na=0; parameter real nt=0; parameter real nalpha=0;
    parameter real namp=0; parameter real rtsam=0;
    parameter real rtscapt=0; parameter real rtsemt=0;
    // TRRANDOM
    parameter int  rnd_type=1; parameter real rnd_ts=0;
    parameter real rnd_param1=1.0; parameter real rnd_param2=0.0;
);

// Convenient per-waveform variants (cleaner parameter names):
extern module spice_vpulse(inout p, inout n;
    parameter real dc=0; parameter real acmag=0; parameter real acphase=0;
    parameter real v1=0; parameter real v2=0;
    parameter real td=0; parameter real tr=0; parameter real tf=0;
    parameter real pw=0; parameter real per=0; parameter int np=0);

extern module spice_vsin(inout p, inout n;
    parameter real dc=0; parameter real acmag=0; parameter real acphase=0;
    parameter real vo=0; parameter real va=0; parameter real freq=0;
    parameter real td=0; parameter real theta=0; parameter real phase=0);

extern module spice_vexp(inout p, inout n;
    parameter real dc=0; parameter real acmag=0; parameter real acphase=0;
    parameter real v1=0; parameter real v2=0;
    parameter real td1=0; parameter real tau1=0;
    parameter real td2=0; parameter real tau2=0);

extern module spice_vpwl(inout p, inout n;
    parameter string pwl = "";      // "t0 v0 t1 v1 ..."
    parameter real   td  = 0.0;
    parameter real   r   = -1.0);  // -1 = no repeat

extern module spice_vsffm(inout p, inout n;
    parameter real vo=0; parameter real va=0; parameter real fc=0;
    parameter real mdi=0; parameter real fs=0);

extern module spice_vam(inout p, inout n;
    parameter real sa=0; parameter real oc=0; parameter real mf=0;
    parameter real fc=0; parameter real td=0);

extern module spice_vtrnoise(inout p, inout n;
    parameter real dc=0;
    parameter real na=0; parameter real nt=0; parameter real nalpha=0;
    parameter real namp=0; parameter real rtsam=0;
    parameter real rtscapt=0; parameter real rtsemt=0);

extern module spice_vtrrandom(inout p, inout n;
    parameter real   dc=0;
    parameter int    type=1;    // 1=uniform, 2=gaussian, 3=exp, 4=poisson
    parameter real   ts=0;
    parameter real   param1=1.0; parameter real param2=0.0);

// Current sources — mirror all V variants but with I=... emission
extern module spice_i(inout p, inout n; /* same params as spice_v */ );
extern module spice_ipulse(inout p, inout n; /* PULSE only */ );
extern module spice_isin(inout p, inout n; /* SIN only */ );
// ... etc.

// RF port source (S-parameter analysis)
extern module spice_port(inout p, inout n;
    parameter int  portnum = 1;
    parameter real z0      = 50.0);
```

### 2.3 Semiconductor Devices

```verilog
// Diode
extern module spice_d(inout a, inout c;
    parameter string model;
    parameter real area  = 1.0;
    parameter real m     = 1.0;
    parameter real pj    = 0.0;   // perimeter junction
    parameter real ic    = 0.0;   // initial condition
    parameter real temp  = 27.0;
    parameter real dtemp = 0.0);

// BJT (NPN/PNP — type in model card)
extern module spice_q(inout c, inout b, inout e;
    parameter string model;
    parameter real   area  = 1.0;
    parameter real   areeb = 1.0;  // emitter area
    parameter real   areec = 1.0;  // collector area
    parameter real   m     = 1.0;
    parameter real   ic    = 0.0;  // initial Vbe
    parameter real   temp  = 27.0;
    parameter real   dtemp = 0.0);

// MOSFET (level 1/2/3/BSIM variants — level in model card)
extern module spice_m(inout d, inout g, inout s, inout b;
    parameter string model;
    parameter real l   = 1e-6;
    parameter real w   = 1e-6;
    parameter real ad  = 0.0; parameter real as_ = 0.0;
    parameter real pd  = 0.0; parameter real ps  = 0.0;
    parameter real nrd = 1.0; parameter real nrs  = 1.0;
    parameter real m   = 1.0;
    parameter real ic1 = 0.0;  // initial Vds
    parameter real ic2 = 0.0;  // initial Vgs
    parameter real ic3 = 0.0;  // initial Vbs
    parameter real temp  = 27.0;
    parameter real dtemp = 0.0);

// JFET
extern module spice_j(inout d, inout g, inout s;
    parameter string model;
    parameter real   area  = 1.0;
    parameter real   ic1   = 0.0;  // initial Vds
    parameter real   ic2   = 0.0;  // initial Vgs
    parameter real   temp  = 27.0;
    parameter real   dtemp = 0.0);

// MESFET (GaAs, level 1/2/3)
extern module spice_z(inout d, inout g, inout s;
    parameter string model;
    parameter real   area  = 1.0;
    parameter real   ic1   = 0.0;
    parameter real   ic2   = 0.0);

// VDMOS (vertical DMOS)
extern module spice_vdmos(inout d, inout g, inout s;
    parameter string model;
    parameter string type = "nmos");  // "nmos" or "pmos"
```

### 2.4 Controlled Sources

```verilog
// VCVS (linear voltage-controlled voltage source)
extern module spice_vcvs(inout p, inout n, inout cp, inout cn;
    parameter real gain = 1.0);

// VCCS (linear voltage-controlled current source)
extern module spice_vccs(inout p, inout n, inout cp, inout cn;
    parameter real gm = 1e-3);  // transconductance (S)

// CCCS (current-controlled current source — requires sense vsource)
extern module spice_cccs(inout p, inout n;
    parameter string vsrc;      // name of sense voltage source
    parameter real   gain = 1.0);

// CCVS (current-controlled voltage source)
extern module spice_ccvs(inout p, inout n;
    parameter string vsrc;      // name of sense voltage source
    parameter real   transres = 1.0);  // transresistance (Ω)
```

### 2.5 Behavioral Source (B-source)

The B-source (`Bxxx N+ N- V=<expr>` or `I=<expr>`) needs an **AST-passthrough**
parameter so expressions written in Piperine are serialized to ngspice expression
syntax verbatim. This requires a new parameter kind `expr` and an expression
serializer.

```verilog
// Voltage B-source
extern module spice_bv(inout p, inout n;
    parameter expr V;    // expression: v(a)+v(b), time*1e3, etc.
    parameter real tc1 = 0.0;
    parameter real tc2 = 0.0);

// Current B-source
extern module spice_bi(inout p, inout n;
    parameter expr I;
    parameter real tc1 = 0.0;
    parameter real tc2 = 0.0);

// E-source VALUE= form (B-source rewrite in ngspice)
extern module spice_evalue(inout p, inout n;
    parameter expr V);

// G-source VALUE= form
extern module spice_gvalue(inout p, inout n;
    parameter expr I);
```

### 2.6 Switches

```verilog
// Voltage-controlled switch
extern module spice_sw(inout p, inout n, inout cp, inout cn;
    parameter string model;
    parameter string initial = "");  // "on" or "off"

// Current-controlled switch
extern module spice_csw(inout p, inout n;
    parameter string vsrc;           // controlling voltage source name
    parameter string model;
    parameter string initial = "");
```

### 2.7 Transmission Lines

```verilog
// Ideal lossless TL (T-element)
extern module spice_tline(inout in_p, inout in_n, inout out_p, inout out_n;
    parameter real z0 = 50.0;
    parameter real td = 0.0;      // propagation delay (s)
    parameter real f  = 0.0;      // frequency (Hz) — alternative to td
    parameter real nl = 0.25);    // normalized length (fractions of wavelength)

// Lossy TL (O-element, LTRA model)
extern module spice_ltra(inout p1, inout n1, inout p2, inout n2;
    parameter string model);

// KSPICE coupled TL (CPL)
extern module spice_cpl(inout p1, inout n1, inout p2, inout n2,
                        inout p3, inout n3, inout p4, inout n4;
    parameter string model);
```

### 2.8 Subcircuit Passthrough

For netlists with existing `.subckt` definitions, Piperine can call them directly
using an `extern module` declaration that maps to an `X`-line instantiation:

```verilog
// Any .subckt already loaded by .include_lib / $include_lib
extern module my_opamp(inout in_p, inout in_n, inout out, inout vdd, inout vss;
    parameter real gain = 1e5);

// Elaborator emits: X1 in_p in_n out vdd vss my_opamp gain=1e5
```

---

## 3. Signal Access

### 3.1 Scalar Voltage / Current

```verilog
// Node voltage — last point (DC/OP) or final value (TRAN)
real v = $V("out");             // v(out) relative to ground
real v = $V("out", "ref");      // differential v(out,ref)

// Branch current
real i = $I("v1");              // i(v1) or v1#branch

// Device operating-point parameter
real id  = $device_param("m1", "id");
real vth = $device_param("m1", "vth");
real gm  = $device_param("m1", "gm");
// Shorthand for common params:
real id  = $Id("m1");   real gm  = $Gm("m1");
real vth = $Vth("m1");  real vds = $Vds("m1");
```

### 3.2 AC Quantities (complex)

After `$ac()`, vectors are complex. Access components explicitly:

```verilog
real mag   = $V_mag("out");        // magnitude   = |v(out)|
real phase = $V_phase("out");      // phase (deg) = ph(v(out))
real db    = $V_db("out");         // dB           = 20*log10(|v(out)|)
real re    = $V_real("out");       // real part    = re(v(out))
real im    = $V_imag("out");       // imag part    = im(v(out))
real gdly  = $V_group_delay("out");// group delay  = -d(ph)/dω
```

### 3.3 Full Vector Retrieval

```verilog
real[$] vout = $get_vec("v(out)");      // all timepoints
real[$] time = $get_vec("time");        // scale vector
real[$] freq = $get_vec("frequency");   // AC scale

// From a specific plot (named or by index):
real[$] old  = $get_vec_from_plot("tran2", "v(out)");

// All vector names in current plot:
string[$] names = $list_vecs();
string[$] names = $list_vecs("ac1");    // from named plot
```

### 3.4 Noise Result Access

```verilog
NoiseResult ns = $noise("v(out)", "v1", "dec", 20, 1.0, 1e9);
real[$] onoise = ns.output_spectrum();  // V²/Hz
real[$] inoise = ns.input_spectrum();
real    total  = ns.output_total();     // integrated total
real    intot  = ns.input_total();
```

---

## 4. Circuit Control

### 4.1 Parameter and Component Alteration

```verilog
// Alter device instance parameter (re-simulate without re-loading)
$alter("r1", "r", 2.2e3);                 // R1 = 2.2 kΩ
$alter("m1", "w", 2e-6);                  // MOSFET width
$alter("v1", "dc", 1.5);                  // DC source value
$alter("@v1[pulse]", {0, 5, 10e-9, 10e-9, 10e-9, 50e-9, 100e-9}); // vector alter

// Alter model parameter
$altermod("nmos18", "vth0", 0.42);
$altermod("nmos18", "tox", 3.2e-9);

// Alter .param value
$alterparam("Vdd", 1.2);
$alterparam("Rval", 2.2e3);
```

### 4.2 Solver Options

```verilog
$set_option("reltol",   1e-6);
$set_option("abstol",   1e-15);
$set_option("method",   "gear");
$set_option("maxord",   6);
$set_option("itl1",     500);
$set_option("gmin",     1e-15);

// Shorthand helpers
$set_temp(85.0);          // .options temp=85 + set temp=85
$set_tnom(27.0);          // nominal temperature for model params
$set_compat("hspice");    // set ngbehavior=hs
$set_compat("pspice");    // set ngbehavior=ps
```

### 4.3 Initial Conditions

```verilog
$nodeset("out", 0.9);       // .nodeset V(out)=0.9 (hint, not hard)
$ic("out", 0.0);            // .ic V(out)=0 (hard initial condition)
$ic("cap_top", 1.8);
```

### 4.4 Save / Probe Selection

```verilog
// Limit saved vectors (must be called before circuit load / re-load)
$save("v(out)", "v(in)", "i(v1)", "@m1[id]");
$save_all();      // save everything (default)
$save_allv();     // voltages only
$save_alli();     // currents only
```

---

## 5. Post-Processing

### 5.1 FFT / Spectral

```verilog
// After $tran — requires linearize first if adaptive timestep
$linearize("v(out)");           // resample to uniform time grid
$fft("v(out)");                 // FFT in-place → new freq-domain vectors
$ifft("v(out)");                // inverse FFT
$psd("v(out)");                 // power spectral density (V²/Hz)
$spec(1e6, 1e9, 1e6, "v(out)"); // spectrum on custom grid (fstart, fstop, fstep)
$fourier(1e6, "v(out)");        // fundamental + 9 harmonics (like .fourier)
```

### 5.2 Vector Arithmetic (inline in initial block)

ngspice nutmeg vector math exposed as Piperine built-in functions operating on
`real[$]` values:

```verilog
real[$] h    = $vec_div($get_vec("v(out)"), $get_vec("v(in)"));
real[$] hdb  = $vec_db(h);                     // 20*log10(|H|)
real[$] ph   = $vec_phase(h);                  // phase (deg)
real[$] gdly = $vec_group_delay(h);            // group delay
real    avg  = $vec_mean($get_vec("v(out)"));
real    rms  = $vec_rms($get_vec("v(out)"));
real    mx   = $vec_max($get_vec("v(out)"));
real    mn   = $vec_min($get_vec("v(out)"));
real[$] d    = $vec_deriv($get_vec("v(out)"), $get_vec("time"));
```

### 5.3 Plot Management (Monte Carlo)

```verilog
string[$] plots  = $get_plots();               // ["tran1", "tran2", ...]
string    cur    = $cur_plot();                 // current active plot name
real[$]   wc_out = $get_vec_from_plot("tran3", "v(out)");
$set_plot("tran2");                            // make tran2 current
$destroy_plot("tran1");                        // free memory
$destroy_plot();                               // destroy current
```

---

## 6. File I/O

```verilog
// Save simulation results
$save_raw("results.raw");                       // binary ngspice raw format
$save_raw("results.raw", "v(out)", "time");     // specific vectors only
$save_ascii("output.txt", "v(out)");            // two-column plain text
$save_s2p("results.s2p");                       // Touchstone (after $sp)
$save_nodev("results.raw");                     // voltages only (no branch currents)

// Load previous results
$load_raw("golden.raw");                        // load into new plot
real[$] golden = $get_vec("v(out)");

// Rawfile format control
$set_option("filetype", "ascii");   // or "binary" (default)
```

---

## 7. Statistical / Monte Carlo

### 7.1 RNG Functions

```verilog
$setseed(42);                           // seed both Piperine RNG and ngspice rndseed
real r = $normal(1e3, 50.0);           // Gaussian: mean, stddev
real r = $uniform(900.0, 1100.0);      // Uniform: lo, hi
real r = $exponential(1e-6);           // Exponential: mean
int  n = $urandom();                    // 32-bit unsigned random
int  n = $urandom_range(0, 255);        // uniform integer range
real r = $dist_normal(0.0, 1.0);       // standard normal
real r = $agauss(1e3, 50.0, 3.0);      // nom + avar/sigma * N(0,1)  (ngspice MC convention)
real r = $aunif(1e3, 0.05);            // nom + nom*rvar * U(-1,1)
```

### 7.2 Monte Carlo Pattern

```verilog
initial begin
    $setseed(42);
    real[$] results;
    for (int mc = 0; mc < 200; mc++) begin
        real r_val = $normal(1e3, 50.0);
        real c_val = $uniform(9e-12, 11e-12);
        $alter("r1", "r", r_val);
        $alter("c1", "c", c_val);
        TranResult t = $tran(1e-9, 100e-9);
        results.push_back(t.signal("v(out)").max());
        $destroy_plot();            // free memory between runs
    end
    $display("mean=%g sigma=%g", $vec_mean(results), $vec_std(results));
end
```

### 7.3 Temperature Sweep

```verilog
foreach (real T in '{-40.0, 0.0, 27.0, 85.0, 125.0}) begin
    $set_temp(T);
    OpResult op = $op();
    $display("T=%g Id=%g", T, $device_param("m1", "id"));
end
```

---

## 8. SOA Monitoring and Analog Events

### 8.1 Timestep Callback

```verilog
// Fires on every ngspice time step during $tran / run_analysis
always @(step) begin
    if ($V("vds") > 35.0)
        $run_error("Vds overvoltage: %g V at t=%g", $V("vds"), $time);
    if ($V("vgs") > 6.0)
        $fatal(0, "Vgs breakdown at t=%g", $time);
end
```

### 8.2 Threshold Crossing Events

```verilog
// Fires once per positive crossing of expr (expr goes from ≤0 to >0)
always @(above($V("vds") - 28.0)) begin
    $warning("Vds approaching limit: %g V at t=%g s", $V("vds"), $time);
end

// Fires once per negative crossing (above threshold → below)
always @(below($V("vds") - 28.0)) begin
    $display("Vds returned below limit at t=%g", $time);
end

// Fires on every zero-crossing (both directions) — maps to ngspice crossing
always @(cross($V("out") - 0.9)) begin
    $display("Out crossed 0.9 V at t=%g", $time);
end
```

### 8.3 Assertion Handlers

```verilog
// Immediate assertions in step handler
always @(step) begin
    assert ($V("out") inside {[-0.1 : 1.9]}) else
        $run_error("out of rail: %g", $V("out"));
    assert ($I("v_load") < 100e-3) else
        $warning("overcurrent: %g A", $I("v_load"));
end
```

---

## 9. Netlist / Model Interop

### 9.1 Library and Include

```verilog
// Structural level (elaboration time):
`include_lib "path/models.lib" "tt"       // include a .lib section
`include    "path/extra.spi"              // verbatim .include

// Inside initial block (loaded into ngspice before analysis):
$include_lib("path/models.lib", "tt");
$include("path/extra.spi");
```

### 9.2 Paramset — Model Card Emission

`paramset` binds device model parameters to a named model card that the elaborator
emits as a `.model` line in the netlist:

```verilog
paramset d1n4148 spice_d;
    .model    = "d1n4148_model";
    .is       = 2.52e-9;
    .n        = 1.752;
    .bv       = 75;
    .ibv      = 1e-8;
    .cjo      = 4e-12;
    .m        = 0.333;
    .tt       = 5.76e-9;
endparamset

// Use in netlist:
d1n4148 D1(.a(anode), .c(cathode));
// Elaborator emits: .model d1n4148_model D(is=2.52e-9 ...)
//                   D1 anode cathode d1n4148_model
```

```verilog
paramset nmos18 spice_m;
    .model = "nmos18_model";
    .level = 14;      // BSIM4
    .tox   = 3.2e-9;
    .vth0  = 0.42;
    // ... full model card
endparamset
```

### 9.3 Global Nets

```verilog
// Module-level declaration — elaborator emits .global
global wire vdd, vss, gnd;
```

Or via elaborator convention: nets named `vdd`, `vss`, `gnd` are automatically global.

### 9.4 `.options` in Netlist

```verilog
// Structural-level option injection (before circuit load):
`options reltol=1e-6 method="gear"
```

---

## 10. Language Completions (SystemVerilog Gaps)

These are language features missing from Phase 2 that are high-value for analog testbenches.

### 10.1 Types

```verilog
// Increment / decrement
i++;  ++i;  i--;  --i;

// More integer types
longint  n;      // 64-bit signed
byte     b;      // 8-bit signed
shortint s;      // 16-bit signed

// User-defined types
typedef real temperature_t;
typedef struct { real v; real i; } vi_pair_t;
typedef enum { IDLE, RUNNING, DONE } state_t;

// inside operator (range check)
if (x inside {[0.0 : 1.8]}) ...
if (state inside {IDLE, DONE}) ...
```

### 10.2 String Methods

```verilog
string s = "v(out)";
int    n = s.len();                     // 6
string u = s.toupper();                 // "V(OUT)"
string sub = s.substr(2, 4);           // "out"
int    cmp = s.compare("v(in)");       // strcmp result
real   r   = s.atoreal();              // parse as real
s.itoa(42);                            // integer → string in-place
$sformatf("mc_run_%03d.raw", mc);      // returns string
```

### 10.3 Array Methods

```verilog
real[$] arr;
arr.sort();
arr.rsort();
arr.reverse();
arr.shuffle();
real     s   = arr.sum();
real     mn  = arr.min()[0];
real     mx  = arr.max()[0];
real[$]  unq = arr.unique();
int[$]   idx = arr.find_index() with (item > threshold);
```

### 10.4 System Functions

```verilog
$clog2(n)                 // ceiling(log2(n)), integer arithmetic
$bits(expr)               // bit width of expression
$size(arr)                // array element count
$signed(v)                // force signed interpretation
$unsigned(v)              // force unsigned
$info("fmt", args...);    // [INFO] severity (logs, no halt)
$strobe("fmt", args...);  // print at end of timestep (for display consistency)
```

### 10.5 Named Blocks and Early Exit

```verilog
begin : sweep_loop
    for (int i = 0; i < 100; i++) begin
        if (some_condition) disable sweep_loop;  // break out of named block
        // ...
    end
end
```

### 10.6 Packages

```verilog
package device_params;
    parameter real VDD = 1.8;
    parameter real IBIAS = 100e-6;
    typedef struct { real l; real w; } mos_size_t;
    function real vt(real temp);
        return 8.617e-5 * (temp + 273.15);
    endfunction
endpackage

import device_params::*;
```

### 10.7 `void` Functions

```verilog
function void log_result(string name, real val);
    $display("[RESULT] %s = %g", name, val);
endfunction

log_result("bandwidth", 1.2e6);   // callable as statement
```

---

## 11. Physical Constants (Predefined)

Injected into global scope before `initial` runs:

| Constant   | Value              | Description                    |
|------------|--------------------|-----------------------------|
| `M_PI`     | 3.14159265358979…  | π                              |
| `M_E`      | 2.71828182845905…  | Euler's number                 |
| `BOLTZMANN`| 1.3806503e-23      | Boltzmann constant (J/K)       |
| `ECHARGE`  | 1.60217646e-19     | Elementary charge (C)          |
| `KELVIN`   | 273.15             | 0°C in Kelvin                  |
| `PLANCK`   | 6.62606896e-34     | Planck constant (J·s)          |
| `C_LIGHT`  | 299792458.0        | Speed of light (m/s)           |
| `T_NOM`    | 27.0               | Nominal temperature (°C)       |
| `EPSILON0` | 8.854187817e-12    | Permittivity of free space     |
| `MU0`      | 1.2566370614e-6    | Permeability of free space     |

---

## 12. OO Result Interface (Future Direction)

Annotated here as the intended direction; not required for Phase 3 to ship.

```verilog
// Analyses return structured objects (instead of void)
TranResult tran  = $tran(1e-9, 100e-9);
AcResult   ac    = $ac("dec", 20, 1.0, 1e9);
DcResult   dc    = $dc("v1", 0.0, 5.0, 0.01);
OpResult   op    = $op();
NoiseResult ns   = $noise("v(out)", "v1", "dec", 20, 1.0, 1e9);

// Signal access
Signal s    = tran.signal("v(out)");
real   mx   = s.max();
real   mn   = s.min();
real   avg  = s.mean();
real   rms  = s.rms();
real[$] raw = s.values();

// AC-specific
AcSignal hs    = ac.signal("v(out)");
real     bw    = hs.bandwidth_3db();
real     pm    = hs.phase_margin();
real     gm    = hs.gain_margin();
real[$]  hdb   = hs.db();
real[$]  phase = hs.phase_deg();

// Noise-specific
real inoise_total = ns.input_total();
real onoise_total = ns.output_total();
```

---

## 13. Implementation Priority Matrix

| Feature Group | Phase | Priority | Blocker for |
|---------------|-------|----------|-------------|
| $dc, $ac, $tran(tstep,tstop) | 3A | P0 | every parametric test |
| spice_vpulse/vsin — waveforms | 3B | P0 | transient/AC tests |
| spice_d, spice_q, spice_m — semiconductors | 3C | P0 | any real circuit |
| paramset + .model emission | 3D | P0 | all model-based devices |
| $V("n","ref"), $V_mag/db/phase | 3E | P0 | AC measurements |
| $alter, $altermod, $alterparam | 3J | P0 | parametric sweeps |
| Physical constants (M_PI etc.) | 3I | P0 | common math |
| $meas structured | 3F | P1 | timing/bandwidth |
| $get_vec → real[$] | 3G | P1 | MC/statistical |
| $device_param / $Id $Gm $Vth | 3H | P1 | OP inspection |
| always @(step) SOA | 4H | P1 | reliability checks |
| spice_l, spice_k — inductors | 3C | P1 | RF/power |
| spice_vcvs/vccs | 3C | P1 | amplifier models |
| $noise, $tf, $sens | 3A | P1 | noise/RF design |
| ++ / -- | SV | P1 | ergonomics |
| $info, $sformatf full | SV | P1 | diagnostics |
| typedef / enum / struct | SV | P1 | code organization |
| $fft, $linearize, $fourier | 4J | P2 | spectral analysis |
| $normal, $uniform, $setseed | 5A | P2 | Monte Carlo |
| plot management ($get_plots etc.) | 4I | P2 | MC result collection |
| File I/O ($save_raw, $load_raw) | 4E | P2 | result archiving |
| $set_option, $set_temp | 4F | P2 | convergence control |
| $save / $save_all | 4K | P2 | memory efficiency |
| always @(above/below/cross) | 4H | P2 | analog events |
| spice_bv / spice_bi (B-source) | 3C | P2 | behavioral models |
| $pz, $disto, $sp | 4A | P2 | RF/stability |
| array sort/find/sum | SV | P2 | MC post-processing |
| string methods full | SV | P2 | filename gen |
| packages | SV | P2 | code reuse |
| $include_lib | 4G | P2 | foundry models |
| void functions | SV | P2 | helpers |
| OO Result types | 5C | P3 | ergonomic API |
| $pss | 4A | P3 | RF PSS |
| spice_tline, spice_ltra | 4D | P3 | signal integrity |
| switch devices | 4C | P3 | power circuits |
| $wrs2p | 4E | P3 | RF S-params |
| MESFET, VDMOS | 5E | P3 | specialized devices |

---

## 14. Parser Changes Required

| Construct | Need |
|-----------|------|
| `paramset name device; .key=val; ... endparamset` | New top-level declaration |
| `global wire name, ...;` | Module-level statement |
| `parameter expr V` | New parameter kind (AST passthrough to serializer) |
| `parameter real arr[]` | Array parameter type |
| `always @(step)` | New event sensitivity form |
| `always @(above(expr))` / `@(below(expr))` / `@(cross(expr))` | Analog events |
| `{v0, v1, v2}` as `real[$]` literal | Array literal expression |
| `typedef` / `enum` / `struct` | User-defined types |
| `package` / `import` | Package system |
| `++` / `--` | Increment/decrement operators |
| `inside { ... }` | Set-membership operator |
| `disable name` | Named block exit |
| `` `include_lib `` / `` `options `` | Structural directives |
| `begin : name` / `end : name` | Named blocks |

---

## 15. Key Design Decisions to Resolve

1. **$tran dynamic vs netlist-declared**: Today `.tran` card must be in the netlist.
   Short-term: `$tran(tstep, tstop)` dynamically re-emits `.tran` via `alter` +
   `reset` + reload. Long-term: `run_analysis("tran tstep tstop")` via IPC.

2. **PWL waveform as string vs array parameter**: `parameter string pwl = "t0 v0 t1 v1 ..."`
   is a short-term hack. Long-term: `parameter real times[]; parameter real values[];`
   (requires array parameter support in elaborator).

3. **Complex value type**: AC results require complex numbers. Options:
   a) `real[$]` of `[mag, phase]` pairs — simple, no new type
   b) `complex` built-in type — cleaner API, more implementation work
   Recommendation: (a) initially, migrate to (b) when OO results land.

4. **B-source expression serializer**: Requires an AST → ngspice expression string
   lowering pass. Covers: `v(node)`, `i(vsrc)`, `@device[param]`, `time`, `temper`,
   `hertz`, all math functions, `pwl(...)`, `u(x)`, `uramp(x)`.

5. **`always @(above/below/cross)` mapping**: These are synthetic — Piperine polls
   the expression value on every `@(step)` callback and fires when the crossing
   condition is detected. No native ngspice crossing callback is used.
