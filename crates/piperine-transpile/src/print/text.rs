//! `Text`: the source writer every printer writes through. It owns the layout rules of RS-02 AC 3:
//! 4 spaces per level, one statement per line, reals in the shortest decimal that parses back to
//! the same bits, and identifiers that stay MSL names, raw where Rust reserves the word.
//!
//! Two name spaces never meet. A name the printer makes (`cx_`, `at_iterate_`, `vd_limit_`,
//! `b0_flow_`, `arg_4_`, `temperature_`) ends in exactly one `_` after a character that is not
//! `_`: [`Text::generated`]. An MSL name prints as written ([`Text::ident`]) unless it ends in
//! `_` or is a name the generated code itself refers to (an import, `Ok`, `f64`, the four
//! keywords Rust refuses raw); then it prints with one more `_`. That map is one to one (an
//! escaped name ends in `_` again, so it is escaped again), and no MSL name prints as a
//! generated one: an escaped name either ends in `__` or is a reserved word plus `_`, and the
//! printer makes neither.

use piperine_ir::Origin;

/// Rust's keywords, strict and reserved, in edition 2024: an MSL name among them prints raw.
const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn", "else", "enum",
    "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "macro", "match", "mod",
    "move", "mut", "override", "priv", "pub", "ref", "return", "static", "struct", "trait", "true", "try", "type",
    "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// The keywords Rust does not accept as raw identifiers: an MSL name among them prints with a
/// trailing `_`.
const UNRAWABLE: &[&str] = &["crate", "self", "Self", "super"];

/// The names generated code refers to besides MSL names, in the scopes MSL names print in: the
/// crates, the runtime module and the export, every name the printer imports, the type
/// parameter `R`, and the prelude's names a generated line spells. An MSL name among them
/// prints with a trailing `_`, so it never shadows one of them.
const RESERVED: &[&str] = &[
    // crates, the runtime module, the export and the two thunks
    "std", "core", "alloc", "piperine_abi", "rt", "piperine_abi_1", "library", "module",
    // the prelude and the primitive types generated code spells
    "Ok", "Err", "Some", "None", "Result", "Option", "Fn", "Box", "Vec", "String", "f64", "u8", "u32", "bool",
    "str", "R",
    // rt
    "Real", "Dual",
    // piperine_abi::device
    "Device", "Eval", "Fault", "Flow", "React", "Values",
    // piperine_abi::layout
    "BIT", "BYTE", "F64", "HIGH", "LOW", "NONE", "REAL",
    // piperine_abi::raw
    "Cell", "DEFAULT", "Field", "GROUND", "Layout", "Library", "List", "Module", "Partial", "Shape", "Statement",
    "Str", "ambient", "cell", "field_flag", "fn_flag", "global", "input", "kind", "leaf", "op", "part", "pin_flag",
    "reaction_flag", "role", "statement_flag", "watch",
    // piperine_abi::shape
    "FunctionSpec", "InputSpec", "PinSpec", "ReactionSpec", "ShapeBuf", "ShapeRef", "TermSpec", "WatchSpec",
    // std::sync
    "Arc", "OnceLock",
];

/// A source text being written, line by line.
#[derive(Debug, Default)]
pub struct Text {
    out: String,
    depth: usize,
}

impl Text {
    pub fn new() -> Text {
        Text::default()
    }

    /// One line at the current depth; an empty `text` is an empty line, with no indentation. A
    /// `text` of several lines (an `if` or a block) has each of its lines placed at that depth.
    pub fn line(&mut self, text: &str) {
        for line in text.split('\n') {
            if !line.is_empty() {
                for _ in 0..self.depth {
                    self.out.push_str("    ");
                }
                self.out.push_str(line);
            }
            self.out.push('\n');
        }
    }

    /// The lines `write` writes, one level deeper.
    pub fn indented(&mut self, write: impl FnOnce(&mut Text)) {
        self.depth += 1;
        write(self);
        self.depth -= 1;
    }

    /// The comment an item sits under: `// <path> @ <file>:<line>:<col>`.
    pub fn comment(&mut self, path: &str, origin: &Origin) {
        self.line(&format!("// {path} @ {}:{}:{}", origin.file, origin.line, origin.col));
    }

    /// The text written so far.
    pub fn finish(self) -> String {
        self.out
    }

