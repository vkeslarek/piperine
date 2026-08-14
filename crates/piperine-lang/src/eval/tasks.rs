//! LRM chapter 9 — general system tasks and functions.
//!
//! **Scope of this file (T47, VAMS-15).** `piperine_vams::ast::Stmt::TaskCall`
//! and `Expr::SystemFunctionCall` are parsed (`crates/piperine-vams`) but —
//! per the T47 survey — nothing in `piperine-lang`'s elaboration pipeline
//! lowers a `$`-task call into the POM yet; there is no VAMS-shaped runtime
//! interpreter to invoke against (`eval/mod.rs`'s own `//!` header records
//! that the PHDL-era `Interpreter`/`Host` died with `parse/`, MD-45, and a
//! replacement is "future work — not reintroduced speculatively"). Building
//! that lowering/interpreter is out of this task's stated `Where`
//! (`eval/tasks.rs` only) — **SPEC_DEVIATION**, carried forward like T48–T52's
//! own documented gaps: this module delivers the LRM ch. 9 *semantics* as
//! pure, directly testable functions —
//!
//! - [`SystemTasks::resolve`] — the fail-loud name table (every task/function
//!   this module implements), with near-match suggestions for a typo'd name.
//! - [`format_display`] — the `$display`/`$write`/`$strobe`/`$monitor`-family
//!   format-string evaluator (LRM Table 9-1, §9.2).
//! - [`SimEnvironment`] + its accessors — `$abstime`/`$temperature`/
//!   `$bound_step` (§9.7, §17.2, §9.5.2).
//! - conversion functions `$itor`/`$rtoi`/`$realtobits`/`$bitstoreal`/
//!   `$signed`/`$unsigned` (§9.13).
//!
//! so that whichever pass eventually lowers `TaskCall`/`SystemFunctionCall`
//! (a follow-up, not speculative here) has a ready, independently-verified
//! implementation to call into — never a second, ad hoc copy.

use crate::value::Value;

/// One LRM ch. 9 system task/function this module implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemTaskKind {
    /// `$write` — like `$display` but no trailing newline (§9.2).
    Write,
    /// `$strobe` — like `$display`, scheduled for end-of-timestep (§9.2).
    Strobe,
    /// `$display` — format and print, trailing newline (§9.2).
    Display,
    /// `$monitor` — like `$display`, re-armed on every operand change (§9.2).
    Monitor,
    /// `$info` — non-fatal diagnostic (§17.1, `severity system tasks`).
    Info,
    /// `$warning` — non-fatal diagnostic (§17.1).
    Warning,
    /// `$error` — non-fatal diagnostic (§17.1).
    Error,
    /// `$fatal` — fatal diagnostic, terminates simulation (§17.1).
    Fatal,
    /// `$finish` — terminate simulation (§9.5.1).
    Finish,
    /// `$stop` — suspend simulation (§9.5.1).
    Stop,
    /// `$discontinuity` — analog discontinuity hint (§9.5.3, Verilog-AMS).
    Discontinuity,
    /// `$bound_step` — bounds the next analog timestep (§9.5.2).
    BoundStep,
    /// `$abstime` — the current absolute simulation time (§17.2).
    Abstime,
    /// `$temperature` — the circuit's ambient temperature (§17.2).
    Temperature,
    /// `$itor` — integer to real (§9.13/§17.3.2).
    Itor,
    /// `$rtoi` — real to integer, truncating (§9.13).
    Rtoi,
    /// `$realtobits` — real to its 64-bit IEEE-754 pattern (§9.13).
    RealToBits,
    /// `$bitstoreal` — 64-bit pattern to real (§9.13).
    BitsToReal,
    /// `$signed` — reinterpret bits as signed (§9.13).
    Signed,
    /// `$unsigned` — reinterpret bits as unsigned (§9.13).
    Unsigned,
}

/// The empty type this module hangs its table lookups and helpers off of
/// (MD-13: no loose functions).
pub struct SystemTasks;

