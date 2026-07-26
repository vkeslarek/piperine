//! SI-suffixed unit newtypes (HOST-21). [`Freq`]/[`Time`] wrap a plain `f64`
//! in SI base units (Hz, seconds); `From<&str>` parses an optional SI prefix
//! (`f`/`p`/`n`/`u`(`µ`)/`m`/`k`/`M`/`G`/`T`) and an optional unit-name
//! suffix (`"Hz"`/`"s"`) — e.g. `Freq::from("10MHz") == Freq(1e7)`,
//! `Freq::from("10M") == Freq(1e7)` (the unit-name suffix is optional — the
//! newtype already names the unit), `Time::from("10ns") == Time(1e-8)`.
//! `From<f64>` treats a bare number as already being in the base unit — the
//! analysis args that accept `impl Into<Freq>` still take plain `f64`
//! literals unchanged (`f64: Into<Freq>` via the blanket `From<f64>`).
//!
//! `From<&str>` cannot return a `Result` (the trait is infallible), so
//! malformed input panics — fail loud, just via a panic rather than a typed
//! `Error` (this constructor is meant for literal/validated strings; a
//! runtime-computed value that might be invalid should stay a plain `f64`).

/// SI magnitude prefixes recognized by [`parse_scaled`], smallest to
/// largest.
const SI_PREFIXES: &[(char, f64)] = &[
    ('f', 1e-15),
    ('p', 1e-12),
    ('n', 1e-9),
    ('u', 1e-6),
    ('µ', 1e-6),
    ('m', 1e-3),
    ('k', 1e3),
    ('M', 1e6),
    ('G', 1e9),
    ('T', 1e12),
];

/// Parse `s` as a number with an optional trailing SI prefix and an
/// optional trailing `unit_suffix` (stripped first, if present — e.g.
/// `parse_scaled("10MHz", "Hz")` and `parse_scaled("10M", "Hz")` both yield
/// `1e7`). Fails loud (a message, not a panic) on anything that doesn't
/// parse as `<number><optional SI prefix>`.
fn parse_scaled(s: &str, unit_suffix: &str) -> Result<f64, String> {
    let trimmed = s.trim();
    let body = trimmed.strip_suffix(unit_suffix).unwrap_or(trimmed);
    if body.is_empty() {
        return Err(format!("`{trimmed}` has no numeric part"));
    }
    let last = body.chars().next_back().expect("body is non-empty");
    let (num_part, mult) = match SI_PREFIXES.iter().find(|(c, _)| *c == last) {
        Some((_, m)) => (&body[..body.len() - last.len_utf8()], *m),
        None => (body, 1.0),
    };
    num_part.trim().parse::<f64>().map(|n| n * mult).map_err(|_| {
        format!(
            "cannot parse `{trimmed}` as a number (expected `<number>` optionally followed by an \
             SI prefix (k/M/G/m/u/n/p/f) and/or the `{unit_suffix}` suffix)"
        )
    })
}

/// A frequency in Hz (HOST-21): `Freq::from(1e6)` is 1 MHz;
/// `Freq::from("10MHz")`/`Freq::from("10M")` are both 10 MHz.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Freq(pub f64);

impl From<f64> for Freq {
    fn from(v: f64) -> Self {
        Freq(v)
    }
}

impl From<&str> for Freq {
    fn from(s: &str) -> Self {
        Freq(parse_scaled(s, "Hz").unwrap_or_else(|e| panic!("invalid Freq: {e}")))
    }
}

impl From<Freq> for f64 {
    fn from(f: Freq) -> f64 {
        f.0
    }
}

/// A duration in seconds (HOST-21): `Time::from(1e-9)` is 1 ns;
/// `Time::from("10ns")`/`Time::from("10n")` are both 10 ns.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Time(pub f64);

impl From<f64> for Time {
    fn from(v: f64) -> Self {
        Time(v)
    }
}

impl From<&str> for Time {
    fn from(s: &str) -> Self {
        Time(parse_scaled(s, "s").unwrap_or_else(|e| panic!("invalid Time: {e}")))
    }
}

impl From<Time> for f64 {
    fn from(t: Time) -> f64 {
        t.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freq_parses_si_suffixed_strings() {
        assert_eq!(Freq::from("10MHz").0, 1e7);
        assert_eq!(Freq::from("10M").0, 1e7);
        assert_eq!(Freq::from("1kHz").0, 1e3);
        assert_eq!(Freq::from("10Hz").0, 10.0);
    }

    #[test]
    fn freq_accepts_bare_f64() {
        let f: Freq = 1e7.into();
        assert_eq!(f.0, 1e7);
    }

    #[test]
    #[should_panic(expected = "invalid Freq")]
    fn freq_garbage_string_fails_loud() {
        let _: Freq = "banana".into();
    }

    #[test]
    fn time_parses_si_suffixed_strings() {
        assert_eq!(Time::from("10ns").0, 1e-8);
        assert_eq!(Time::from("1ms").0, 1e-3);
    }
}
