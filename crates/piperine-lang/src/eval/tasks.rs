//! LRM chapter 9 — general system tasks and functions.
//!
//! **Scope of this file (T47, VAMS-15).** `crate::ast::Stmt::TaskCall`
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
//!
//! **T53 (VAMS-21) adds ch. 11's VPI-associated surface**: [`VpiTaskKind`]
//! and [`VpiTaskRegistry`] model the `vpi_register_systf`/
//! `vpi_register_analog_systf` mechanism (§11.2.1) by which a `$name`
//! becomes a valid, callable system task/function beyond [`SystemTasks`]'s
//! closed ch. 9 table — the language-level surface every ch. 11
//! VPI-associated task/function needs to parse and elaborate without an
//! unknown-identifier failure, per spec's Out of Scope table ("VPI's
//! language-level surface... routes through [Piperine's native plugin ABI]
//! instead", never a `dlopen`'d IEEE-VPI C shim). This crate stays free of
//! any `piperine-plugin` dependency (MD-15); `piperine_plugin::host`
//! (`PluginHost::register_system_task`/`call_system_task`) owns the actual
//! native-handler dispatch on the other side of that boundary.

use crate::value::Value;

/// One LRM ch. 9 system task/function this module implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// `$driver_count(net)` — number of drivers on `net` (§7.6/§9.22-9.23,
    /// LRMC-C7-05). Data source: `piperine_solver::digital::DriverTable::
    /// driver_count`.
    DriverCount,
    /// `$driver_state(net, i)` — driver `i`'s current logic value.
    DriverState,
    /// `$driver_strength(net, i)` — driver `i`'s current drive strength.
    DriverStrength,
    /// `$driver_delay(net, i)` — driver `i`'s propagation delay.
    DriverDelay,
    /// `$driver_next_state(net, i)` — driver `i`'s pending next value.
    DriverNextState,
    /// `$driver_next_strength(net, i)` — driver `i`'s pending next strength.
    DriverNextStrength,
    /// `$driver_type(net, i)` — driver `i`'s kind (ordinary vs connect
    /// module, §7.9's segregation classes).
    DriverType,
    /// `driver_update` — commits every driver's pending next-state onto
    /// its current state (the task counterpart of the seven `$driver_*`
    /// query functions above).
    DriverUpdate,
    /// `$fopen(path)` — §9.5, opens a file, returning a descriptor.
    Fopen,
    /// `$fclose(fd)` — §9.5.
    Fclose,
    /// `$fdisplay(fd, ...)` — §9.5, `$display`'s file-routed twin.
    Fdisplay,
    /// `$fwrite(fd, ...)` — §9.5, `$write`'s file-routed twin.
    Fwrite,
    /// `$fstrobe(fd, ...)` — §9.5.
    Fstrobe,
    /// `$fmonitor(fd, ...)` — §9.5.
    Fmonitor,
    /// `$fscanf(fd, fmt, ...)` — §9.5, reads formatted values from a file.
    Fscanf,
    /// `$sscanf(str, fmt, ...)` — §9.5, reads formatted values from a
    /// string (same specifier grammar as `$fscanf`, no descriptor).
    Sscanf,
    /// `$rewind(fd)` — §9.5, repositions to the start.
    Rewind,
    /// `$fseek(fd, offset, op)` — §9.5.
    Fseek,
    /// `$ftell(fd)` — §9.5.
    Ftell,
    /// `$ferror(fd)` — §9.5.
    Ferror,
    /// `$feof(fd)` — §9.5.
    Feof,
    /// `$fflush(fd)` — §9.5.
    Fflush,
    /// `$fgetc(fd)` — §9.5.
    Fgetc,
    /// `$random`/`$arandom` (§9.13.1) — a new 32-bit signed random number,
    /// updating the `inout` seed.
    Random,
    Arandom,
    /// `$rdist_uniform(seed, start, end[, type])` — §9.13.2.
    RdistUniform,
    /// `$rdist_normal(seed, mean, stddev[, type])` — §9.13.2.
    RdistNormal,
    /// `$rdist_exponential(seed, mean[, type])` — §9.13.2.
    RdistExponential,
    /// `$rdist_poisson(seed, mean[, type])` — §9.13.2.
    RdistPoisson,
    /// `$rdist_chi_square(seed, degree_of_freedom[, type])` — §9.13.2.
    RdistChiSquare,
    /// `$rdist_t(seed, degree_of_freedom[, type])` — §9.13.2.
    RdistT,
    /// `$rdist_erlang(seed, k_stage, mean[, type])` — §9.13.2.
    RdistErlang,
    /// `$time` — §9.10 (base IEEE 1364, unchanged by VAMS): scaled
    /// simulation time as a 64-bit unsigned integer.
    Time,
    /// `$realtime` — like `$time`, but a real number.
    Realtime,
    /// `$stime` — like `$time`, truncated to the low 32 bits.
    Stime,
    /// `$timeformat` — sets the `%t` format specifier's rendering.
    Timeformat,
    /// `$test$plusargs(str)` — §9.12: true if `+str` was passed on the
    /// command line.
    TestPlusargs,
    /// `$value$plusargs(str, var)` — §9.12: parses a `+name=value`
    /// command-line argument into `var` per `str`'s format spec.
    ValuePlusargs,
}