impl SystemTasks {
    /// Every name this module implements, paired with its kind. Data table,
    /// not a `match` — `resolve`/near-match search both read it.
    const TABLE: &'static [(&'static str, SystemTaskKind)] = &[
        ("$write", SystemTaskKind::Write),
        ("$strobe", SystemTaskKind::Strobe),
        ("$display", SystemTaskKind::Display),
        ("$monitor", SystemTaskKind::Monitor),
        ("$info", SystemTaskKind::Info),
        ("$warning", SystemTaskKind::Warning),
        ("$error", SystemTaskKind::Error),
        ("$fatal", SystemTaskKind::Fatal),
        ("$finish", SystemTaskKind::Finish),
        ("$stop", SystemTaskKind::Stop),
        ("$discontinuity", SystemTaskKind::Discontinuity),
        ("$bound_step", SystemTaskKind::BoundStep),
        ("$abstime", SystemTaskKind::Abstime),
        ("$temperature", SystemTaskKind::Temperature),
        ("$itor", SystemTaskKind::Itor),
        ("$rtoi", SystemTaskKind::Rtoi),
        ("$realtobits", SystemTaskKind::RealToBits),
        ("$bitstoreal", SystemTaskKind::BitsToReal),
        ("$signed", SystemTaskKind::Signed),
        ("$unsigned", SystemTaskKind::Unsigned),
    ];

    /// Resolves a `$name` to its [`SystemTaskKind`]. Unknown names fail loud
    /// with the closest matches from [`Self::TABLE`] (edit-distance <= 3,
    /// top 3), so a typo doesn't have to be diagnosed by re-reading ch. 9.
    pub fn resolve(name: &str) -> Result<SystemTaskKind, SystemTaskError> {
        Self::TABLE
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, k)| *k)
            .ok_or_else(|| SystemTaskError::unknown(name, Self::near_matches(name)))
    }

    /// The (up to 3) known names closest to `name` by Levenshtein distance,
    /// ordered nearest-first; ties broken by table order. Names farther than
    /// distance 3 are not offered — an unrelated name is not a "did you
    /// mean", it's just wrong.
    fn near_matches(name: &str) -> Vec<String> {
        const MAX_DISTANCE: usize = 3;
        const MAX_SUGGESTIONS: usize = 3;
        let mut scored: Vec<(usize, &str)> = Self::TABLE
            .iter()
            .map(|(n, _)| (Self::levenshtein(name, n), *n))
            .filter(|(d, _)| *d <= MAX_DISTANCE)
            .collect();
        scored.sort_by_key(|(d, _)| *d);
        scored.into_iter().take(MAX_SUGGESTIONS).map(|(_, n)| n.to_string()).collect()
    }

    /// Classic Wagner–Fischer edit distance over `char`s. Small alphabet
    /// (`$` + ASCII identifier text), so the `O(len(a) * len(b))` table is
    /// fine at these string lengths.
    fn levenshtein(a: &str, b: &str) -> usize {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        let (la, lb) = (a.len(), b.len());
        let mut row: Vec<usize> = (0..=lb).collect();
        for i in 1..=la {
            let mut prev_diag = row[0];
            row[0] = i;
            for j in 1..=lb {
                let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
                let above = row[j];
                let insert_del = row[j - 1].min(row[j]) + 1;
                let subst = prev_diag + cost;
                row[j] = insert_del.min(subst);
                prev_diag = above;
            }
        }
        row[lb]
    }
}

/// LRM Table 9-1 format-string evaluator shared by `$write`/`$strobe`/
/// `$display`/`$monitor` — they differ only in *when* they fire and whether
/// a trailing newline is appended (`Display`/`Monitor` yes, `Write`/`Strobe`
/// no per §9.2), never in how `%`-specifiers render, so callers pick that up
/// via `newline` rather than this module carrying four near-identical copies.
///
/// Supported specifiers: `%d`/`%o`/`%b`/`%h`|`%x` (integer radices), `%c`
/// (character code), `%s` (string), `%e`/`%f`/`%g` (real), `%t` (time,
/// rendered as its raw real value — no `$timeformat` scaling in this pure
/// evaluator), `%v` (net strength, rendered as its logic value — no
/// strength-model plumbing here), `%m` (hierarchical name, substituted from
/// `scope`), `%%` (a literal `%`).
pub fn format_display(fmt: &str, args: &[Value], scope: &str) -> Result<String, SystemTaskError> {
    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    let mut args = args.iter();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let spec = chars
            .next()
            .ok_or_else(|| SystemTaskError::malformed(fmt, "trailing `%` with no specifier".to_string()))?;
        if spec == '%' {
            out.push('%');
            continue;
        }
        if spec == 'm' {
            out.push_str(scope);
            continue;
        }
        let arg = args
            .next()
            .ok_or_else(|| SystemTaskError::malformed(fmt, format!("`%{spec}` has no matching argument")))?;
        out.push_str(&render_spec(spec, arg, fmt)?);
    }
    Ok(out)
}

