use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use piperine_interpreter::{SystemTask, SimulatorBackend, Value, InterpreterError, AnalysisHandleObj};

// NOTE: $display, $write, $warning, $run_error, $fatal, $error, $sformatf,
// $abs, $min, $max are registered automatically by SystemTaskRegistry::default()
// (piperine_interpreter::stdlib). Only ngspice-specific tasks live here.

static MEAS_COUNTER: AtomicU32 = AtomicU32::new(0);

fn next_meas_name() -> String {
    format!("_p3m{}", MEAS_COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn require_str<'a>(args: &'a [Value], idx: usize, label: &str) -> Result<&'a str, InterpreterError> {
    args.get(idx)
        .and_then(|v| v.as_str())
        .ok_or_else(|| InterpreterError::TypeError {
            expected: format!("string for {label}"),
            got: args.get(idx).map(|v| v.type_name()).unwrap_or("nothing").into(),
        })
}

fn require_f64(args: &[Value], idx: usize, label: &str) -> Result<f64, InterpreterError> {
    args.get(idx)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| InterpreterError::TypeError {
            expected: format!("real for {label}"),
            got: args.get(idx).map(|v| v.type_name()).unwrap_or("nothing").into(),
        })
}

fn require_i64(args: &[Value], idx: usize, label: &str) -> Result<i64, InterpreterError> {
    args.get(idx)
        .and_then(|v| v.as_integer())
        .ok_or_else(|| InterpreterError::TypeError {
            expected: format!("integer for {label}"),
            got: args.get(idx).map(|v| v.type_name()).unwrap_or("nothing").into(),
        })
}

/// An optional real: positional slot `idx`, else named `key`, else `default`.
fn opt_f64(pos: &[Value], named: &HashMap<String, Value>, idx: usize, key: &str, default: f64) -> f64 {
    pos.get(idx).or_else(|| named.get(key)).and_then(|v| v.as_f64()).unwrap_or(default)
}

/// An optional integer: positional slot `idx`, else named `key`, else `default`.
fn opt_i64(pos: &[Value], named: &HashMap<String, Value>, idx: usize, key: &str, default: i64) -> i64 {
    pos.get(idx).or_else(|| named.get(key)).and_then(|v| v.as_integer()).unwrap_or(default)
}