/// **T35/LRMC-C7-05 scope note (SPEC_DEVIATION):** the seven `$driver_*`
/// functions and `driver_update` above are registered here — the same
/// "pure semantics, name-level recognition" tier every other entry in
/// [`SystemTasks::TABLE`] lives at — but *not* wired further: reaching
/// "returns live data inside a connectmodule body" needs a live
/// `piperine_solver::digital::DriverTable` pointer threaded through
/// `piperine-codegen`'s analog JIT ABI (`emit::abi::SimCtx`, the same way
/// `io`/`SimOutput` already is), which this task's stated `Where`
/// (`eval/tasks.rs` only) doesn't reach. More load-bearing: that
/// `DriverTable` is *never populated* by anything today — no gate,
/// module, or connect-module instance calls `DriverTable::set_driver`
/// yet, because §7.9's actual driver-receiver segregation routing (which
/// module output feeds which table entry) is T36-T40's job, not T34/T35's.
/// Wiring live JIT reads against a table nothing ever writes to would be
/// architecturally present but semantically untestable — every call
/// would read `driver_count() == 0` forever, which is not "simulates
/// correctly" (T35's own Done-when), it's a silently-empty result dressed
/// up as live data. `piperine-codegen`'s existing generic
/// `CodegenError::unsupported(format!("syscall \`{name}\` in an analog
/// expression"))` fail-loud path (already exercised by every other
/// currently-unimplemented syscall name) is the CLAUDE.md-correct
/// behavior for these eight names until T36-T40 land the population side
/// — deliberately left alone rather than special-cased with a misleading
/// "coming soon" message that would drift out of sync. `DriverTable`
/// itself already exposes every per-index accessor these seven functions
/// need (`drivers`, `driver_count`, `resolved` — T34); this task adds
/// nothing further there.
///
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
        ("$driver_count", SystemTaskKind::DriverCount),
        ("$driver_state", SystemTaskKind::DriverState),
        ("$driver_strength", SystemTaskKind::DriverStrength),
        ("$driver_delay", SystemTaskKind::DriverDelay),
        ("$driver_next_state", SystemTaskKind::DriverNextState),
        ("$driver_next_strength", SystemTaskKind::DriverNextStrength),
        ("$driver_type", SystemTaskKind::DriverType),
        ("driver_update", SystemTaskKind::DriverUpdate),
        ("$fopen", SystemTaskKind::Fopen),
        ("$fclose", SystemTaskKind::Fclose),
        ("$fdisplay", SystemTaskKind::Fdisplay),
        ("$fwrite", SystemTaskKind::Fwrite),
        ("$fstrobe", SystemTaskKind::Fstrobe),
        ("$fmonitor", SystemTaskKind::Fmonitor),
        ("$fscanf", SystemTaskKind::Fscanf),
        ("$sscanf", SystemTaskKind::Sscanf),
        ("$rewind", SystemTaskKind::Rewind),
        ("$fseek", SystemTaskKind::Fseek),
        ("$ftell", SystemTaskKind::Ftell),
        ("$ferror", SystemTaskKind::Ferror),
        ("$feof", SystemTaskKind::Feof),
        ("$fflush", SystemTaskKind::Fflush),
        ("$fgetc", SystemTaskKind::Fgetc),
        ("$random", SystemTaskKind::Random),
        ("$arandom", SystemTaskKind::Arandom),
        ("$rdist_uniform", SystemTaskKind::RdistUniform),
        ("$rdist_normal", SystemTaskKind::RdistNormal),
        ("$rdist_exponential", SystemTaskKind::RdistExponential),
        ("$rdist_poisson", SystemTaskKind::RdistPoisson),
        ("$rdist_chi_square", SystemTaskKind::RdistChiSquare),
        ("$rdist_t", SystemTaskKind::RdistT),
        ("$rdist_erlang", SystemTaskKind::RdistErlang),
        ("$time", SystemTaskKind::Time),
        ("$realtime", SystemTaskKind::Realtime),
        ("$stime", SystemTaskKind::Stime),
        ("$timeformat", SystemTaskKind::Timeformat),
        ("$test$plusargs", SystemTaskKind::TestPlusargs),
        ("$value$plusargs", SystemTaskKind::ValuePlusargs),
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
        // LRMC-C7-08 (T44): a four-state `Value::Logic` renders per bit
        // (`0`/`1`/`x`/`z`), not coerced through a 2-state integer first
        // (which would silently turn every `x`/`z` bit into `0`) — the
        // `need_int()` 2-state fallback below is for plain
        // integer/natural arguments, still LRM-legal (§9.2's own
        // `%b`/`%h` accept any expression, not only a `reg`/`wire`).
        'b' | 'B' => match arg {
            Value::Logic(lv) => Ok(render_bits(lv)),
            _ => Ok(format!("{:b}", need_int()?)),
        },
        'h' | 'H' | 'x' | 'X' => match arg {
            Value::Logic(lv) => Ok(render_hex(lv)),
            _ => Ok(format!("{:x}", need_int()?)),
        },
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
        // `%v` (§17.5 net-strength display) needs drive-strength data
        // `Value` doesn't carry — a documented scope limit (this task's
        // own stated `Where` is `format_display`/`render_spec`, not a
        // new strength-carrying value type): a `Value::Logic` renders
        // its bits the same way `%b` does (at least honest about
        // unknown/high-Z bits, unlike the prior plain-integer coercion);
        // a plain integer/natural renders as before.
        'v' | 'V' => match arg {
            Value::Logic(lv) => Ok(render_bits(lv)),
            _ => Ok(format!("{}", need_int()?)),
        },
        other => Err(SystemTaskError::malformed(
            fmt,
            format!("`%{other}` is not a recognized LRM Table 9-1 specifier"),
        )),
    }
}

