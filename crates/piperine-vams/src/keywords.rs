//! LRM Annex B — the complete set of Verilog-AMS HDL reserved keywords
//! (Table B.1), transcribed mechanically from `VAMS-LRM-2-4.md` (not
//! retyped by hand — see the extraction note below) so the list matches
//! the standard's own table exactly. §2.8.2 defines a keyword as "a
//! predefined simple identifier": lexically, `Lexer` emits every keyword as
//! a plain `Tok::Ident`; this table is what a grammar layer consults to
//! decide whether an identifier token is actually a keyword. An escaped
//! identifier is never treated as a keyword (§2.8.2), regardless of what
//! this table contains — `Keywords::is_reserved` only ever receives
//! `Tok::Ident` text from a caller that already made that distinction.

use crate::error::{ParseError, ParseErrorKind, Span};

/// All 212 keywords in Table B.1, in the table's own left-to-right,
/// top-to-bottom reading order. Extracted programmatically from the table
/// text in `VAMS-LRM-2-4.md` (a Python one-off run against the checked-in
/// LRM, not hand-typed) to eliminate transcription error — the ordering has
/// no semantic meaning, it exists purely so the source can be diffed
/// against the table by eye.
const KEYWORDS: &[&str] = &[
    "above", "defparam", "ifnone", "abs", "design", "incdir", "absdelay", "disable",
    "include", "absdelta", "discipline", "inf", "abstol", "discrete", "initial", "access",
    "domain", "initial_step", "acos", "driver_update", "inout", "acosh", "edge", "input",
    "ac_stim", "else", "instance", "aliasparam", "end", "integer", "always", "endcase",
    "join", "analog", "endconfig", "laplace_nd", "analysis", "endconnectrules", "laplace_np", "and",
    "enddiscipline", "laplace_zd", "asin", "endfunction", "laplace_zp", "asinh", "endgenerate", "large",
    "assert", "endmodule", "last_crossing", "assign", "endnature", "liblist", "atan", "endparamset",
    "library", "atan2", "endprimitive", "limexp", "atanh", "endspecify", "ln", "automatic",
    "endtable", "localparam", "begin", "endtask", "log", "branch", "event", "macromodule",
    "buf", "exclude", "max", "bufif0", "exp", "medium", "bufif1", "final_step",
    "merged", "case", "flicker_noise", "min", "casex", "floor", "module", "casez",
    "flow", "nand", "ceil", "for", "nature", "cell", "force", "negedge",
    "cmos", "forever", "net_resolution", "config", "fork", "nmos", "connect", "from",
    "noise_table", "connectmodule", "function", "noise_table_log", "connectrules", "generate", "nor", "continuous",
    "genvar", "noshowcancelled", "cos", "ground", "not", "cosh", "highz0", "notif0",
    "cross", "highz1", "notif1", "ddt", "hypot", "or", "ddt_nature", "idt",
    "output", "ddx", "idtmod", "parameter", "deassign", "idt_nature", "paramset", "default",
    "if", "pmos", "posedge", "slew", "units", "potential", "small", "unsigned",
    "pow", "specify", "use", "primitive", "specparam", "uwire", "pull0", "split",
    "vectored", "pull1", "sqrt", "wait", "pulldown", "string", "wand", "pullup",
    "strong0", "weak0", "pulsestyle_onevent", "strong1", "weak1", "pulsestyle_ondetect", "supply0", "while",
    "rcmos", "supply1", "white_noise", "real", "table", "wire", "realtime", "tan",
    "wor", "reg", "tanh", "wreal", "release", "task", "xnor", "repeat",
    "time", "xor", "resolveto", "timer", "zi_nd", "rnmos", "tran", "zi_np",
    "rpmos", "tranif0", "zi_zd", "rtran", "tranif1", "zi_zp", "rtranif0", "transition",
    "rtranif1", "tri", "scalared", "tri0", "sin", "tri1", "sinh", "triand",
    "showcancelled", "trior", "signed", "trireg",
];

/// Lookup over Table B.1's reserved keyword set.
pub struct Keywords;

impl Keywords {
    /// `true` if `word` is one of Table B.1's reserved keywords. Keywords
    /// are "defined in lowercase only" (§2.8.2) — this is a case-sensitive
    /// exact match, not a case-insensitive one.
    pub fn is_reserved(word: &str) -> bool {
        KEYWORDS.contains(&word)
    }

    /// The full keyword set, in Table B.1's reading order — the Annex B
    /// conformance gate (`tests/annex_b_keywords.rs`) iterates this.
    pub fn all() -> &'static [&'static str] {
        KEYWORDS
    }

    /// A grammar production that needs a plain identifier calls this on an
    /// `Ident` token's text before accepting it. Returns a typed error
    /// naming the keyword when `word` is reserved — the mechanism that
    /// makes "keywords cannot be used as identifiers" an enforced rule
    /// rather than a documented one.
    pub fn expect_identifier(word: &str, span: Span) -> Result<(), ParseError> {
        if Self::is_reserved(word) {
            return Err(ParseError::new(
                ParseErrorKind::UnexpectedToken {
                    expected: "an identifier".to_string(),
                    found: format!("keyword `{word}`"),
                },
                span,
            ));
        }
        Ok(())
    }
}
