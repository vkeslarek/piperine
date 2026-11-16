//! `Backend`: which backend runs a design (CLI-04 AC 7 and 8). The command line's flag, then
//! the variable `PIPERINE_BACKEND`, then the run's `backend` key, then `rust`. The choice is a
//! pure function of the three texts, so no test sets a variable.

use crate::failure::Failure;

/// A backend a design runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// The reference interpreter (`piperine-interp`): the M1 analog subset.
    Interp,
    /// The generated Rust compiled by `rustc` (`piperine-transpile`): everything.
    Rust,
}

impl Backend {
    /// Every backend, in the order their names are listed to a user.
    pub const ALL: [Backend; 2] = [Backend::Interp, Backend::Rust];

    /// The name a user writes.
    pub fn name(&self) -> &'static str {
        match self {
            Backend::Interp => "interp",
            Backend::Rust => "rust",
        }
    }

    /// The backend named `text`; any other text is a `Failure::Host` naming the two values.
    pub fn parse(text: &str) -> Result<Backend, Failure> {
        Backend::ALL.into_iter().find(|b| b.name() == text).ok_or_else(|| Failure::Host {
            code: "P0010",
            message: format!("unknown backend `{text}`: the backends are `interp` and `rust`"),
        })
    }

    /// The first of `flag`, `variable` and `key` that is given, parsed; `rust` when none is.
    pub fn choose(flag: Option<&str>, variable: Option<&str>, key: Option<&str>) -> Result<Backend, Failure> {
        match flag.or(variable).or(key) {
            Some(text) => Backend::parse(text),
            None => Ok(Backend::Rust),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_then_the_variable_then_the_key_then_rust() {
        // CLI-04 AC 7, over all eight presence patterns of the three places. Each place names a
        // backend no earlier place names, so the winner shows which place was read.
        let (flag, variable, key) = ("interp", "rust", "interp");
        let cases: [(Option<&str>, Option<&str>, Option<&str>, Backend); 8] = [
            (None, None, None, Backend::Rust),
            (None, None, Some(key), Backend::Interp),
            (None, Some(variable), None, Backend::Rust),
            (None, Some("interp"), Some("rust"), Backend::Interp),
            (Some(flag), None, None, Backend::Interp),
            (Some(flag), None, Some("rust"), Backend::Interp),
            (Some(flag), Some(variable), None, Backend::Interp),
            (Some("rust"), Some("interp"), Some("interp"), Backend::Rust),
        ];
        for (f, v, k, want) in cases {
            assert_eq!(Backend::choose(f, v, k).unwrap_or_else(|e| panic!("{f:?} {v:?} {k:?}: {e}")), want, "flag {f:?}, variable {v:?}, key {k:?}");
        }
        // CLI-04 AC 8: another value, wherever it comes from, names the two valid values; an
        // invalid place behind a valid one is never read
        for (f, v, k) in [(Some("x"), None, None), (None, Some("x"), None), (None, None, Some("x"))] {
            let error = Backend::choose(f, v, k).expect_err("`x` is no backend").to_string();
            assert!(error.contains("`x`") && error.contains("`interp`") && error.contains("`rust`"), "{error}");
        }
        assert_eq!(Backend::choose(Some("rust"), Some("x"), Some("x")).ok(), Some(Backend::Rust));
    }
}