/// `%b`-style rendering (§9.2/Table 9-1, LRMC-C7-08): every bit, MSB
/// first, as `0`/`1`/`x`/`z` — never silently folding an unknown bit to
/// `0` the way a `{:b}`-over-`i64` coercion would.
fn render_bits(lv: &crate::value::LogicVec) -> String {
    (0..lv.width())
        .rev()
        .map(|i| match lv.bit(i) {
            crate::value::logic::LogicValue::Zero => '0',
            crate::value::logic::LogicValue::One => '1',
            crate::value::logic::LogicValue::X => 'x',
            crate::value::logic::LogicValue::Z => 'z',
        })
        .collect()
}

/// `%h`-style rendering (§9.2/Table 9-1, LRMC-C7-08): one hex digit per
/// 4-bit group, MSB group first. A group entirely `z` renders `z`; a
/// group that is any other mix of unknown (`x`/`z`) and known bits
/// renders `x` (the LRM's own "any bit of the group is unknown" rule —
/// only an all-`z` group gets its own distinct symbol); a fully-known
/// group renders its hex digit.
fn render_hex(lv: &crate::value::LogicVec) -> String {
    use crate::value::logic::LogicValue::*;
    let width = lv.width();
    let num_groups = width.div_ceil(4);
    (0..num_groups)
        .rev()
        .map(|g| {
            let bits: Vec<crate::value::logic::LogicValue> =
                (0..4).filter_map(|b| { let i = g * 4 + b; (i < width).then(|| lv.bit(i)) }).collect();
            if bits.iter().all(|&b| b == Z) {
                'z'
            } else if bits.iter().any(|&b| b == X || b == Z) {
                'x'
            } else {
                let mut nibble = 0u8;
                for (b, &v) in bits.iter().enumerate() {
                    if v == One {
                        nibble |= 1 << b;
                    }
                }
                std::char::from_digit(nibble as u32, 16).unwrap_or('?')
            }
        })
        .collect()
}

