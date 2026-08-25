//! Annex A grammar conformance gate (tasks.md T13, VAMS-23). Mechanically
//! proves every LRM Annex A section either (a) has an implementing grammar
//! method — indexed by the `Annex A.X.Y` citation in that method's doc
//! comment, the convention every `grammar/{expr,stmt,item}.rs` production
//! already follows — or (b) is explicitly marked out of scope with a
//! written reason naming the task/phase that will cover it. No section is
//! ever silently unaccounted for.
//!
//! **Granularity note (SPEC_DEVIATION from a literal "every production"
//! reading of T13's Done-when):** `VAMS-LRM-2-4.md`'s Annex A is a PDF-to-
//! text extraction with unreliable line-wrapping; a regex over raw
//! `name ::=` tokens yields ~500 noisy, often-merged pseudo-productions
//! that do not correspond to real grammar rules. The LRM's own section
//! *headers* (down to the leaf level actually used — e.g. A.2.1.1/A.2.1.2/
//! A.2.1.3 where A.2.1 has no direct content, but a bare leaf like A.2.3
//! where it does) are the reliable, matches-the-actual-document unit, and
//! are exactly the granularity `grammar/*.rs` doc comments already cite.
//! Reason: within-task correction — a rule-level extraction was attempted
//! first and produced unusable noise; section-level is what the source
//! document actually supports mechanically.
//!
//! LRM-COVERS: A.1, A.1.1, A.1.2, A.1.3, A.1.4, A.1.6, A.1.7, A.1.8, A.1.9, A.2, A.2.1, A.2.1.1, A.2.1.2, A.2.3, A.2.6, A.2.7, A.2.8, A.4, A.4.1, A.4.2, A.5.1, A.5.2, A.6, A.6.1, A.6.3, A.6.4, A.6.5, A.6.6, A.6.7, A.6.8, A.6.10, A.7, A.7.1, A.8, A.8.1, A.8.3, A.9, A.9.1, A.9.3
//!
//! Every `Coverage::Implemented` row above whose id appears in this list
//! is proved by `every_annex_a_section_is_implemented_or_carries_a_
//! written_reason` below: the row's citation-needle (`"Annex A.X.Y"`)
//! must actually appear in a live `grammar/*.rs` production, checked
//! mechanically, not just claimed in a doc comment. The bare parent ids
//! (`A.1`/`A.2`/`A.2.1`/`A.4`/`A.6`/`A.7`/`A.8`/`A.9`) are pure
//! organizational headings in the LRM itself — each is a `##`-level
//! heading with no text of its own before its first `###` child (verified
//! against `VAMS-LRM-2-4.md` directly) — carried on the same declaration
//! as a genuinely `Implemented` child per that child's own row above.

use std::fs;
use std::path::Path;

