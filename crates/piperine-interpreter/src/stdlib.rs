//! Standard library system tasks available in every Piperine runtime,
//! regardless of simulator backend.
//!
//! These tasks implement language-level semantics (I/O, error signalling) and
//! do NOT depend on any specific simulator. They are registered automatically
//! by `SystemTaskRegistry::default()`.

use crate::backend::SimulatorBackend;
use crate::error::InterpreterError;
use crate::task::SystemTask;
use crate::value::Value;

// ── $display(fmt, args...) ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct DisplayTask;

impl SystemTask for DisplayTask {
    fn name(&self) -> &str { "display" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let output = if arguments.is_empty() {
            String::new()
        } else {
            let fmt = arguments[0].as_str().ok_or_else(|| InterpreterError::TypeError {
                expected: "string format".into(),
                got: arguments[0].type_name().into(),
            })?.to_string();
            format_string(&fmt, &arguments[1..])
        };
        simulator.print(&output);
        Ok(None)
    }
}

// ── $write(fmt, args...) ─────────────────────────────────────────────────────
// Like $display but without trailing newline (matches SV semantics).

#[derive(Debug)]
pub struct WriteTask;

impl SystemTask for WriteTask {
    fn name(&self) -> &str { "write" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let output = if arguments.is_empty() {
            String::new()
        } else {
            let fmt = arguments[0].as_str().ok_or_else(|| InterpreterError::TypeError {
                expected: "string format".into(),
                got: arguments[0].type_name().into(),
            })?.to_string();
            format_string(&fmt, &arguments[1..])
        };
        simulator.print(&output);
        Ok(None)
    }
}

// ── $warning(fmt, args...) ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct WarningTask;

impl SystemTask for WarningTask {
    fn name(&self) -> &str { "warning" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let msg = if arguments.is_empty() {
            "warning".into()
        } else {
            format_string(arguments[0].as_str().unwrap_or_default(), &arguments[1..])
        };
        simulator.print(&format!("WARNING: {msg}"));
        Ok(None)
    }
}

// ── $run_error(fmt, args...) ─────────────────────────────────────────────────
// Non-fatal: raises RunFailed (marks current run as failed, but does not halt).

#[derive(Debug)]
pub struct RunErrorTask;

impl SystemTask for RunErrorTask {
    fn name(&self) -> &str { "run_error" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let msg = if arguments.is_empty() {
            "run failed".into()
        } else {
            format_string(arguments[0].as_str().unwrap_or_default(), &arguments[1..])
        };
        Err(InterpreterError::RunFailed { message: msg })
    }
}

// ── $fatal([exit_code,] fmt, args...) ────────────────────────────────────────
// Fatal: halts the interpreter unconditionally.

#[derive(Debug)]
pub struct FatalTask;

impl SystemTask for FatalTask {
    fn name(&self) -> &str { "fatal" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let mut exit_code = 1u32;
        let mut fmt_idx = 0;
        if !arguments.is_empty() && matches!(arguments[0], Value::Integer(_)) {
            exit_code = arguments[0].as_integer().unwrap_or(1) as u32;
            fmt_idx = 1;
        }
        let msg = if fmt_idx < arguments.len() {
            format_string(arguments[fmt_idx].as_str().unwrap_or_default(), &arguments[(fmt_idx + 1)..])
        } else {
            "fatal error".into()
        };
        Err(InterpreterError::Fatal { message: msg, exit_code })
    }
}

// ── $error(fmt, args...) ─────────────────────────────────────────────────────
// Alias for $fatal(1, ...) — matches SV `$error` semantics.

#[derive(Debug)]
pub struct ErrorTask;

impl SystemTask for ErrorTask {
    fn name(&self) -> &str { "error" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let msg = if arguments.is_empty() {
            "error".into()
        } else {
            format_string(arguments[0].as_str().unwrap_or_default(), &arguments[1..])
        };
        Err(InterpreterError::Fatal { message: msg, exit_code: 1 })
    }
}

// ── $sformatf(fmt, args...) → string ─────────────────────────────────────────
// Returns a formatted string value (does not print). Used for string building.

#[derive(Debug)]
pub struct SFormatfTask;

impl SystemTask for SFormatfTask {
    fn name(&self) -> &str { "sformatf" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        if arguments.is_empty() {
            return Ok(Some(Value::String(String::new())));
        }
        let fmt = arguments[0].as_str().ok_or_else(|| InterpreterError::TypeError {
            expected: "string format".into(),
            got: arguments[0].type_name().into(),
        })?.to_string();
        Ok(Some(Value::String(format_string(&fmt, &arguments[1..]))))
    }
}

// ── $abs(x) → real / integer ─────────────────────────────────────────────────

#[derive(Debug)]
pub struct AbsTask;