/// Run an analysis command and wrap its result in a typed handle (`OpResult`, …).
/// This is the tail shared by every analysis task.
fn analysis(simulator: &mut dyn SimulatorBackend, cmd: &str, kind: &'static str)
    -> Result<Option<Value>, InterpreterError>
{
    let result = simulator.run_analysis_simple(cmd)?;
    Ok(Some(AnalysisHandleObj::new(result, kind)))
}

// ── $op() ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct OperatingPointTask;

impl SystemTask for OperatingPointTask {
    fn name(&self) -> &str { "op" }
    fn call(&self, _arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        analysis(simulator, "op", "OpResult")
    }
}

// ── $tran([tstep, tstop [, tstart [, tmax [, uic]]]]) ────────────────────────

#[derive(Debug)]
pub struct TransientTask;

impl SystemTask for TransientTask {
    fn name(&self) -> &str { "tran" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        self.call_named(arguments, HashMap::new(), simulator)
    }
    fn call_named(&self, positional: Vec<Value>, named: HashMap<String, Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        if positional.is_empty() {
            // Legacy mode: run pre-declared .tran
            return analysis(simulator, "run", "TranResult");
        }
        let tstep = require_f64(&positional, 0, "tstep")?;
        let tstop = require_f64(&positional, 1, "tstop")?;
        let tstart = opt_f64(&positional, &named, 2, "tstart", 0.0);
        let tmax   = opt_f64(&positional, &named, 3, "tmax", 0.0);
        let uic = named.get("uic").and_then(|v| v.as_integer()).unwrap_or(0);

        let mut cmd = format!("tran {tstep} {tstop}");
        if tstart != 0.0 { cmd += &format!(" {tstart}"); }
        if tmax != 0.0   { cmd += &format!(" {tmax}"); }
        if uic != 0      { cmd += " uic"; }

        analysis(simulator, &cmd, "TranResult")
    }
}

// ── $ac(spacing, points, fstart, fstop) ──────────────────────────────────────

#[derive(Debug)]
pub struct AcTask;

impl SystemTask for AcTask {
    fn name(&self) -> &str { "ac" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        self.call_named(arguments, HashMap::new(), simulator)
    }
    fn call_named(&self, positional: Vec<Value>, named: HashMap<String, Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let spacing = positional.get(0).or_else(|| named.get("spacing"))
            .and_then(|v| v.as_str()).unwrap_or("dec").to_string();
        let points = opt_i64(&positional, &named, 1, "points", 20);
        let fstart = positional.get(2).or_else(|| named.get("fstart"))
            .and_then(|v| v.as_f64())
            .ok_or_else(|| InterpreterError::TypeError { expected: "real fstart".into(), got: "nothing".into() })?;
        let fstop = positional.get(3).or_else(|| named.get("fstop"))
            .and_then(|v| v.as_f64())
            .ok_or_else(|| InterpreterError::TypeError { expected: "real fstop".into(), got: "nothing".into() })?;
        let cmd = format!("ac {spacing} {points} {fstart} {fstop}");
        analysis(simulator, &cmd, "AcResult")
    }
}

// ── $dc(src, start, stop, step [, src2, start2, stop2, step2]) ───────────────

#[derive(Debug)]
pub struct DcTask;

impl SystemTask for DcTask {
    fn name(&self) -> &str { "dc" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let src   = require_str(&arguments, 0, "src")?.to_string();
        let start = require_f64(&arguments, 1, "start")?;
        let stop  = require_f64(&arguments, 2, "stop")?;
        let step  = require_f64(&arguments, 3, "step")?;
        let mut cmd = format!("dc {src} {start} {stop} {step}");
        if arguments.len() >= 8 {
            let src2   = require_str(&arguments, 4, "src2")?.to_string();
            let start2 = require_f64(&arguments, 5, "start2")?;
            let stop2  = require_f64(&arguments, 6, "stop2")?;
            let step2  = require_f64(&arguments, 7, "step2")?;
            cmd += &format!(" {src2} {start2} {stop2} {step2}");
        }
        analysis(simulator, &cmd, "DcResult")
    }
}

// ── $noise(output, input_src, spacing, points, fstart, fstop [, ptspersum]) ──

#[derive(Debug)]
pub struct NoiseTask;

impl SystemTask for NoiseTask {
    fn name(&self) -> &str { "noise" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        self.call_named(arguments, HashMap::new(), simulator)
    }
    fn call_named(&self, positional: Vec<Value>, named: HashMap<String, Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let output    = require_str(&positional, 0, "output")?.to_string();
        let input_src = require_str(&positional, 1, "input_src")?.to_string();
        let spacing   = require_str(&positional, 2, "spacing")?.to_string();
        let points    = require_i64(&positional, 3, "points")?;
        let fstart    = require_f64(&positional, 4, "fstart")?;
        let fstop     = require_f64(&positional, 5, "fstop")?;
        let ptspersum = opt_i64(&positional, &named, 6, "ptspersum", 1);
        let cmd = format!("noise {output} {input_src} {spacing} {points} {fstart} {fstop} {ptspersum}");
        analysis(simulator, &cmd, "NoiseResult")
    }
}

// ── $tf(outvar, input_src) ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct TfTask;

impl SystemTask for TfTask {
    fn name(&self) -> &str { "tf" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let outvar    = require_str(&arguments, 0, "outvar")?.to_string();
        let input_src = require_str(&arguments, 1, "input_src")?.to_string();
        let cmd = format!("tf {outvar} {input_src}");
        analysis(simulator, &cmd, "TfResult")
    }
}

// ── $sens(outvar) ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SensTask;

impl SystemTask for SensTask {
    fn name(&self) -> &str { "sens" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let outvar = require_str(&arguments, 0, "outvar")?.to_string();
        let cmd = format!("sens {outvar}");
        analysis(simulator, &cmd, "SensResult")
    }
}

// ── $sens_ac(outvar, spacing, points, fstart, fstop) ─────────────────────────

#[derive(Debug)]
pub struct SensAcTask;

impl SystemTask for SensAcTask {
    fn name(&self) -> &str { "sens_ac" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let outvar  = require_str(&arguments, 0, "outvar")?.to_string();
        let spacing = require_str(&arguments, 1, "spacing")?.to_string();
        let points  = require_i64(&arguments, 2, "points")?;
        let fstart  = require_f64(&arguments, 3, "fstart")?;
        let fstop   = require_f64(&arguments, 4, "fstop")?;
        let cmd = format!("sens {outvar} ac {spacing} {points} {fstart} {fstop}");
        analysis(simulator, &cmd, "SensResult")
    }
}

// ── $pz(in_p, in_n, out_p, out_n, vol_or_cur, pol_zer_pz) ───────────────────

#[derive(Debug)]
pub struct PzTask;

impl SystemTask for PzTask {
    fn name(&self) -> &str { "pz" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let in_p    = require_str(&arguments, 0, "in_p")?.to_string();
        let in_n    = require_str(&arguments, 1, "in_n")?.to_string();
        let out_p   = require_str(&arguments, 2, "out_p")?.to_string();
        let out_n   = require_str(&arguments, 3, "out_n")?.to_string();
        let vol_cur = require_str(&arguments, 4, "vol_or_cur")?.to_string();
        let pz_type = require_str(&arguments, 5, "pol_zer_pz")?.to_string();
        let cmd = format!("pz {in_p} {in_n} {out_p} {out_n} {vol_cur} {pz_type}");
        analysis(simulator, &cmd, "PzResult")
    }
}

// ── $disto(spacing, points, fstart, fstop [, f2overf1]) ──────────────────────

#[derive(Debug)]
pub struct DistoTask;

impl SystemTask for DistoTask {
    fn name(&self) -> &str { "disto" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        self.call_named(arguments, HashMap::new(), simulator)
    }
    fn call_named(&self, positional: Vec<Value>, named: HashMap<String, Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let spacing = require_str(&positional, 0, "spacing")?.to_string();
        let points  = require_i64(&positional, 1, "points")?;
        let fstart  = require_f64(&positional, 2, "fstart")?;
        let fstop   = require_f64(&positional, 3, "fstop")?;
        let f2overf1 = opt_f64(&positional, &named, 4, "f2overf1", 0.9);
        let mut cmd = format!("disto {spacing} {points} {fstart} {fstop}");
        if (f2overf1 - 0.9).abs() > 1e-12 {
            cmd += &format!(" {f2overf1}");
        }
        analysis(simulator, &cmd, "DistoResult")
    }
}

// ── $pss(fguess, stabtime, points, harmonics) ────────────────────────────────

#[derive(Debug)]
pub struct PssTask;

impl SystemTask for PssTask {
    fn name(&self) -> &str { "pss" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let fguess    = require_f64(&arguments, 0, "fguess")?;
        let stabtime  = require_f64(&arguments, 1, "stabtime")?;
        let points    = require_i64(&arguments, 2, "points")?;
        let harmonics = require_i64(&arguments, 3, "harmonics")?;
        let cmd = format!("pss {fguess} {stabtime} {points} {harmonics}");
        analysis(simulator, &cmd, "PssResult")
    }
}

// ── $sp(spacing, points, fstart, fstop) ──────────────────────────────────────

#[derive(Debug)]
pub struct SpTask;

impl SystemTask for SpTask {
    fn name(&self) -> &str { "sp" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let spacing = require_str(&arguments, 0, "spacing")?.to_string();
        let points  = require_i64(&arguments, 1, "points")?;
        let fstart  = require_f64(&arguments, 2, "fstart")?;
        let fstop   = require_f64(&arguments, 3, "fstop")?;
        let cmd = format!("sp {spacing} {points} {fstart} {fstop}");
        analysis(simulator, &cmd, "SpResult")
    }
}

// ── $V("node" [, "ref"]) ─────────────────────────────────────────────────────

#[derive(Debug)]
pub struct VoltageTask;

impl SystemTask for VoltageTask {
    fn name(&self) -> &str { "V" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let node1 = require_str(&arguments, 0, "node")?.to_string();
        let query = if arguments.len() > 1 {
            let node2 = require_str(&arguments, 1, "ref")?.to_string();
            format!("v({node1}, {node2})")
        } else {
            format!("v({node1})")
        };
        let vector = simulator.get_vector(&query)?;
        let last = vector.last().copied().ok_or_else(|| {
            InterpreterError::SimulatorError(format!("vector {query} is empty"))
        })?;
        Ok(Some(Value::Real(last)))
    }
}

// ── $I("branch") ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct CurrentTask;

impl SystemTask for CurrentTask {
    fn name(&self) -> &str { "I" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let branch = require_str(&arguments, 0, "branch")?.to_string();
        let vector = simulator.get_vector(&format!("i({branch})"))
            .or_else(|_| simulator.get_vector(&format!("{branch}#branch")))?;
        let last = vector.last().copied().ok_or_else(|| {
            InterpreterError::SimulatorError(format!("vector i({branch}) is empty"))
        })?;
        Ok(Some(Value::Real(last)))
    }
}

// ── $meas family ─────────────────────────────────────────────────────────────
//
// All meas tasks issue a `.meas` command and read back the named result vector.
// A counter-based unique name avoids clashes between multiple $meas calls.

fn run_meas_and_read(
    cmd: &str,
    meas_name: &str,
    simulator: &mut dyn SimulatorBackend,
) -> Result<Option<Value>, InterpreterError> {
    simulator.run_command(cmd)?;
    let values = simulator.get_vector(meas_name)
        .unwrap_or_else(|_| vec![f64::NAN]);
    Ok(Some(Value::Real(*values.first().unwrap_or(&f64::NAN))))
}

/// `$meas(analysis_type, result_name, spec_string)` — raw ngspice `.meas` passthrough.
///
/// Example: `real bw = $meas("ac", "bw3db", "WHEN vdb(out)=-3 FALL=1");`
#[derive(Debug)]
pub struct MeasTask;

impl SystemTask for MeasTask {
    fn name(&self) -> &str { "meas" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let analysis  = require_str(&arguments, 0, "analysis_type")?.to_string();
        let meas_name = require_str(&arguments, 1, "result_name")?.to_string();
        let spec      = require_str(&arguments, 2, "spec_string")?.to_string();
        let cmd = format!("meas {analysis} {meas_name} {spec}");
        run_meas_and_read(&cmd, &meas_name, simulator)
    }
}

/// `$meas_find_at(analysis, signal, x)` — FIND signal AT=x.
#[derive(Debug)]
pub struct MeasFindAtTask;

impl SystemTask for MeasFindAtTask {
    fn name(&self) -> &str { "meas_find_at" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let analysis = require_str(&arguments, 0, "analysis")?.to_string();
        let signal   = require_str(&arguments, 1, "signal")?.to_string();
        let x        = require_f64(&arguments, 2, "x")?;
        let name = next_meas_name();
        let cmd = format!("meas {analysis} {name} FIND {signal} AT={x}");
        run_meas_and_read(&cmd, &name, simulator)
    }
}

/// `$meas_when(analysis, signal, value)` — find time/x when signal crosses value.
#[derive(Debug)]
pub struct MeasWhenTask;

impl SystemTask for MeasWhenTask {
    fn name(&self) -> &str { "meas_when" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let analysis = require_str(&arguments, 0, "analysis")?.to_string();
        let signal   = require_str(&arguments, 1, "signal")?.to_string();
        let value    = require_f64(&arguments, 2, "value")?;
        let name = next_meas_name();
        let cmd = format!("meas {analysis} {name} WHEN {signal}={value}");
        run_meas_and_read(&cmd, &name, simulator)
    }
}

/// `$meas_trig_targ(analysis, trig_sig, trig_val, trig_edge, trig_n, targ_sig, targ_val, targ_edge, targ_n)`.
#[derive(Debug)]
pub struct MeasTrigTargTask;

impl SystemTask for MeasTrigTargTask {
    fn name(&self) -> &str { "meas_trig_targ" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let analysis   = require_str(&arguments, 0, "analysis")?.to_string();
        let trig_sig   = require_str(&arguments, 1, "trig_sig")?.to_string();
        let trig_val   = require_f64(&arguments, 2, "trig_val")?;
        let trig_edge  = require_str(&arguments, 3, "trig_edge")?.to_string().to_uppercase();
        let trig_n     = require_i64(&arguments, 4, "trig_n")?;
        let targ_sig   = require_str(&arguments, 5, "targ_sig")?.to_string();
        let targ_val   = require_f64(&arguments, 6, "targ_val")?;
        let targ_edge  = require_str(&arguments, 7, "targ_edge")?.to_string().to_uppercase();
        let targ_n     = require_i64(&arguments, 8, "targ_n")?;
        let name = next_meas_name();
        let cmd = format!(
            "meas {analysis} {name} TRIG {trig_sig} VAL={trig_val} {trig_edge}={trig_n} \
             TARG {targ_sig} VAL={targ_val} {targ_edge}={targ_n}"
        );
        run_meas_and_read(&cmd, &name, simulator)
    }
}

/// `$meas_rms(analysis, signal, from, to)` — RMS over [from, to].
#[derive(Debug)]
pub struct MeasRmsTask;

impl SystemTask for MeasRmsTask {
    fn name(&self) -> &str { "meas_rms" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let analysis = require_str(&arguments, 0, "analysis")?.to_string();
        let signal   = require_str(&arguments, 1, "signal")?.to_string();
        let from     = require_f64(&arguments, 2, "from")?;
        let to       = require_f64(&arguments, 3, "to")?;
        let name = next_meas_name();
        let cmd = format!("meas {analysis} {name} RMS {signal} FROM={from} TO={to}");
        run_meas_and_read(&cmd, &name, simulator)
    }
}

/// `$meas_avg(analysis, signal, from, to)` — average over [from, to].
#[derive(Debug)]
pub struct MeasAvgTask;

impl SystemTask for MeasAvgTask {
    fn name(&self) -> &str { "meas_avg" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let analysis = require_str(&arguments, 0, "analysis")?.to_string();
        let signal   = require_str(&arguments, 1, "signal")?.to_string();
        let from     = require_f64(&arguments, 2, "from")?;
        let to       = require_f64(&arguments, 3, "to")?;
        let name = next_meas_name();
        let cmd = format!("meas {analysis} {name} AVG {signal} FROM={from} TO={to}");
        run_meas_and_read(&cmd, &name, simulator)
    }
}

/// `$meas_min(analysis, signal)` — minimum value.
#[derive(Debug)]
pub struct MeasMinTask;

impl SystemTask for MeasMinTask {
    fn name(&self) -> &str { "meas_min" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let analysis = require_str(&arguments, 0, "analysis")?.to_string();
        let signal   = require_str(&arguments, 1, "signal")?.to_string();
        let name = next_meas_name();
        let cmd = format!("meas {analysis} {name} MIN {signal}");
        run_meas_and_read(&cmd, &name, simulator)
    }
}

/// `$meas_max(analysis, signal)` — maximum value.
#[derive(Debug)]
pub struct MeasMaxTask;

impl SystemTask for MeasMaxTask {
    fn name(&self) -> &str { "meas_max" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let analysis = require_str(&arguments, 0, "analysis")?.to_string();
        let signal   = require_str(&arguments, 1, "signal")?.to_string();
        let name = next_meas_name();
        let cmd = format!("meas {analysis} {name} MAX {signal}");
        run_meas_and_read(&cmd, &name, simulator)
    }
}

/// `$meas_max_at(analysis, signal)` — time/x at which signal reaches its maximum.
#[derive(Debug)]
pub struct MeasMaxAtTask;

impl SystemTask for MeasMaxAtTask {
    fn name(&self) -> &str { "meas_max_at" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let analysis = require_str(&arguments, 0, "analysis")?.to_string();
        let signal   = require_str(&arguments, 1, "signal")?.to_string();
        let name = next_meas_name();
        let cmd = format!("meas {analysis} {name} MAX {signal} AT");
        run_meas_and_read(&cmd, &name, simulator)
    }
}

/// `$meas_integral(analysis, signal, from, to)` — integral (area under curve).
#[derive(Debug)]
pub struct MeasIntegralTask;

impl SystemTask for MeasIntegralTask {
    fn name(&self) -> &str { "meas_integral" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let analysis = require_str(&arguments, 0, "analysis")?.to_string();
        let signal   = require_str(&arguments, 1, "signal")?.to_string();
        let from     = require_f64(&arguments, 2, "from")?;
        let to       = require_f64(&arguments, 3, "to")?;
        let name = next_meas_name();
        let cmd = format!("meas {analysis} {name} INTEG {signal} FROM={from} TO={to}");
        run_meas_and_read(&cmd, &name, simulator)
    }
}

// ── $alter(dev, param, value) ────────────────────────────────────────────────

#[derive(Debug)]
pub struct AlterTask;

impl SystemTask for AlterTask {
    fn name(&self) -> &str { "alter" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let dev   = require_str(&arguments, 0, "dev")?.to_string();
        let param = require_str(&arguments, 1, "param")?.to_string();
        let val   = require_f64(&arguments, 2, "value")?;
        simulator.run_command(&format!("alter {dev} {param} = {val}"))?;
        Ok(None)
    }
}

// ── $altermod(model, param, value) ───────────────────────────────────────────

#[derive(Debug)]
pub struct AltermodTask;

impl SystemTask for AltermodTask {
    fn name(&self) -> &str { "altermod" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let model = require_str(&arguments, 0, "model")?.to_string();
        let param = require_str(&arguments, 1, "param")?.to_string();
        let val   = require_f64(&arguments, 2, "value")?;
        simulator.run_command(&format!("altermod {model} {param} = {val}"))?;
        Ok(None)
    }
}

// ── $alterparam(param, value) ────────────────────────────────────────────────

#[derive(Debug)]
pub struct AlterparamTask;

impl SystemTask for AlterparamTask {
    fn name(&self) -> &str { "alterparam" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let param = require_str(&arguments, 0, "param")?.to_string();
        let val   = require_f64(&arguments, 1, "value")?;
        simulator.run_command(&format!("alterparam {param} = {val}"))?;
        simulator.run_command("reset")?;
        Ok(None)
    }
}

// ── $set_option(opt, val) ────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SetOptionTask;

impl SystemTask for SetOptionTask {
    fn name(&self) -> &str { "set_option" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let opt = require_str(&arguments, 0, "opt")?.to_string();
        let val = arguments.get(1);
        match val {
            Some(Value::Real(v)) => simulator.run_command(&format!("option {opt}={v}"))?,
            Some(Value::String(s)) => simulator.run_command(&format!("option {opt}={s}"))?,
            Some(Value::Integer(i)) => simulator.run_command(&format!("option {opt}={i}"))?,
            _ => simulator.run_command(&format!("option {opt}"))?, // flag
        }
        Ok(None)
    }
}

// ── $set_temp(t) ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SetTempTask;

impl SystemTask for SetTempTask {
    fn name(&self) -> &str { "set_temp" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let temp = require_f64(&arguments, 0, "temp")?;
        simulator.run_command(&format!("set temp = {temp}"))?;
        Ok(None)
    }
}

// ── $set_tnom(t) ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SetTnomTask;

impl SystemTask for SetTnomTask {
    fn name(&self) -> &str { "set_tnom" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let tnom = require_f64(&arguments, 0, "tnom")?;
        simulator.run_command(&format!("set tnom = {tnom}"))?;
        Ok(None)
    }
}

// ── $get_vec(name) ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct GetVecTask;

impl SystemTask for GetVecTask {
    fn name(&self) -> &str { "get_vec" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let name = require_str(&arguments, 0, "name")?.to_string();
        let vector = simulator.get_vector(&name)?;
        Ok(Some(Value::RealVec(vector.clone())))
    }
}