fn render_spec(spec: char, arg: &Value, fmt: &str) -> Result<String, SystemTaskError> {
    let need_int = || {
        arg.as_integer()
            .or_else(|| arg.as_natural().map(|n| n as i64))
            .ok_or_else(|| {
                SystemTaskError::malformed(
                    fmt,
                    format!("`%{spec}` needs an integer argument, got {}", arg.type_name()),
                )
            })
    };
    let need_real = || {
        arg.coerce_real().map_err(|_| {
            SystemTaskError::malformed(fmt, format!("`%{spec}` needs a real argument, got {}", arg.type_name()))
        })
    };
    match spec {
        'd' | 'D' => Ok(format!("{}", need_int()?)),
        'o' | 'O' => Ok(format!("{:o}", need_int()?)),
        'b' | 'B' => Ok(format!("{:b}", need_int()?)),
        'h' | 'H' | 'x' | 'X' => Ok(format!("{:x}", need_int()?)),
        'c' | 'C' => {
            let code = need_int()?;
            let byte = u8::try_from(code)
                .map_err(|_| SystemTaskError::malformed(fmt, format!("`%c` code {code} is out of byte range")))?;
            Ok((byte as char).to_string())
        }
        's' | 'S' => Ok(arg.as_string().map(str::to_string).unwrap_or_else(|| arg.to_string())),
        'e' | 'E' => Ok(format!("{:e}", need_real()?)),
        'f' | 'F' => Ok(format!("{:.6}", need_real()?)),
        'g' | 'G' => Ok(format!("{}", need_real()?)),
        't' | 'T' => Ok(format!("{}", need_real()?)),
        'v' | 'V' => Ok(format!("{}", need_int()?)),
        other => Err(SystemTaskError::malformed(
            fmt,
            format!("`%{other}` is not a recognized LRM Table 9-1 specifier"),
        )),
    }
}

/// The live values `$abstime`/`$temperature`/`$bound_step` read — supplied
/// by whatever runtime eventually calls into this module (T47's scope ends
/// at the pure query, not at wiring a real clock/thermostat in).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimEnvironment {
    pub abstime: f64,
    pub temperature: f64,
    pub time_step: f64,
}

impl SimEnvironment {
    pub fn new(abstime: f64, temperature: f64, time_step: f64) -> Self {
        Self { abstime, temperature, time_step }
    }

    /// `$abstime` (§17.2) — the current absolute simulation time, seconds.
    pub fn abstime(&self) -> Value {
        Value::Real(self.abstime)
    }

    /// `$temperature` (§17.2) — the circuit's ambient temperature, Kelvin.
    pub fn temperature(&self) -> Value {
        Value::Real(self.temperature)
    }

    /// `$bound_step(step)` (§9.5.2) — bounds the next analog timestep to
    /// `min(step, requested)`; requesting a non-positive bound fails loud
    /// (LRM requires a positive time bound).
    pub fn bound_step(&self, requested: f64) -> Result<f64, SystemTaskError> {
        if requested <= 0.0 {
            return Err(SystemTaskError::invalid_argument(
                "$bound_step",
                format!("bound must be positive, got {requested}"),
            ));
        }
        Ok(self.time_step.min(requested))
    }
}

/// LRM §9.13 conversion functions, as pure `Value -> Value` transforms.
pub struct Conversions;

impl Conversions {
    /// `$itor(i)` — integer to real (exact for any `i64`, per IEEE-754
    /// double having 52 mantissa bits < `i64`'s 63 magnitude bits only for
    /// values beyond 2^53; LRM defines this as the natural numeric widening,
    /// so this mirrors the same `as f64` cast the rest of the crate uses).
    pub fn itor(v: &Value) -> Result<Value, SystemTaskError> {
        v.as_integer()
            .map(|i| Value::Real(i as f64))
            .or_else(|| v.as_natural().map(|n| Value::Real(n as f64)))
            .ok_or_else(|| Self::type_err("$itor", v))
    }

    /// `$rtoi(r)` — real to integer, truncating toward zero (§9.13).
    pub fn rtoi(v: &Value) -> Result<Value, SystemTaskError> {
        v.as_real().map(|r| Value::Int(r.trunc() as i64)).ok_or_else(|| Self::type_err("$rtoi", v))
    }

    /// `$realtobits(r)` — the real's IEEE-754 double bit pattern (§9.13).
    pub fn realtobits(v: &Value) -> Result<Value, SystemTaskError> {
        v.as_real()
            .map(|r| Value::Nat(r.to_bits()))
            .ok_or_else(|| Self::type_err("$realtobits", v))
    }

    /// `$bitstoreal(b)` — reinterprets a 64-bit pattern as an IEEE-754
    /// double (§9.13).
    pub fn bitstoreal(v: &Value) -> Result<Value, SystemTaskError> {
        let bits = v
            .as_natural()
            .or_else(|| v.as_integer().map(|i| i as u64))
            .ok_or_else(|| Self::type_err("$bitstoreal", v))?;
        Ok(Value::Real(f64::from_bits(bits)))
    }