impl SystemTask for AbsTask {
    fn name(&self) -> &str { "abs" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        match arguments.first() {
            Some(Value::Real(v))    => Ok(Some(Value::Real(v.abs()))),
            Some(Value::Integer(i)) => Ok(Some(Value::Integer(i.abs()))),
            other => Err(InterpreterError::TypeError {
                expected: "numeric".into(),
                got: other.map(|v| v.type_name()).unwrap_or("nothing").into(),
            }),
        }
    }
}

// ── $min(a, b) / $max(a, b) ──────────────────────────────────────────────────

#[derive(Debug)]
pub struct MinTask;

impl SystemTask for MinTask {
    fn name(&self) -> &str { "min" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let a = arguments.get(0).and_then(|v| v.as_f64());
        let b = arguments.get(1).and_then(|v| v.as_f64());
        match (a, b) {
            (Some(a), Some(b)) => Ok(Some(Value::Real(a.min(b)))),
            _ => Err(InterpreterError::TypeError { expected: "two numeric args".into(), got: format!("{} args", arguments.len()) }),
        }
    }
}

#[derive(Debug)]
pub struct MaxTask;

impl SystemTask for MaxTask {
    fn name(&self) -> &str { "max" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let a = arguments.get(0).and_then(|v| v.as_f64());
        let b = arguments.get(1).and_then(|v| v.as_f64());
        match (a, b) {
            (Some(a), Some(b)) => Ok(Some(Value::Real(a.max(b)))),
            _ => Err(InterpreterError::TypeError { expected: "two numeric args".into(), got: format!("{} args", arguments.len()) }),
        }
    }
}

// ── Math ($sqrt, $sin, $pow, …) ──────────────────────────────────────────────
//
// Verilog-AMS real math functions. Unary and binary forms cover the common
// analog needs (gains, dB, time constants, RMS, geometry).

fn arg_real(args: &[Value], idx: usize, task: &str) -> Result<f64, InterpreterError> {
    args.get(idx).and_then(|v| v.as_f64()).ok_or_else(|| InterpreterError::TypeError {
        expected: format!("real argument {idx} for ${task}"),
        got: args.get(idx).map(|v| v.type_name()).unwrap_or("nothing").into(),
    })
}

/// A one-argument real math function: `$sqrt(x)`, `$sin(x)`, …
pub struct UnaryMathTask {
    pub task_name: &'static str,
    pub func: fn(f64) -> f64,
}

impl std::fmt::Debug for UnaryMathTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnaryMathTask({})", self.task_name)
    }
}

impl SystemTask for UnaryMathTask {
    fn name(&self) -> &str { self.task_name }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let x = arg_real(&arguments, 0, self.task_name)?;
        Ok(Some(Value::Real((self.func)(x))))
    }
}

/// A two-argument real math function: `$pow(x, y)`, `$atan2(y, x)`, `$hypot(x, y)`.
pub struct BinaryMathTask {
    pub task_name: &'static str,
    pub func: fn(f64, f64) -> f64,
}

impl std::fmt::Debug for BinaryMathTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinaryMathTask({})", self.task_name)
    }
}

impl SystemTask for BinaryMathTask {
    fn name(&self) -> &str { self.task_name }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let a = arg_real(&arguments, 0, self.task_name)?;
        let b = arg_real(&arguments, 1, self.task_name)?;
        Ok(Some(Value::Real((self.func)(a, b))))
    }
}

/// `$clog2(n)` — ceiling of log2(n): the bit width needed to index `n` values.
/// `$clog2(1) = 0`, `$clog2(2) = 1`, `$clog2(5) = 3`. Returns an integer.
#[derive(Debug)]
pub struct Clog2Task;

impl SystemTask for Clog2Task {
    fn name(&self) -> &str { "clog2" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let n = arguments.first().and_then(|v| v.as_integer()).unwrap_or(0);
        let bits = if n <= 1 { 0 } else { 64 - ((n - 1) as u64).leading_zeros() as i64 };
        Ok(Some(Value::Integer(bits)))
    }
}

// ── Format string engine ─────────────────────────────────────────────────────
//
// Shared by $display, $write, $warning, $sformatf, $run_error, $fatal.
// Supports: %g %e %f %d %s %b %o %h %% and width specifiers (%0d, %8.3f, etc.).