enum Coverage {
    /// A grammar method's doc comment cites this section (`Annex A.X.Y`).
    Implemented,
    /// Not yet implemented — the reason names what covers it and when.
    OutOfScope(&'static str),
}

struct Section {
    id: &'static str,
    title: &'static str,
    coverage: Coverage,
}

/// Every LRM Annex A section, leaf granularity, in document order. Grep
/// ground truth: `sed -n '16568,17932p' VAMS-LRM-2-4.md` (Annex A's full
/// text span in the in-repo LRM copy).
const SECTIONS: &[Section] = &[
    Section { id: "A.1.1", title: "Library source text", coverage: Coverage::Implemented },
    Section { id: "A.1.2", title: "Verilog source text", coverage: Coverage::Implemented },
    Section { id: "A.1.3", title: "Module parameters and ports", coverage: Coverage::Implemented },
    Section { id: "A.1.4", title: "Module items", coverage: Coverage::Implemented },
    Section {
        id: "A.1.5",
        title: "Configuration source text",
        coverage: Coverage::OutOfScope(
            "config_declaration body (design/config_rule statements) — T9 parses the \
             declaration shell only; full config-rule grammar deferred, no VAMS-NN maps \
             to it (not cited by spec.md's LRM Coverage Map as a P1-P3 requirement beyond \
             the declaration existing)",
        ),
    },
    Section { id: "A.1.6", title: "Nature Declaration", coverage: Coverage::Implemented },
    Section { id: "A.1.7", title: "Discipline Declaration", coverage: Coverage::Implemented },
    Section {
        id: "A.1.8",
        title: "Connectrules Declaration",
        coverage: Coverage::Implemented,
    },
    Section { id: "A.1.9", title: "Paramset Declaration", coverage: Coverage::Implemented },
    Section { id: "A.2.1.1", title: "Module parameter declarations", coverage: Coverage::Implemented },
    Section { id: "A.2.1.2", title: "Port declarations", coverage: Coverage::Implemented },
    Section { id: "A.2.1.3", title: "Type declarations", coverage: Coverage::Implemented },
    Section {
        id: "A.2.2.1",
        title: "Net and variable types",
        coverage: Coverage::OutOfScope("net_type/strength/charge-storage keyword set beyond `electrical`-style disciplines — full 1364 net-type table deferred to T47/T48 (Phase 9, VAMS-15/18)"),
    },
    Section {
        id: "A.2.2.2",
        title: "Strengths",
        coverage: Coverage::OutOfScope("drive-strength grammar exists (T11) but strength disambiguation from unnamed-instance terminals is a known non-blocking gap logged in T11's SPEC_DEVIATION; full semantics land with gate primitives, T48 (Phase 9, VAMS-18)"),
    },
    Section {
        id: "A.2.2.3",
        title: "Delays",
        coverage: Coverage::OutOfScope("delay expressions on gate/UDP instances — deferred to T48/T49 (Phase 9, VAMS-18/19) alongside the primitives that use them"),
    },
    Section { id: "A.2.3", title: "Declaration lists", coverage: Coverage::Implemented },
    Section {
        id: "A.2.4",
        title: "Declaration assignments",
        coverage: Coverage::OutOfScope("net/param declaration-with-initializer forms beyond what T9's parameter grammar already covers are exercised incrementally as later phases' fixtures need them; no VAMS-NN maps to this section standalone"),
    },
    Section {
        id: "A.2.5",
        title: "Declaration ranges",
        coverage: Coverage::OutOfScope("bit-range (`[msb:lsb]`) on port/net declarators beyond T9's `BitRange` type is exercised as later fixtures need it (e.g. bus-width examples in Phase 8 migration); no VAMS-NN maps to this section standalone"),
    },
    Section { id: "A.2.6", title: "Function declarations", coverage: Coverage::Implemented },
    Section { id: "A.2.7", title: "Task declarations", coverage: Coverage::Implemented },
    Section { id: "A.2.8", title: "Block item declarations", coverage: Coverage::Implemented },
    Section {
        id: "A.3.1",
        title: "Primitive instantiation and instances",
        coverage: Coverage::OutOfScope("T11 parses gate_instantiation's shell (name/terminals/strength/delay tokens); per-primitive truth-table semantics are T48 (Phase 9, VAMS-18)"),
    },
    Section {
        id: "A.3.2",
        title: "Primitive strengths",
        coverage: Coverage::OutOfScope("see A.2.2.2 — same non-blocking gap, same owner (T48)"),
    },
    Section {
        id: "A.3.3",
        title: "Primitive terminals",
        coverage: Coverage::OutOfScope("terminal-list grammar is covered by T11's gate_instantiation parse; per-terminal drive semantics are T48 (Phase 9, VAMS-18)"),
    },
    Section {
        id: "A.3.4",
        title: "Primitive gate and switch types",
        coverage: Coverage::OutOfScope("the full IEEE 1364 gate-keyword table (and/nand/buf/bufif0/1/.../pass switches) is enumerated by T48 (Phase 9, VAMS-18); T11 parses gate_instantiation generically over any keyword"),
    },
    Section { id: "A.4.1", title: "Module instantiation", coverage: Coverage::Implemented },
    Section { id: "A.4.2", title: "Generate construct", coverage: Coverage::Implemented },
    Section { id: "A.5.1", title: "UDP declaration", coverage: Coverage::Implemented },
    Section { id: "A.5.2", title: "UDP ports", coverage: Coverage::Implemented },
    Section { id: "A.5.3", title: "UDP body", coverage: Coverage::Implemented },
    Section {
        id: "A.5.4",
        title: "UDP instantiation",
        coverage: Coverage::OutOfScope("UDP instantiation reuses A.3.1's primitive-instantiation grammar (T11); per-UDP state-table evaluation is T49 (Phase 9, VAMS-19)"),
    },
    Section { id: "A.6.1", title: "Continuous assignment statements", coverage: Coverage::Implemented },
    Section {
        id: "A.6.2",
        title: "Procedural blocks and assignments",
        coverage: Coverage::OutOfScope("blocking/non-blocking assignment statements themselves are covered under A.6.4 (Statements, implemented, T8); this section's `always`/`initial` construct wrapper is T10, not T8 — cited separately, not double-booked here"),
    },
    Section { id: "A.6.3", title: "Parallel and sequential blocks", coverage: Coverage::Implemented },
    Section { id: "A.6.4", title: "Statements", coverage: Coverage::Implemented },
    Section { id: "A.6.5", title: "Timing control statements", coverage: Coverage::Implemented },
    Section { id: "A.6.6", title: "Conditional statements", coverage: Coverage::Implemented },
    Section { id: "A.6.7", title: "Case statements", coverage: Coverage::Implemented },
    Section { id: "A.6.8", title: "Looping statements", coverage: Coverage::Implemented },
    Section {
        id: "A.6.9",
        title: "Task enable statements",
        coverage: Coverage::OutOfScope("bare task-call statement parses under A.6.4/A.6.9's `system_task_enable`/`task_enable` shape (T8's `bare_user_task_call_parses_as_statement`, `system_task_call_with_args_parses_as_statement`); real task binding/execution is T47's system-task implementation (Phase 9, VAMS-15), not grammar-only"),
    },
    Section { id: "A.6.10", title: "Contribution statements", coverage: Coverage::Implemented },
    Section {
        id: "A.7.1",
        title: "Specify block declaration",
        coverage: Coverage::Implemented,
    },
    Section {
        id: "A.7.2",
        title: "Specify path declarations",
        coverage: Coverage::OutOfScope("T11 treats the specify block body as an opaque span (stated scope in its own doc comment); full path-declaration grammar has no VAMS-NN mapped to it — not in spec.md's LRM Coverage Map beyond the block's existence"),
    },
    Section {
        id: "A.7.3",
        title: "Specify block terminals",
        coverage: Coverage::OutOfScope("see A.7.2 — same opaque-body scope decision"),
    },
    Section {
        id: "A.7.4",
        title: "Specify path delays",
        coverage: Coverage::OutOfScope("see A.7.2 — same opaque-body scope decision"),
    },
    Section {
        id: "A.7.5.1",
        title: "System timing check commands",
        coverage: Coverage::OutOfScope("see A.7.2 — same opaque-body scope decision"),
    },
    Section {
        id: "A.7.5.2",
        title: "System timing check command arguments",
        coverage: Coverage::OutOfScope("see A.7.2 — same opaque-body scope decision"),
    },
    Section {
        id: "A.7.5.3",
        title: "System timing check event definitions",
        coverage: Coverage::OutOfScope("see A.7.2 — same opaque-body scope decision"),
    },
    Section { id: "A.8.1", title: "Concatenations and assignment patterns", coverage: Coverage::Implemented },
    Section {
        id: "A.8.2",
        title: "Function calls",
        coverage: Coverage::OutOfScope("function-call-as-expression parses under A.8.3/A.8.4 (T7's call-expression grammar, cited there); this section's own header is not separately cited to avoid double-booking one implementation across two rows"),
    },
    Section { id: "A.8.3", title: "Expressions", coverage: Coverage::Implemented },
    Section {
        id: "A.8.4",
        title: "Primaries",
        coverage: Coverage::OutOfScope("primary-expression grammar (identifiers, literals, parenthesized exprs) is implemented as part of T7's Pratt parser under the A.8.3 citation (primaries are not a separable production in this implementation's structure, same reasoning as A.8.2)"),
    },
    Section {
        id: "A.8.5",
        title: "Expression left-side values",
        coverage: Coverage::OutOfScope("lvalue grammar is shared with A.6.4's assignment-statement target parsing (T8); no separate A.8.5-cited method exists because assignment targets and expressions share one expression parser by construction"),
    },
    Section {
        id: "A.8.6",
        title: "Operators",
        coverage: Coverage::OutOfScope("operator tokens are lexed (T3, LRM chapter 2's operator table) and consumed by T7's Pratt precedence table directly; no single method to cite for a token-class enumeration"),
    },
    Section {
        id: "A.8.7",
        title: "Numbers",
        coverage: Coverage::OutOfScope("number literal grammar is T3's lexer scope (LRM chapter 2, §2.6), not the parser's — Annex A.8.7 restates the lexical rule at the expression-grammar level; T3's own doc cites §2.6 directly rather than Annex A.8.7"),
    },
    Section {
        id: "A.8.8",
        title: "Strings",
        coverage: Coverage::OutOfScope("string literal grammar is T3's lexer scope (LRM §2.7), same reasoning as A.8.7"),
    },
    Section { id: "A.9.3", title: "Identifiers", coverage: Coverage::Implemented },
    Section {
        id: "A.8.9",
        title: "Analog references",
        coverage: Coverage::OutOfScope("`V(...)`/`I(...)` access-function call syntax parses as an ordinary A.8.3 function-call expression (T7); access-function *semantics* (binding to a POM Branch) is piperine-lang's job, T23 (Phase 5, VAMS-02), out of piperine-vams's parse-only scope entirely"),
    },
    Section {
        id: "A.9.1",
        title: "Attributes",
        coverage: Coverage::Implemented,
    },
    Section {
        id: "A.9.2",
        title: "Comments",
        coverage: Coverage::OutOfScope("comment lexing is T3's scope (LRM §2.4); Annex A.9.2 restates it at the grammar level, and T3's own doc cites §2.4 directly rather than Annex A.9.2"),
    },
    Section {
        id: "A.9.4",
        title: "White space",
        coverage: Coverage::OutOfScope("whitespace handling is T3's lexer scope (LRM §2.3); not a parser-level production"),
    },
];

#[test]
fn every_annex_a_section_is_implemented_or_carries_a_written_reason() {
    let source = concat_grammar_source();

    let mut unclaimed = Vec::new();
    let mut blank_reasons = Vec::new();

    for section in SECTIONS {
        match section.coverage {
            Coverage::Implemented => {
                let needle = format!("Annex {}", section.id);
                if !source.contains(&needle) {
                    unclaimed.push(format!("{} ({})", section.id, section.title));
                }
            }
            Coverage::OutOfScope(reason) => {
                if reason.trim().is_empty() {
                    blank_reasons.push(format!("{} ({})", section.id, section.title));
                }
            }
        }
    }

    assert!(
        unclaimed.is_empty(),
        "Annex A sections marked Implemented but no grammar/*.rs method cites them \
         (VAMS-23 conformance gap — fix the doc comment or downgrade to OutOfScope with \
         a reason): {unclaimed:?}"
    );
    assert!(
        blank_reasons.is_empty(),
        "Annex A sections marked OutOfScope with a blank reason (never allowed — name \
         what covers it and when): {blank_reasons:?}"
    );
}

#[test]
fn every_annex_a_section_id_is_unique_and_well_formed() {
    let mut seen = std::collections::HashSet::new();
    for section in SECTIONS {
        assert!(
            section.id.starts_with("A."),
            "section id {:?} must start with \"A.\"",
            section.id
        );
        assert!(
            seen.insert(section.id),
            "section {} ({}) is listed twice in SECTIONS — each Annex A leaf section \
             gets exactly one row",
            section.id,
            section.title
        );
    }
}

#[test]
fn the_section_table_covers_every_annex_a_leaf_the_lrm_defines() {
    // Cross-check against the in-repo LRM copy itself, so this table can't
    // silently drift from the document it claims to enumerate: every
    // "### **A.X.Y ...**" (and the three-level "#### **A.X.Y.Z ...**")
    // heading inside Annex A's span must have a matching SECTIONS row.
    let lrm_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../VAMS-LRM-2-4.md");
    let Ok(lrm) = fs::read_to_string(&lrm_path) else {
        eprintln!("VAMS-LRM-2-4.md not found at workspace root — skipping cross-check \
                    against the source document (the two other tests in this file still \
                    run against SECTIONS itself)");
        return;
    };

    let annex_a_start = lrm.find("## **A.1 Source text**").expect("Annex A.1 heading must exist in the LRM copy");
    let annex_b_start = lrm.find("# **Annex B**").expect("Annex B heading must exist in the LRM copy");
    let annex_a_text = &lrm[annex_a_start..annex_b_start];

    let mut missing = Vec::new();
    for line in annex_a_text.lines() {
        let trimmed = line.trim();
        let is_leaf_heading = trimmed.starts_with("### **A.")
            || (trimmed.starts_with("#### **A.") && trimmed.matches('.').count() >= 3);
        if !is_leaf_heading {
            continue;
        }
        let Some(id) = extract_section_id(trimmed) else { continue };
        // A.2.1/A.2.2 are parent headings with no direct content (their
        // A.2.1.x/A.2.2.x children carry it) — the SECTIONS table
        // deliberately skips the bare parent, matching how the LRM itself
        // has nothing but a sub-heading list under those two.
        if id == "A.2.1" || id == "A.2.2" || id == "A.7.5" {
            continue;
        }
        if !SECTIONS.iter().any(|s| s.id == id) {
            missing.push(id);
        }
    }

    assert!(
        missing.is_empty(),
        "Annex A leaf sections present in VAMS-LRM-2-4.md but missing from this file's \
         SECTIONS table (the gate's ground truth has drifted from the document): {missing:?}"
    );
}

/// Pulls `A.1.6` out of a heading line like `### **A.1.6 Nature
/// Declaration**` — the id is always the token right after the leading
/// `A.` prefix, up to the first space.
fn extract_section_id(heading_line: &str) -> Option<String> {
    let after_hashes = heading_line.trim_start_matches('#').trim_start().trim_start_matches("**");
    let id_end = after_hashes.find(' ')?;
    let id = &after_hashes[..id_end];
    if id.starts_with("A.") { Some(id.to_string()) } else { None }
}

fn concat_grammar_source() -> String {
    let grammar_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/grammar");
    let mut out = String::new();
    for name in ["mod.rs", "cursor.rs", "expr.rs", "stmt.rs", "item.rs"] {
        let path = grammar_dir.join(name);
        out.push_str(&fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}")));
    }
    out
}