    /// `v` as a Rust `f64` expression: the shortest decimal that parses back to the same bits,
    /// always with a `.` or an exponent so it is a float literal; an infinity or a NaN by its
    /// `f64` constant's name.
    pub fn real(v: f64) -> String {
        if v.is_nan() {
            "f64::NAN".to_string()
        } else if v == f64::INFINITY {
            "f64::INFINITY".to_string()
        } else if v == f64::NEG_INFINITY {
            "f64::NEG_INFINITY".to_string()
        } else {
            format!("{v:?}")
        }
    }

    /// The MSL name `name` as a Rust identifier: unchanged; `r#name` for a keyword; `name_` for
    /// a name that ends in `_`, one of the four keywords Rust refuses as raw identifiers, or a
    /// name the generated code refers to.
    pub fn ident(name: &str) -> String {
        // `~` marks a variant the front end minted (`rev3~1`); Rust has no such character in a
        // name, and an MSL name has none, so it prints as `_v` (`rev3_v1`, a snake-case name).
        let name = name.replace('~', "_v");
        let name = name.as_str();
        if name.ends_with('_') || UNRAWABLE.contains(&name) || RESERVED.contains(&name) {
            format!("{name}_")
        } else if KEYWORDS.contains(&name) {
            format!("r#{name}")
        } else {
            name.to_string()
        }
    }

    /// A name the printer makes from `word` (`cx`, `vd_limit`, `b0_flow`): `word_`, a name no
    /// MSL name prints as. `word` must not end in `_` and must not be a reserved name.
    pub fn generated(word: &str) -> String {
        debug_assert!(!word.ends_with('_') && !RESERVED.contains(&word) && !UNRAWABLE.contains(&word), "{word}");
        format!("{word}_")
    }
}

#[cfg(test)]
mod tests {
    use super::Text;
    use piperine_ir::Origin;

    /// The significant digits of a decimal literal: its mantissa's digits, leading and trailing
    /// zeros dropped.
    fn significant(literal: &str) -> usize {
        let mantissa = literal.trim_start_matches('-').split('e').next().unwrap_or("");
        let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
        let digits = digits.trim_start_matches('0').trim_end_matches('0');
        digits.len().max(1)
    }

    /// A finite `v` prints as a float literal that parses back to its bits, and no decimal with
    /// one significant digit fewer does (the nearest such decimal would if any did).
    fn round_trips_shortest(v: f64) {
        let text = Text::real(v);
        assert!(text.contains('.') || text.contains('e'), "{text} is not a float literal");
        let back: f64 = text.parse().unwrap_or_else(|e| panic!("{text}: {e}"));
        assert_eq!(back.to_bits(), v.to_bits(), "{text} does not parse back to {v:e}");
        let n = significant(&text);
        if n > 1 {
            let shorter = format!("{v:.*e}", n - 2);
            let parsed: f64 = shorter.parse().unwrap_or_else(|e| panic!("{shorter}: {e}"));
            assert_ne!(parsed.to_bits(), v.to_bits(), "{shorter} is shorter than {text} and also round-trips");
        }
    }

    #[test]
    fn reals_round_trip_in_the_shortest_form() {
        // the forms a reader expects, written from the values themselves
        assert_eq!(Text::real(0.1), "0.1");
        assert_eq!(Text::real(1e-14), "1e-14");
        assert_eq!(Text::real(-0.0), "-0.0");
        assert_eq!(Text::real(1.0), "1.0");
        assert_eq!(Text::real(1000.0), "1000.0");
        assert_eq!(Text::real(5e-324), "5e-324");
        assert_eq!(Text::real(f64::INFINITY), "f64::INFINITY");
        assert_eq!(Text::real(f64::NEG_INFINITY), "f64::NEG_INFINITY");
        assert_eq!(Text::real(f64::NAN), "f64::NAN");
        assert_eq!(Text::real(f64::from_bits(0x7ff8_0000_0000_0001)), "f64::NAN");

        // the subnormals: the smallest, the largest, and a spread between them
        let smallest_normal = f64::MIN_POSITIVE.to_bits();
        let subnormals = [1u64, 2, 3, 0x000f_ffff_ffff_ffff, 0x0008_0000_0000_0000, 0x0000_0123_4567_89ab]
            .into_iter()
            .chain((1..64).map(|k| smallest_normal / 64 * k));
        for bits in subnormals {
            let v = f64::from_bits(bits);
            assert!(v.is_subnormal(), "{bits:#x}");
            round_trips_shortest(v);
            round_trips_shortest(-v);
        }
        for v in [-0.0, 0.0, 1e-14, 0.1, f64::MAX, f64::MIN, f64::MIN_POSITIVE, f64::EPSILON] {
            round_trips_shortest(v);
        }

        // 1000 bit patterns from a fixed linear congruential sequence: every finite one
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        let mut finite = 0;
        for _ in 0..1000 {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            let v = f64::from_bits(state);
            if v.is_finite() {
                round_trips_shortest(v);
                finite += 1;
            } else {
                assert!(Text::real(v).starts_with("f64::"), "{v}");
            }
        }
        assert!(finite > 900, "{finite} finite patterns of 1000");
    }