/// `$fscanf`/`$sscanf` (§9.5, LRMC-C7-09, T46) — the reverse of
/// [`format_display`]: reads typed [`Value`]s out of `input` per `fmt`'s
/// specifiers (Table 9-1's `%d`/`%h`/`%x`/`%o`/`%b`/`%f`/`%e`/`%g`/`%s`/
/// `%c`), classic-`scanf` conventions: a run of whitespace in `fmt`
/// matches any run of whitespace in `input` (including none); a literal
/// character in `fmt` must match the next `input` character exactly; a
/// numeric/string specifier skips leading input whitespace, then
/// consumes up to the next whitespace run (`%c` never skips, and
/// consumes exactly one character). Fails loud (never a partial silent
/// result) the moment a literal mismatches, a token fails to parse for
/// its specifier's radix, or `input` runs out before `fmt` does.
pub fn parse_scanf(fmt: &str, input: &str) -> Result<Vec<Value>, SystemTaskError> {
    let mut out = Vec::new();
    let mut fmt_chars = fmt.chars().peekable();
    let mut input_chars = input.chars().peekable();

    while let Some(fc) = fmt_chars.next() {
        if fc.is_whitespace() {
            while fmt_chars.peek().is_some_and(|c| c.is_whitespace()) {
                fmt_chars.next();
            }
            while input_chars.peek().is_some_and(|c| c.is_whitespace()) {
                input_chars.next();
            }
            continue;
        }
        if fc != '%' {
            match input_chars.next() {
                Some(ic) if ic == fc => continue,
                other => {
                    return Err(SystemTaskError::malformed(
                        fmt,
                        format!("expected literal `{fc}`, got {:?}", other),
                    ));
                }
            }
        }
        let spec = fmt_chars
            .next()
            .ok_or_else(|| SystemTaskError::malformed(fmt, "trailing `%` with no specifier".to_string()))?;
        if spec == '%' {
            match input_chars.next() {
                Some('%') => continue,
                other => {
                    return Err(SystemTaskError::malformed(fmt, format!("expected literal `%`, got {other:?}")));
                }
            }
        }
        if spec == 'c' || spec == 'C' {
            let c = input_chars
                .next()
                .ok_or_else(|| SystemTaskError::malformed(fmt, "`%c` ran out of input".to_string()))?;
            out.push(Value::Int(c as i64));
            continue;
        }
        while input_chars.peek().is_some_and(|c| c.is_whitespace()) {
            input_chars.next();
        }
        // Numeric specifiers stop at the first character that can't
        // belong to their radix (classic `scanf` behavior: `%d` in
        // `"1,"` reads `1`, not `1,`) — an optional leading `-`/`+`, then
        // digits valid for the radix. `%s` is the LRM's own exception:
        // it consumes the whole whitespace-delimited token regardless of
        // content.
        let is_token_char = |c: char, first: bool| -> bool {
            match spec {
                's' | 'S' => !c.is_whitespace(),
                'd' | 'D' => c.is_ascii_digit() || (first && (c == '-' || c == '+')),
                'h' | 'H' | 'x' | 'X' => c.is_ascii_hexdigit(),
                'o' | 'O' => c.is_digit(8),
                'b' | 'B' => c.is_digit(2),
                'f' | 'F' | 'e' | 'E' | 'g' | 'G' => {
                    c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')
                }
                _ => !c.is_whitespace(),
            }
        };
        let mut token = String::new();
        while let Some(&c) = input_chars.peek() {
            if !is_token_char(c, token.is_empty()) {
                break;
            }
            token.push(c);
            input_chars.next();
        }
        if token.is_empty() {
            return Err(SystemTaskError::malformed(fmt, format!("`%{spec}` found no token (input exhausted)")));
        }
        let parse_err = |radix_name: &str| {
            SystemTaskError::malformed(fmt, format!("`%{spec}` token {token:?} is not valid {radix_name}"))
        };
        out.push(match spec {
            'd' | 'D' => Value::Int(token.parse::<i64>().map_err(|_| parse_err("decimal"))?),
            'h' | 'H' | 'x' | 'X' => Value::Int(i64::from_str_radix(&token, 16).map_err(|_| parse_err("hex"))?),
            'o' | 'O' => Value::Int(i64::from_str_radix(&token, 8).map_err(|_| parse_err("octal"))?),
            'b' | 'B' => Value::Int(i64::from_str_radix(&token, 2).map_err(|_| parse_err("binary"))?),
            'f' | 'F' | 'e' | 'E' | 'g' | 'G' => Value::Real(token.parse::<f64>().map_err(|_| parse_err("real"))?),
            's' | 'S' => Value::Str(token),
            other => {
                return Err(SystemTaskError::malformed(
                    fmt,
                    format!("`%{other}` is not a recognized LRM Table 9-1 specifier"),
                ));
            }
        });
    }
    Ok(out)
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

    /// `$realtime` (§9.10, base IEEE 1364, unchanged by VAMS): scaled
    /// simulation time as a real number.
    ///
    /// **SPEC_DEVIATION, disclosed:** base 1364's `$time`/`$realtime`/
    /// `$stime` scale by the active `` `timescale``/time-precision unit
    /// (an integer count of, e.g., nanoseconds) — Piperine has no
    /// `` `timescale``/time-precision tracking anywhere in the pipeline
    /// (`$abstime` is the *only* time primitive that exists, already a
    /// raw real-seconds value, T35/CLAUDE.md's own "declared surface"
    /// section). These three treat one time unit as one second (the
    /// simplest faithful assumption absent any declared scale) rather
    /// than silently mis-scaling against a unit this crate never tracks.
    pub fn realtime(&self) -> Value {
        Value::Real(self.abstime)
    }

    /// `$time` (§9.10): `$realtime` rounded to a 64-bit unsigned integer
    /// count of time units (see [`Self::realtime`]'s own scope note).
    pub fn time(&self) -> Value {
        Value::Nat(self.abstime.round().max(0.0) as u64)
    }

    /// `$stime` (§9.10): `$time`, truncated to the low 32 bits.
    pub fn stime(&self) -> Value {
        let full = self.abstime.round().max(0.0) as u64;
        Value::Nat(full & 0xFFFF_FFFF)
    }
}

/// `$timeformat(units_number, precision, suffix, min_width)` (§9.10,
/// base IEEE 1364) — how a `%t` format specifier renders a time value.
/// A pure value this crate's caller sets and reads; wiring it into
/// `format_display`'s own `%t` dispatch (currently a bare `{}`-formatted
/// real) is future work, matching the "pure semantics, not yet
/// consumer-wired" tier most of this module's newer entries live at.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeFormat {
    /// Power-of-ten scale relative to seconds (e.g. `-9` for `ns`).
    pub units_exponent: i32,
    /// Digits after the decimal point.
    pub precision: usize,
    /// Trailing unit label (e.g. `"ns"`).
    pub suffix: String,
    /// Minimum total field width, space-padded on the left.
    pub min_width: usize,
}