pub fn format_string(format: &str, arguments: &[Value]) -> String {
    let mut out = String::with_capacity(format.len());
    let mut chars = format.chars().peekable();
    let mut arg_idx = 0;

    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // Consume optional width / precision (e.g., `%8.3f`, `%0d`, `%-10s`)
        let mut spec = String::new();
        if chars.peek() == Some(&'-') { spec.push(chars.next().unwrap()); }
        while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            spec.push(chars.next().unwrap());
        }
        if chars.peek() == Some(&'.') {
            spec.push(chars.next().unwrap());
            while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                spec.push(chars.next().unwrap());
            }
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('g') | Some('G') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_f64()).unwrap_or(0.0);
                out.push_str(&format!("{v:e}").replace("e0", "").replace("e", "e+"));
                // Simple %g: let Rust decide; good enough for engineering use.
                out = out.trim_end_matches(&out.clone()).to_string(); // reset trick
                out.push_str(&format!("{v}"));
                arg_idx += 1;
            }
            Some('e') | Some('E') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_f64()).unwrap_or(0.0);
                out.push_str(&format!("{v:e}"));
                arg_idx += 1;
            }
            Some('f') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let prec: usize = spec.split('.').nth(1).and_then(|s| s.parse().ok()).unwrap_or(6);
                out.push_str(&format!("{v:.prec$}"));
                arg_idx += 1;
            }
            Some('d') | Some('i') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_integer()).unwrap_or(0);
                out.push_str(&format!("{v}"));
                arg_idx += 1;
            }
            Some('s') => {
                let v = arguments.get(arg_idx).map(|v| v.to_string()).unwrap_or_default();
                out.push_str(&v);
                arg_idx += 1;
            }
            Some('b') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_integer()).unwrap_or(0);
                out.push_str(&format!("{v:b}"));
                arg_idx += 1;
            }
            Some('o') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_integer()).unwrap_or(0);
                out.push_str(&format!("{v:o}"));
                arg_idx += 1;
            }
            Some('h') | Some('x') | Some('X') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_integer()).unwrap_or(0);
                out.push_str(&format!("{v:x}"));
                arg_idx += 1;
            }
            Some(other) => { out.push('%'); out.push_str(&spec); out.push(other); }
            None        => { out.push('%'); out.push_str(&spec); }
        }
    }
    out
}

// ── Randomization ($urandom, $dist_normal, …) ────────────────────────────────
//
// One process-wide generator (xorshift64*). Calls advance it; `$srandom(seed)`
// or a seed argument resets it for reproducible runs. SystemVerilog threads an
// `inout seed` through `$dist_*`; we can't write back through a by-value arg, so
// the seed argument (when present and non-zero) reseeds the shared generator and
// the sequence is otherwise global. Good enough for Monte Carlo sweeps.

// One generator per thread: a real run drives a testbench on a single thread, so
// `$srandom(seed)` reproducibility holds, and parallel test threads never share state.
thread_local! {
    static RNG_STATE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Next raw 64-bit value, seeding lazily from the clock on first use.
fn rng_next() -> u64 {
    RNG_STATE.with(|state| {
        let mut x = state.get();
        if x == 0 {
            x = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x9E3779B97F4A7C15)
                | 1;
        }
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        state.set(x);
        x.wrapping_mul(0x2545F4914F6CDD1D)
    })
}

fn rng_seed(seed: u64) {
    RNG_STATE.with(|state| state.set(seed | 1));
}

/// Uniform real in `[0, 1)`.
fn rng_unit() -> f64 {
    (rng_next() >> 11) as f64 / (1u64 << 53) as f64
}

/// Uniform integer in `[lo, hi]` inclusive (order-independent).
fn rng_range(a: i64, b: i64) -> i64 {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let span = (hi - lo) as u64 + 1;
    lo + (rng_next() % span) as i64
}

/// `$srandom(seed)` — reseed the shared generator.
#[derive(Debug)]
pub struct SRandomTask;
impl SystemTask for SRandomTask {
    fn name(&self) -> &str { "srandom" }
    fn call(&self, args: Vec<Value>, _: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let seed = args.first().and_then(|v| v.as_integer()).unwrap_or(0);
        rng_seed(seed as u64);
        Ok(None)
    }
}

/// `$urandom([seed])` — unsigned 32-bit random.
#[derive(Debug)]
pub struct URandomTask;
impl SystemTask for URandomTask {
    fn name(&self) -> &str { "urandom" }
    fn call(&self, args: Vec<Value>, _: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        if let Some(seed) = args.first().and_then(|v| v.as_integer()) {
            if seed != 0 { rng_seed(seed as u64); }
        }
        Ok(Some(Value::Integer((rng_next() as u32) as i64)))
    }
}

/// `$random([seed])` — signed 32-bit random.
#[derive(Debug)]
pub struct RandomTask;
impl SystemTask for RandomTask {
    fn name(&self) -> &str { "random" }
    fn call(&self, args: Vec<Value>, _: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        if let Some(seed) = args.first().and_then(|v| v.as_integer()) {
            if seed != 0 { rng_seed(seed as u64); }
        }
        Ok(Some(Value::Integer((rng_next() as u32 as i32) as i64)))
    }
}