    #[test]
    fn keywords_become_raw_identifiers() {
        for (msl, rust) in [
            ("fn", "r#fn"),
            ("match", "r#match"),
            ("type", "r#type"),
            ("gen", "r#gen"),
            ("async", "r#async"),
            ("yield", "r#yield"),
            ("self", "self_"),
            ("crate", "crate_"),
            // MSL names that are not Rust keywords stay as written
            ("r", "r"),
            ("vt", "vt"),
            ("is", "is"),
            ("pnjlim", "pnjlim"),
            ("union", "union"),
            ("Resistor", "Resistor"),
        ] {
            assert_eq!(Text::ident(msl), rust, "{msl}");
        }
    }

    #[test]
    fn an_msl_name_never_prints_as_a_generated_one() {
        // a name that ends in `_`, one of the four unrawable keywords, or a name the generated
        // code refers to takes one more `_`; any other name prints as written
        for (msl, rust) in [
            ("self", "self_"),
            ("self_", "self__"),
            ("self__", "self___"),
            ("cx", "cx"),
            ("cx_", "cx__"),
            ("vd_limit_", "vd_limit__"),
            ("limit_vd_last", "limit_vd_last"),
            ("arg_4", "arg_4"),
            ("Ok", "Ok_"),
            ("f64", "f64_"),
            ("Real", "Real_"),
            ("rt", "rt_"),
            ("module", "module_"),
            ("library", "library_"),
            ("_new", "_new"),
        ] {
            assert_eq!(Text::ident(msl), rust, "{msl}");
        }
        // the printer's own names: one trailing `_` after a character that is not `_`
        for (word, rust) in [("cx", "cx_"), ("vd_limit", "vd_limit_"), ("x__limit", "x__limit_"), ("arg_4", "arg_4_")] {
            assert_eq!(Text::generated(word), rust, "{word}");
        }
        // one to one over names that differ only in their `_`s and the reserved words, and
        // apart from every generated name
        let msl = ["x", "x_", "x__", "self", "self_", "self__", "Ok", "Ok_", "cx", "cx_", "vd_limit", "vd_limit_"];
        let printed: Vec<String> = msl.iter().map(|n| Text::ident(n)).collect();
        for (i, a) in printed.iter().enumerate() {
            for (j, b) in printed.iter().enumerate() {
                assert!(i == j || a != b, "`{}` and `{}` both print as `{a}`", msl[i], msl[j]);
            }
        }
        for word in ["x", "cx", "vd_limit", "arg_4", "b0_flow", "temperature", "is_static"] {
            let generated = Text::generated(word);
            assert!(!printed.contains(&generated), "{generated} is also an MSL name's print");
        }
    }

    #[test]
    fn lines_are_indented_by_four_one_statement_each() {
        let at = Origin { file: "tests/ngspice/devices.msl".into(), line: 8, col: 23 };
        let mut t = Text::new();
        t.comment("body Resistor", &Origin { file: "tests/ngspice/devices.msl".into(), line: 5, col: 1 });
        t.line("pub mod resistor {");
        t.indented(|t| {
            t.line("use super::*;");
            t.line("");
            t.line("fn terms_block<R: Real>(p: R, n: R, r: R) -> [R; 1] {");
            t.indented(|t| {
                t.comment("b0.flow", &at);
                t.line("let b0_flow = (p - n) / r;");
                t.line("[b0_flow]");
            });
            t.line("}");
        });
        t.line("}");
        let expected = "\
// body Resistor @ tests/ngspice/devices.msl:5:1
pub mod resistor {
    use super::*;

    fn terms_block<R: Real>(p: R, n: R, r: R) -> [R; 1] {
        // b0.flow @ tests/ngspice/devices.msl:8:23
        let b0_flow = (p - n) / r;
        [b0_flow]
    }
}
";
        assert_eq!(t.finish(), expected);
    }
}