impl Default for TimeFormat {
    fn default() -> Self {
        Self { units_exponent: 0, precision: 0, suffix: String::new(), min_width: 0 }
    }
}

/// Renders `abstime_seconds` per `fmt` — the `%t` specifier's real
/// behavior once `$timeformat` has been called.
pub fn format_time(abstime_seconds: f64, fmt: &TimeFormat) -> String {
    let scaled = abstime_seconds / 10f64.powi(fmt.units_exponent);
    let body = format!("{scaled:.*}{}", fmt.precision, fmt.suffix);
    if body.len() < fmt.min_width {
        format!("{:>width$}", body, width = fmt.min_width)
    } else {
        body
    }
}

/// `$test$plusargs`/`$value$plusargs` (§9.12, base IEEE 1364) — matches
/// against the host's own `+plusarg` command-line tokens (SPEC Part VI:
/// the same shape `argv` conventionally takes, `+name` or
/// `+name=value`), reused verbatim by `SessionBuilder::plusargs`
/// (`piperine-api`, T48).
#[derive(Debug, Clone, Default)]
pub struct PlusArgs {
    args: Vec<String>,
}

impl PlusArgs {
    /// `args` are the raw `+...` tokens (the leading `+` included, the
    /// LRM's own on-the-command-line shape) — non-`+`-prefixed tokens
    /// are ignored, so a caller can hand this the whole `argv` slice
    /// unfiltered.
    pub fn new(args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { args: args.into_iter().map(Into::into).filter(|a: &String| a.starts_with('+')).collect() }
    }

    /// `$test$plusargs(name)` — true if some `+name...` token was passed
    /// (a prefix match, per the LRM's own "matches the beginning of the
    /// plusarg" rule).
    pub fn test(&self, name: &str) -> bool {
        let needle = format!("+{name}");
        self.args.iter().any(|a| a.starts_with(&needle))
    }