    /// `$signed(b)` — reinterprets an integer's bit pattern as signed
    /// (§9.13); a no-op on a value that's already `Int`.
    pub fn signed(v: &Value) -> Result<Value, SystemTaskError> {
        v.as_natural()
            .map(|n| Value::Int(n as i64))
            .or_else(|| v.as_integer().map(Value::Int))
            .ok_or_else(|| Self::type_err("$signed", v))
    }

    /// `$unsigned(b)` — reinterprets an integer's bit pattern as unsigned
    /// (§9.13).
    pub fn unsigned(v: &Value) -> Result<Value, SystemTaskError> {
        v.as_integer()
            .map(|i| Value::Nat(i as u64))
            .or_else(|| v.as_natural().map(Value::Nat))
            .ok_or_else(|| Self::type_err("$unsigned", v))
    }

    fn type_err(task: &str, v: &Value) -> SystemTaskError {
        SystemTaskError::invalid_argument(task, format!("expected a numeric argument, got {}", v.type_name()))
    }
}

/// Errors raised evaluating LRM ch. 9 system tasks/functions (E-STD).
#[derive(Debug, Clone, thiserror::Error, miette::Diagnostic)]
pub enum SystemTaskError {
    /// VAMS E4007 — `$name` does not match any LRM chapter 9 (or Annex/
    /// Verilog-AMS-added) task/function this crate implements.
    ///
    /// **SPEC_DEVIATION (E-STD 6, span always carried):** [`SystemTasks::resolve`]
    /// takes a bare `&str`, not a spanned call site (no lowering pass calls it
    /// yet, per this module's own `//!` header) — `span` is always `None`
    /// today; the field exists so the eventual `TaskCall`/`SystemFunctionCall`
    /// lowering pass can populate it without a breaking type change.
    #[error("unknown system task or function `{name}`")]
    #[diagnostic(code(E4007), help("{help}"))]
    Unknown {
        name: String,
        suggestions: Vec<String>,
        /// Precomputed at construction ([`SystemTaskError::unknown`]) —
        /// miette's `help(...)` template interpolates named fields, not
        /// arbitrary method calls, so the rendered "did you mean" text is
        /// built once up front rather than at diagnostic-render time.
        help: String,
        #[label("no such system task or function")]
        span: Option<miette::SourceSpan>,
    },
    /// VAMS E4008 — a format string references a specifier LRM Table 9-1
    /// doesn't define, or supplies too few/mistyped arguments for it.
    /// `span` carries the same not-yet-wired caveat as [`Self::Unknown`].
    #[error("malformed format string: {reason}")]
    #[diagnostic(
        code(E4008),
        help("LRM Table 9-1 (§9.2) enumerates the legal `%`-specifiers and their argument types.")
    )]
    MalformedFormat {
        fmt: String,
        reason: String,
        #[label("format string used here")]
        span: Option<miette::SourceSpan>,
    },
    /// VAMS E4009 — an argument to a system task/function violates its
    /// LRM-defined domain (e.g. `$bound_step`'s non-positive bound, §9.5.2).
    /// `span` carries the same not-yet-wired caveat as [`Self::Unknown`].
    #[error("`{task}`: {reason}")]
    #[diagnostic(code(E4009), help("Check the argument against the task's LRM clause (ch. 9)."))]
    InvalidArgument {
        task: String,
        reason: String,
        #[label("offending argument")]
        span: Option<miette::SourceSpan>,
    },
}

impl SystemTaskError {
    /// Builds a `MalformedFormat` — a struct method (MD-13) so every
    /// construction site shares one place that fills the not-yet-wired
    /// `span` (see [`Self::Unknown`]'s doc comment).
    fn malformed(fmt: &str, reason: String) -> Self {
        Self::MalformedFormat { fmt: fmt.to_string(), reason, span: None }
    }

    /// Builds an `InvalidArgument` — see [`Self::malformed`].
    fn invalid_argument(task: &str, reason: String) -> Self {
        Self::InvalidArgument { task: task.to_string(), reason, span: None }
    }