/// `$urandom_range(maxval [, minval=0])` — uniform integer, bounds inclusive.
#[derive(Debug)]
pub struct URandomRangeTask;
impl SystemTask for URandomRangeTask {
    fn name(&self) -> &str { "urandom_range" }
    fn call(&self, args: Vec<Value>, _: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let maxval = args.first().and_then(|v| v.as_integer()).ok_or_else(|| {
            InterpreterError::TypeError { expected: "integer maxval".into(), got: "nothing".into() }
        })?;
        let minval = args.get(1).and_then(|v| v.as_integer()).unwrap_or(0);
        Ok(Some(Value::Integer(rng_range(minval, maxval))))
    }
}

/// `$dist_uniform(seed, start, end)` — uniform integer in `[start, end]`.
#[derive(Debug)]
pub struct DistUniformTask;
impl SystemTask for DistUniformTask {
    fn name(&self) -> &str { "dist_uniform" }
    fn call(&self, args: Vec<Value>, _: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        maybe_reseed(&args);
        let start = arg_real(&args, 1, "dist_uniform")? as i64;
        let end   = arg_real(&args, 2, "dist_uniform")? as i64;
        Ok(Some(Value::Integer(rng_range(start, end))))
    }
}

/// `$dist_normal(seed, mean, std_deviation)` — Gaussian (returns real).
///
/// Note: SystemVerilog returns an integer here; Piperine returns a real, which
/// is what analog component-tolerance Monte Carlo actually wants.
#[derive(Debug)]
pub struct DistNormalTask;
impl SystemTask for DistNormalTask {
    fn name(&self) -> &str { "dist_normal" }
    fn call(&self, args: Vec<Value>, _: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        maybe_reseed(&args);
        let mean = arg_real(&args, 1, "dist_normal")?;
        let std  = arg_real(&args, 2, "dist_normal")?;
        // Box–Muller transform.
        let u1 = rng_unit().max(f64::MIN_POSITIVE);
        let u2 = rng_unit();
        let z = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
        Ok(Some(Value::Real(mean + std * z)))
    }
}

/// `$dist_exponential(seed, mean)` — exponential distribution (returns real).
#[derive(Debug)]
pub struct DistExponentialTask;
impl SystemTask for DistExponentialTask {
    fn name(&self) -> &str { "dist_exponential" }
    fn call(&self, args: Vec<Value>, _: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        maybe_reseed(&args);
        let mean = arg_real(&args, 1, "dist_exponential")?;
        let u = rng_unit().max(f64::MIN_POSITIVE);
        Ok(Some(Value::Real(-mean * u.ln())))
    }
}

/// `$dist_*` take `seed` as their first argument; a non-zero seed reseeds.
fn maybe_reseed(args: &[Value]) {
    if let Some(seed) = args.first().and_then(|v| v.as_integer()) {
        if seed != 0 { rng_seed(seed as u64); }
    }
}

/// All stdlib tasks. Called by `SystemTaskRegistry::default()`.
pub fn register_stdlib(registry: &mut crate::task::SystemTaskRegistry) {
    registry.register(Box::new(DisplayTask));
    registry.register(Box::new(WriteTask));
    registry.register(Box::new(WarningTask));
    registry.register(Box::new(RunErrorTask));
    registry.register(Box::new(FatalTask));
    registry.register(Box::new(ErrorTask));
    registry.register(Box::new(SFormatfTask));
    registry.register(Box::new(AbsTask));
    registry.register(Box::new(MinTask));
    registry.register(Box::new(MaxTask));

    // Unary real math
    for (name, func) in [
        ("sqrt",  f64::sqrt   as fn(f64) -> f64),
        ("ln",    f64::ln),
        ("log10", f64::log10),
        ("exp",   f64::exp),
        ("sin",   f64::sin),
        ("cos",   f64::cos),
        ("tan",   f64::tan),
        ("asin",  f64::asin),
        ("acos",  f64::acos),
        ("atan",  f64::atan),
        ("sinh",  f64::sinh),
        ("cosh",  f64::cosh),
        ("tanh",  f64::tanh),
        ("floor", f64::floor),
        ("ceil",  f64::ceil),
    ] {
        registry.register(Box::new(UnaryMathTask { task_name: name, func }));
    }

    // Binary real math
    for (name, func) in [
        ("pow",   f64::powf as fn(f64, f64) -> f64),
        ("atan2", f64::atan2),
        ("hypot", f64::hypot),
    ] {
        registry.register(Box::new(BinaryMathTask { task_name: name, func }));
    }

    registry.register(Box::new(Clog2Task));

    // Randomization
    registry.register(Box::new(SRandomTask));
    registry.register(Box::new(RandomTask));
    registry.register(Box::new(URandomTask));
    registry.register(Box::new(URandomRangeTask));
    registry.register(Box::new(DistUniformTask));
    registry.register(Box::new(DistNormalTask));
    registry.register(Box::new(DistExponentialTask));
}