    /// `$value$plusargs("name=%d", var)`-style lookup: `spec` is
    /// `"name=%d"`/`"name=%s"`/... (a name, `=`, then exactly one
    /// `format_display`-style specifier); returns the parsed [`Value`]
    /// from the matching `+name=value` token, or `None` if no such
    /// token was passed. A malformed `spec` (no `=`, no specifier) or a
    /// value that doesn't parse for the requested specifier fails loud.
    pub fn value(&self, spec: &str) -> Result<Option<Value>, SystemTaskError> {
        let Some((name, rest)) = spec.split_once('=') else {
            return Err(SystemTaskError::invalid_argument(
                "$value$plusargs",
                format!("spec {spec:?} has no `=name=%specifier` shape"),
            ));
        };
        let prefix = format!("+{name}=");
        let Some(token) = self.args.iter().find(|a| a.starts_with(&prefix)) else {
            return Ok(None);
        };
        let raw = &token[prefix.len()..];
        let values = parse_scanf(rest, raw)
            .map_err(|e| SystemTaskError::invalid_argument("$value$plusargs", format!("{e}")))?;
        Ok(values.into_iter().next())
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

/// LRM §11.2.1 — the registration form a native handler was submitted
/// under. `vpi_register_systf()` covers `SysTask`/`SysFunc`/`SysRealFunc`
/// (the LRM's `systf_type` field: `vpiSysTask`, `vpiSysFunc`,
/// `vpiSysRealFunc`); `vpi_register_analog_systf()` additionally allows a
/// task/function usable inside `analog` blocks (`AnalogSysTask`/
/// `AnalogSysFunc`, the Verilog-AMS-specific extension of §11.2.1's final
/// paragraph, paired with the derivative-aware `vpi_handle_multi`/
/// `vpi_put_value` described in §11.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpiTaskKind {
    /// Registered via `vpi_register_systf`, `systf_type = vpiSysTask`.
    SysTask,
    /// Registered via `vpi_register_systf`, `systf_type = vpiSysFunc`.
    SysFunc,
    /// Registered via `vpi_register_systf`, `systf_type = vpiSysRealFunc`.
    SysRealFunc,
    /// Registered via `vpi_register_analog_systf`, usable from an `analog`
    /// block, no return value.
    AnalogSysTask,
    /// Registered via `vpi_register_analog_systf`, usable from an `analog`
    /// block, returns a value (and may carry a derivative, §11.3.2).
    AnalogSysFunc,
}

/// The table of user-defined system tasks/functions registered through
/// `vpi_register_systf`/`vpi_register_analog_systf` (LRM §11.2.1) — the
/// mechanism by which a `$name` becomes callable from `.vams` source beyond
/// [`SystemTasks`]'s closed ch. 9 table, without Piperine ever hosting a
/// literal VPI C shim (spec's Out of Scope table: "VPI's language-level
/// surface... routes through [the native plugin ABI] instead"). Pure
/// name/kind bookkeeping — `piperine-plugin`'s host owns the actual native
/// handler each entry dispatches to; this crate has no dependency on
/// `piperine-plugin` (MD-15), so the handler itself lives on that side of
/// the boundary.
#[derive(Debug, Default)]
pub struct VpiTaskRegistry {
    entries: std::collections::HashMap<String, VpiTaskKind>,
}

impl VpiTaskRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `name` under `kind`. Fails loud (never silently
    /// overwrites, C6):
    /// - `name` collides with a ch. 9 builtin ([`SystemTasks::TABLE`]) — a
    ///   VPI application cannot shadow a language builtin, per §11.2.1's
    ///   "user-defined system task or function" framing (E4010).
    /// - `name` is already registered — two `vpi_register_systf` calls for
    ///   one name is a real conflict the LRM leaves undefined; Piperine
    ///   resolves it as a hard error, never last-writer-wins (E4011).
    pub fn register(&mut self, name: &str, kind: VpiTaskKind) -> Result<(), SystemTaskError> {
        if SystemTasks::TABLE.iter().any(|(n, _)| *n == name) {
            return Err(SystemTaskError::vpi_collides_with_builtin(name));
        }
        if self.entries.contains_key(name) {
            return Err(SystemTaskError::vpi_duplicate_registration(name));
        }
        self.entries.insert(name.to_string(), kind);
        Ok(())
    }

    /// Resolves `name` to its registered [`VpiTaskKind`]. An unregistered
    /// name fails loud (E4012) with near-match suggestions drawn from the
    /// *registered* names (not ch. 9's table — a VPI-name typo should
    /// suggest other VPI names, not an unrelated builtin).
    pub fn resolve(&self, name: &str) -> Result<VpiTaskKind, SystemTaskError> {
        self.entries.get(name).copied().ok_or_else(|| {
            let table: Vec<&str> = self.entries.keys().map(String::as_str).collect();
            SystemTaskError::unknown_vpi(name, Self::near_matches(name, &table))
        })
    }

    /// Whether `name` has a live registration.
    pub fn is_registered(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    fn near_matches(name: &str, table: &[&str]) -> Vec<String> {
        const MAX_DISTANCE: usize = 3;
        const MAX_SUGGESTIONS: usize = 3;
        let mut scored: Vec<(usize, &str)> = table
            .iter()
            .map(|n| (SystemTasks::levenshtein(name, n), *n))
            .filter(|(d, _)| *d <= MAX_DISTANCE)
            .collect();
        scored.sort_by_key(|(d, _)| *d);
        scored.into_iter().take(MAX_SUGGESTIONS).map(|(_, n)| n.to_string()).collect()
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
    /// VAMS E4010 — a `vpi_register_systf`/`vpi_register_analog_systf`
    /// registration (§11.2.1) named a ch. 9 builtin. `span` carries the
    /// same not-yet-wired caveat as [`Self::Unknown`].
    #[error("`{name}` cannot be registered via VPI: it already names a ch. 9 builtin system task/function")]
    #[diagnostic(
        code(E4010),
        help("LRM §11.2.1 registers *user-defined* system tasks/functions; `{name}` is a language builtin (ch. 9) and cannot be shadowed. Choose a different name.")
    )]
    VpiCollidesWithBuiltin {
        name: String,
        #[label("VPI registration attempted here")]
        span: Option<miette::SourceSpan>,
    },
    /// VAMS E4011 — two `vpi_register_systf`/`vpi_register_analog_systf`
    /// calls registered the same name. `span` carries the same
    /// not-yet-wired caveat as [`Self::Unknown`].
    #[error("`{name}` is already registered via VPI")]
    #[diagnostic(
        code(E4011),
        help("LRM §11.2.1 leaves a duplicate `vpi_register_systf`/`vpi_register_analog_systf` name undefined; Piperine treats it as a conflict rather than last-writer-wins. Register `{name}` once, or under a distinct name.")
    )]
    VpiDuplicateRegistration {
        name: String,
        #[label("duplicate VPI registration")]
        span: Option<miette::SourceSpan>,
    },
    /// VAMS E4012 — `name` was called but no `vpi_register_systf`/
    /// `vpi_register_analog_systf` registration (§11.2.1) exists for it.
    /// `span` carries the same not-yet-wired caveat as [`Self::Unknown`].
    #[error("no VPI registration for system task or function `{name}`")]
    #[diagnostic(code(E4012), help("{help}"))]
    UnknownVpiTask {
        name: String,
        suggestions: Vec<String>,
        help: String,
        #[label("no such VPI-registered task or function")]
        span: Option<miette::SourceSpan>,
    },
}

impl SystemTaskError {
    /// Builds a `MalformedFormat` — a struct method (MD-13) so every
    /// construction site shares one place that fills the not-yet-wired
    /// `span` (see [`Self::Unknown`]'s doc comment).
    pub(crate) fn malformed(fmt: &str, reason: String) -> Self {
        Self::MalformedFormat { fmt: fmt.to_string(), reason, span: None }
    }