    /// Builds the `Unknown` variant, precomputing its "did you mean" help
    /// text (E-STD 4) from the near-match list.
    fn unknown(name: &str, suggestions: Vec<String>) -> Self {
        let help = if suggestions.is_empty() {
            "LRM chapter 9 defines the general system tasks/functions; no close match was found \
             among the ones this crate implements."
                .to_string()
        } else {
            format!(
                "LRM chapter 9 defines the general system tasks/functions. Did you mean: {}?",
                suggestions.join(", ")
            )
        };
        Self::Unknown { name: name.to_string(), suggestions, help, span: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_every_table_entry() {
        for (name, kind) in SystemTasks::TABLE {
            assert_eq!(SystemTasks::resolve(name).unwrap(), *kind);
        }
    }

    #[test]
    fn unknown_name_fails_loud_with_near_match() {
        let err = SystemTasks::resolve("$displa").unwrap_err();
        match err {
            SystemTaskError::Unknown { name, suggestions, .. } => {
                assert_eq!(name, "$displa");
                assert!(suggestions.contains(&"$display".to_string()), "{suggestions:?}");
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn unrelated_name_gets_no_suggestions() {
        let err = SystemTasks::resolve("$zzz_totally_unrelated").unwrap_err();
        match err {
            SystemTaskError::Unknown { suggestions, .. } => assert!(suggestions.is_empty()),
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn levenshtein_basic_cases() {
        assert_eq!(SystemTasks::levenshtein("abc", "abc"), 0);
        assert_eq!(SystemTasks::levenshtein("abc", "abd"), 1);
        assert_eq!(SystemTasks::levenshtein("", "abc"), 3);
        assert_eq!(SystemTasks::levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn display_format_decimal_and_string() {
        let out = format_display("x=%d, s=%s", &[Value::Int(42), Value::Str("hi".into())], "top")
            .unwrap();
        assert_eq!(out, "x=42, s=hi");
    }

    #[test]
    fn display_format_radices() {
        assert_eq!(format_display("%h", &[Value::Int(255)], "top").unwrap(), "ff");
        assert_eq!(format_display("%o", &[Value::Int(8)], "top").unwrap(), "10");
        assert_eq!(format_display("%b", &[Value::Int(5)], "top").unwrap(), "101");
    }

    #[test]
    fn display_format_percent_literal_and_hierarchical_name() {
        assert_eq!(format_display("100%%", &[], "top").unwrap(), "100%");
        assert_eq!(format_display("in %m", &[], "top.sub").unwrap(), "in top.sub");
    }

    #[test]
    fn display_format_missing_argument_fails_loud() {
        let err = format_display("%d", &[], "top").unwrap_err();
        assert!(matches!(err, SystemTaskError::MalformedFormat { .. }));
    }

    #[test]
    fn display_format_unknown_specifier_fails_loud() {
        let err = format_display("%q", &[Value::Int(1)], "top").unwrap_err();
        assert!(matches!(err, SystemTaskError::MalformedFormat { .. }));
    }

    #[test]
    fn sim_environment_queries() {
        let env = SimEnvironment::new(1.5e-6, 300.0, 1e-9);
        assert_eq!(env.abstime(), Value::Real(1.5e-6));
        assert_eq!(env.temperature(), Value::Real(300.0));
        assert_eq!(env.bound_step(2e-9).unwrap(), 1e-9);
        assert_eq!(env.bound_step(0.5e-9).unwrap(), 0.5e-9);
    }

    #[test]
    fn bound_step_rejects_non_positive() {
        let env = SimEnvironment::new(0.0, 300.0, 1e-9);
        let err = env.bound_step(0.0).unwrap_err();
        assert!(matches!(err, SystemTaskError::InvalidArgument { .. }));
        let err = env.bound_step(-1.0).unwrap_err();
        assert!(matches!(err, SystemTaskError::InvalidArgument { .. }));
    }

    #[test]
    fn conversions_round_trip() {
        assert_eq!(Conversions::itor(&Value::Int(7)).unwrap(), Value::Real(7.0));
        assert_eq!(Conversions::rtoi(&Value::Real(7.9)).unwrap(), Value::Int(7));
        assert_eq!(Conversions::rtoi(&Value::Real(-7.9)).unwrap(), Value::Int(-7));
        let bits = Conversions::realtobits(&Value::Real(1.5)).unwrap();
        assert_eq!(Conversions::bitstoreal(&bits).unwrap(), Value::Real(1.5));
        assert_eq!(Conversions::signed(&Value::Nat(u64::MAX)).unwrap(), Value::Int(-1));
        assert_eq!(Conversions::unsigned(&Value::Int(-1)).unwrap(), Value::Nat(u64::MAX));
    }

    #[test]
    fn conversions_reject_non_numeric() {
        let err = Conversions::itor(&Value::Str("x".into())).unwrap_err();
        assert!(matches!(err, SystemTaskError::InvalidArgument { .. }));
    }
}