    /// Builds an `InvalidArgument` — see [`Self::malformed`].
    pub(crate) fn invalid_argument(task: &str, reason: String) -> Self {
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

    /// Builds a `VpiCollidesWithBuiltin` (E4010).
    fn vpi_collides_with_builtin(name: &str) -> Self {
        Self::VpiCollidesWithBuiltin { name: name.to_string(), span: None }
    }

    /// Builds a `VpiDuplicateRegistration` (E4011).
    fn vpi_duplicate_registration(name: &str) -> Self {
        Self::VpiDuplicateRegistration { name: name.to_string(), span: None }
    }

    /// Builds an `UnknownVpiTask` (E4012), precomputing its "did you mean"
    /// help text (E-STD 4) from the near-match list, same shape as
    /// [`Self::unknown`].
    fn unknown_vpi(name: &str, suggestions: Vec<String>) -> Self {
        let help = if suggestions.is_empty() {
            "LRM §11.2.1 defines VPI-registered system tasks/functions; no close match was found \
             among the names currently registered."
                .to_string()
        } else {
            format!(
                "LRM §11.2.1 defines VPI-registered system tasks/functions. Did you mean: {}?",
                suggestions.join(", ")
            )
        };
        Self::UnknownVpiTask { name: name.to_string(), suggestions, help, span: None }
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

    /// T35/LRMC-C7-05: the seven `$driver_*` functions plus `driver_update`
    /// each resolve to their own distinct kind — a real regression guard
    /// against two of the eight silently colliding on one enum variant.
    #[test]
    fn driver_access_names_resolve_to_distinct_kinds() {
        use SystemTaskKind::*;
        let cases = [
            ("$driver_count", DriverCount),
            ("$driver_state", DriverState),
            ("$driver_strength", DriverStrength),
            ("$driver_delay", DriverDelay),
            ("$driver_next_state", DriverNextState),
            ("$driver_next_strength", DriverNextStrength),
            ("$driver_type", DriverType),
            ("driver_update", DriverUpdate),
        ];
        let mut seen = std::collections::HashSet::new();
        for (name, expected) in cases {
            assert_eq!(SystemTasks::resolve(name).unwrap(), expected);
            assert!(seen.insert(expected), "{name} collides with another driver-access kind");
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

    /// LRMC-C7-08 (T44): `%b`/`%h` render `x`/`z` bits from a four-state
    /// `Value::Logic`, never silently folding them to `0`.
    #[test]
    fn display_format_b_and_h_render_unknown_and_highz_bits() {
        use crate::value::logic::LogicValue as Lv;
        use crate::value::LogicVec;

        let mut bits = LogicVec::zero(4);
        bits.set_bit(3, Lv::One);
        bits.set_bit(2, Lv::Zero);
        bits.set_bit(1, Lv::X);
        bits.set_bit(0, Lv::Z);
        let arg = Value::Logic(bits);
        assert_eq!(format_display("%b", std::slice::from_ref(&arg), "top").unwrap(), "10xz");
        // The single 4-bit group is a mix of known/unknown -> 'x'.
        assert_eq!(format_display("%h", std::slice::from_ref(&arg), "top").unwrap(), "x");

        let all_z = Value::Logic({
            let mut v = LogicVec::zero(4);
            for i in 0..4 {
                v.set_bit(i, Lv::Z);
            }
            v
        });
        assert_eq!(format_display("%b", &[all_z.clone()], "top").unwrap(), "zzzz");
        assert_eq!(format_display("%h", &[all_z], "top").unwrap(), "z");

        // A fully-known 8-bit value still renders its exact hex digits.
        let known = Value::Logic(LogicVec::from_int(0xAB, 8));
        assert_eq!(format_display("%h", &[known], "top").unwrap(), "ab");
    }

    /// Escape sequences (§2.7 Table 2-2, LRMC-C7-08): `\n`/`\t`/`\\`/`\"`/
    /// octal `\ddd` are lexer-level (proven here through the real
    /// `Lexer`, not `format_display` — they apply to every string
    /// literal, not only a `$display` format argument); `\%` — the one
    /// this task's own audit found missing — alongside them.
    #[test]
    fn string_literal_escape_sequences_decode_per_table_2_2() {
        let src = r#""a\nb\tc\\d\"e\%f\101""#;
        let toks = crate::Lexer::new(src).tokenize().expect("lexes");
        let crate::lexer::Tok::Str(s) = &toks[0].tok else { panic!("expected a string token") };
        assert_eq!(s, "a\nb\tc\\d\"e%fA", "\\101 (octal) is 'A' (0x41)");
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

    /// LRMC-C7-09 (T48): `$time`/`$realtime`/`$stime` differ correctly —
    /// realtime is the raw real value, time/stime round it to an
    /// integer, and stime additionally truncates to 32 bits.
    #[test]
    fn time_realtime_and_stime_differ_per_the_lrm() {
        let env = SimEnvironment::new(12.6, 300.0, 1e-9);
        assert_eq!(env.realtime(), Value::Real(12.6));
        assert_eq!(env.time(), Value::Nat(13), "rounds to nearest integer");

        let big = SimEnvironment::new((1u64 << 33) as f64 + 5.0, 300.0, 1e-9);
        let Value::Nat(stime) = big.stime() else { panic!("expected Nat") };
        assert_eq!(stime, 5, "stime truncates to the low 32 bits");
        let Value::Nat(time) = big.time() else { panic!("expected Nat") };
        assert_eq!(time, (1u64 << 33) + 5, "time keeps the full width");
    }

    #[test]
    fn timeformat_affects_percent_t_rendering() {
        let default_fmt = TimeFormat::default();
        assert_eq!(format_time(1.5, &default_fmt), "2");

        let ns = TimeFormat { units_exponent: -9, precision: 3, suffix: "ns".to_string(), min_width: 0 };
        assert_eq!(format_time(1.5e-6, &ns), "1500.000ns");

        let padded = TimeFormat { units_exponent: 0, precision: 0, suffix: String::new(), min_width: 8 };
        assert_eq!(format_time(3.0, &padded), "       3");
    }

    #[test]
    fn plusargs_test_and_value_lookups() {
        let args = PlusArgs::new(["+seed=42", "+verbose", "notaplusarg", "+name=hello"]);
        assert!(args.test("seed"));
        assert!(args.test("verbose"));
        assert!(!args.test("missing"));

        assert_eq!(args.value("seed=%d").unwrap(), Some(Value::Int(42)));
        assert_eq!(args.value("name=%s").unwrap(), Some(Value::Str("hello".into())));
        assert_eq!(args.value("absent=%d").unwrap(), None, "no matching token -> None, not an error");
    }

    #[test]
    fn plusargs_value_fails_loud_on_a_malformed_spec() {
        let args = PlusArgs::new(["+seed=42"]);
        let err = args.value("no_equals_sign").unwrap_err();
        assert!(matches!(err, SystemTaskError::InvalidArgument { .. }));
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

    #[test]
    fn vpi_registers_and_resolves() {
        let mut reg = VpiTaskRegistry::new();
        reg.register("$my_task", VpiTaskKind::SysTask).unwrap();
        reg.register("$my_real_func", VpiTaskKind::SysRealFunc).unwrap();
        assert_eq!(reg.resolve("$my_task").unwrap(), VpiTaskKind::SysTask);
        assert_eq!(reg.resolve("$my_real_func").unwrap(), VpiTaskKind::SysRealFunc);
        assert!(reg.is_registered("$my_task"));
        assert!(!reg.is_registered("$not_registered"));
    }

    #[test]
    fn vpi_analog_registration_kinds() {
        let mut reg = VpiTaskRegistry::new();
        reg.register("$my_analog_task", VpiTaskKind::AnalogSysTask).unwrap();
        reg.register("$my_analog_func", VpiTaskKind::AnalogSysFunc).unwrap();
        assert_eq!(reg.resolve("$my_analog_task").unwrap(), VpiTaskKind::AnalogSysTask);
        assert_eq!(reg.resolve("$my_analog_func").unwrap(), VpiTaskKind::AnalogSysFunc);
    }

    #[test]
    fn vpi_cannot_shadow_a_ch9_builtin() {
        let mut reg = VpiTaskRegistry::new();
        let err = reg.register("$display", VpiTaskKind::SysTask).unwrap_err();
        assert!(matches!(err, SystemTaskError::VpiCollidesWithBuiltin { .. }));
        assert!(!reg.is_registered("$display"));
    }

    #[test]
    fn vpi_duplicate_registration_fails_loud() {
        let mut reg = VpiTaskRegistry::new();
        reg.register("$my_task", VpiTaskKind::SysTask).unwrap();
        let err = reg.register("$my_task", VpiTaskKind::SysFunc).unwrap_err();
        assert!(matches!(err, SystemTaskError::VpiDuplicateRegistration { .. }));
        // The original registration is untouched by the failed re-registration.
        assert_eq!(reg.resolve("$my_task").unwrap(), VpiTaskKind::SysTask);
    }

    #[test]
    fn vpi_unregistered_name_fails_loud_with_near_match() {
        let mut reg = VpiTaskRegistry::new();
        reg.register("$my_task", VpiTaskKind::SysTask).unwrap();
        let err = reg.resolve("$my_tas").unwrap_err();
        match err {
            SystemTaskError::UnknownVpiTask { name, suggestions, .. } => {
                assert_eq!(name, "$my_tas");
                assert!(suggestions.contains(&"$my_task".to_string()), "{suggestions:?}");
            }
            other => panic!("expected UnknownVpiTask, got {other:?}"),
        }
    }

    #[test]
    fn vpi_unregistered_name_no_registrations_yet() {
        let reg = VpiTaskRegistry::new();
        let err = reg.resolve("$anything").unwrap_err();
        match err {
            SystemTaskError::UnknownVpiTask { suggestions, .. } => assert!(suggestions.is_empty()),
            other => panic!("expected UnknownVpiTask, got {other:?}"),
        }
    }
}
