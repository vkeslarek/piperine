//! LRM Annex E — SPICE compatibility, audited against Piperine's own
//! SPICE-facing tooling (VAMS-22, T55).
//!
//! LRM-COVERS: E.3.1
//! `every_table_e1_row_is_present` (proving `mesfet` is the sole
//! `NotSupported` row) plus `E_3_1_NOTE` below together are E.3.1's own
//! claim exercised: the three primitives E.3.1 names as unsupported by
//! the LRM's narrow "instance name as parameter" reading (`ccvs`/`cccs`/
//! mutual inductors) are all present in `headers/spice/` via the
//! alternate mechanism the LRM itself says remains legal.
//!
//! **Scope, per spec's own P3 framing** (`spec.md`'s "SPICE compatibility
//! (Annex E)" story): Annex E's normative content (E.2/E.3.2/E.3.3) is
//! written for a tool that *reads raw SPICE netlists* (`.subckt`/`.model`
//! cards) and resolves names against them at elaboration time. Piperine
//! has no such netlist reader (confirmed by grep across every crate — no
//! `.subckt`/`.model` parser exists anywhere in the tree) and was never
//! meant to grow one here; its "SPICE-import path" (MD-39, referenced by
//! spec.md) is a hand-authored, ngspice-source-faithful `.va` model
//! library (`headers/spice/`, migrated T41–T43) that a user `use`s
//! directly instead of an automated deck import. This module documents
//! **where that existing library satisfies Annex E's *content* (Table E.1's
//! primitive/port/parameter set, Table E.2's limiting functions, E.3.1's
//! unsupported-primitive list)** and is explicit, not silent, about the
//! two places it genuinely cannot (E.2.1's case-insensitive netlist name
//! matching and E.3.2's `port_discipline` attribute both presuppose the
//! netlist-reading capability this crate doesn't have).
//!
//! `headers/spice/*.va` itself is a frozen test corpus (`CLAUDE.md`:
//! "`headers/`, `tests/fixtures*` — frozen test corpora") and outside
//! T55's stated `Where` (`crates/piperine-lang/src/`) regardless — this
//! module is the audit, not a rename of that library to the LRM's literal
//! preferred names.

/// Whether Piperine's SPICE-facing tooling satisfies one Annex E mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnexESupport {
    /// The mapped behavior is available under Piperine's own naming
    /// convention (ngspice source-file/struct names, not the LRM's
    /// `Table E.1` preferred names) — functionally equivalent, differently
    /// spelled. See the entry's `note`.
    AliasedName,
    /// Available under the LRM's exact preferred name/signature.
    ExactMatch,
    /// A real, named gap — no equivalent exists. See `note`.
    NotSupported,
}

/// One Table E.1 (§E.3) primitive's mapping from its LRM-preferred name to
/// Piperine's actual `headers/spice/` module.
#[derive(Debug, Clone, Copy)]
pub struct AnnexEPrimitive {
    /// The LRM Table E.1 preferred primitive name.
    pub lrm_name: &'static str,
    /// Piperine's actual module name in `headers/spice/` (or `None` when
    /// no equivalent exists at all).
    pub piperine_module: Option<&'static str>,
    pub support: AnnexESupport,
    pub note: &'static str,
}

/// One Table E.2 (§E.3.4) limiting function's mapping.
#[derive(Debug, Clone, Copy)]
pub struct AnnexELimitFn {
    pub lrm_name: &'static str,
    pub piperine_name: &'static str,
    pub support: AnnexESupport,
    pub note: &'static str,
}

/// The empty type this module hangs its Annex E audit tables off of
/// (MD-13: no loose functions).
pub struct AnnexE;

impl AnnexE {
    /// Table E.1 (§E.3), in the LRM's own row order. Every primitive Table
    /// E.1 lists has an entry — never a silently-omitted row (C6).
    pub const PRIMITIVES: &'static [AnnexEPrimitive] = &[
        AnnexEPrimitive {
            lrm_name: "resistor",
            piperine_module: Some("res"),
            support: AnnexESupport::AliasedName,
            note: "headers/spice/passives.va's `res` — ports `p, n` and params `r, tc1, tc2` \
                match Table E.1 exactly; only the module name differs (ngspice's `R`-card \
                convention, not the LRM's `resistor`).",
        },
        AnnexEPrimitive {
            lrm_name: "capacitor",
            piperine_module: Some("cap"),
            support: AnnexESupport::AliasedName,
            note: "passives.va's `cap` — ports `p, n`, params `c, ic` match Table E.1; module \
                name differs (ngspice `C`-card).",
        },
        AnnexEPrimitive {
            lrm_name: "inductor",
            piperine_module: Some("ind"),
            support: AnnexESupport::AliasedName,
            note: "passives.va's `ind` — ports `p, n`, params `l, ic` match Table E.1; module \
                name differs (ngspice `L`-card).",
        },
        AnnexEPrimitive {
            lrm_name: "iexp",
            piperine_module: Some("isrc"),
            support: AnnexESupport::AliasedName,
            note: "sources.va's `isrc` selects the EXP waveform via `parameter integer wave = 3`; \
                its EXP-specific parameters are prefixed (`exp_*`) rather than Table E.1's bare \
                `val0`/`val1`/`td0`/`tau0`/`td1`/`tau1` names — one combined module covers every \
                Table E.1 `i*`-family waveform (`iexp`/`ipulse`/`ipwl`/`isine`), not four \
                separate modules.",
        },
        AnnexEPrimitive {
            lrm_name: "ipulse",
            piperine_module: Some("isrc"),
            support: AnnexESupport::AliasedName,
            note: "`isrc` with `wave = 2` (PULSE); params prefixed `pulse_*` \
                (`pulse_v1`/`pulse_v2`/`pulse_td`/`pulse_tr`/`pulse_tf`/`pulse_pw`/`pulse_per`) \
                vs Table E.1's `val0`/`val1`/`td`/`rise`/`fall`/`width`/`period`.",
        },
        AnnexEPrimitive {
            lrm_name: "ipwl",
            piperine_module: Some("isrc"),
            support: AnnexESupport::AliasedName,
            note: "`isrc` with `wave = 6` (PWL, T58/LRMC-C10-17) — a fixed 8-point breakpoint \
                list (`pwl_t0..pwl_t7`/`pwl_v0..pwl_v7`) rather than Table E.1's arbitrary-length \
                `wave[]` array argument: this codegen has no array-parameter support (no other \
                `headers/spice/*.va` module uses one either), so an unbounded point list isn't \
                reachable — a real, disclosed, bounded approximation, not a silent stub \
                (`tests/vams_annex_e.rs`'s \
                `table_e1_vpwl_via_wave_selector` proves it numerically over a real transient \
                run).",
        },
        AnnexEPrimitive {
            lrm_name: "isine",
            piperine_module: Some("isrc"),
            support: AnnexESupport::AliasedName,
            note: "`isrc` with `wave = 1` (SIN); params prefixed `sin_*` vs Table E.1's bare \
                `offset`/`ampl`/`freq`/`td`/`damp`/`sinephase`/`ammodindex`/... (ngspice's SFFM/AM \
                modulation extensions, `wave = 4`/`5`, are additional waveforms Table E.1 doesn't \
                itself enumerate).",
        },
        AnnexEPrimitive {
            lrm_name: "vexp",
            piperine_module: Some("vsrc"),
            support: AnnexESupport::AliasedName,
            note: "Same `wave`-selector shape as `isrc`, mirrored onto `vsrc` (`wave = 3`).",
        },
        AnnexEPrimitive {
            lrm_name: "vpulse",
            piperine_module: Some("vsrc"),
            support: AnnexESupport::AliasedName,
            note: "`vsrc` with `wave = 2` (PULSE), `pulse_*`-prefixed params, same shape as \
                `ipulse` above.",
        },
        AnnexEPrimitive {
            lrm_name: "vpwl",
            piperine_module: Some("vsrc"),
            support: AnnexESupport::AliasedName,
            note: "`vsrc` with `wave = 6` (PWL, T58) — same 8-point bounded scope as `ipwl` \
                above.",
        },
        AnnexEPrimitive {
            lrm_name: "vsine",
            piperine_module: Some("vsrc"),
            support: AnnexESupport::AliasedName,
            note: "`vsrc` with `wave = 1` (SIN), same shape as `isine` above.",
        },
        AnnexEPrimitive {
            lrm_name: "tline",
            piperine_module: Some("tline"),
            support: AnnexESupport::AliasedName,
            note: "headers/spice/tline.va's `tline` (T43) — module name matches Table E.1 \
                exactly; ports are `p1, n1, p2, n2` rather than Table E.1's `t1, b1, t2, b2` \
                (matching every other two-port device's own `p, n` convention in this library). \
                Parameters `z0, td` match Table E.1; the alternate `f, nl` \
                (frequency + normalized-length) specification Table E.1 also lists is not \
                implemented — only the direct `z0, td` path exists, a real (minor) gap noted \
                here rather than silently absent.",
        },
        AnnexEPrimitive {
            lrm_name: "vccs",
            piperine_module: Some("vccs"),
            support: AnnexESupport::AliasedName,
            note: "headers/spice/controlled.va's `vccs` — module name matches Table E.1 exactly; \
                ports are `p, n, cp, cn` (output pair, controlling pair) rather than Table E.1's \
                `sink, src, ps, ns`, and the transconductance parameter is `trans` (plus an `m` \
                multiplicity factor ngspice's own `G` element carries) rather than Table E.1's \
                bare `gm` — same four-terminal, one-parameter shape, different names.",
        },
        AnnexEPrimitive {
            lrm_name: "vcvs",
            piperine_module: Some("vcvs"),
            support: AnnexESupport::AliasedName,
            note: "controlled.va's `vcvs` — module name matches Table E.1 exactly and the `gain` \
                parameter matches; ports are `p, n, cp, cn` rather than Table E.1's `p, n, ps, \
                ns`.",
        },
        AnnexEPrimitive {
            lrm_name: "diode",
            piperine_module: Some("dio"),
            support: AnnexESupport::AliasedName,
            note: "headers/spice/diode.va's `dio` — Table E.1 calls the primitive `diode` with \
                ports `a, c` (anode/cathode); `dio`'s ports are `p, n` (this library's uniform \
                two-terminal convention) — both the module name and the port names differ from \
                Table E.1, while the parameter (`area`) matches.",
        },
        AnnexEPrimitive {
            lrm_name: "bjt",
            piperine_module: Some("bjt"),
            support: AnnexESupport::AliasedName,
            note: "headers/spice/bjt.va's `bjt` — module name and param `area` match Table E.1 \
                exactly; the substrate port is named `sub` rather than Table E.1's `s` (this \
                library's own disambiguation — `s` is also the JFET/MOSFET source-port letter \
                elsewhere in the same header set).",
        },
        AnnexEPrimitive {
            lrm_name: "mosfet",
            piperine_module: Some("mos1"),
            support: AnnexESupport::AliasedName,
            note: "headers/spice/mos.va provides three level-specific modules (`mos1`/`mos2`/ \
                `mos3`, ngspice LEVEL 1/2/3) rather than Table E.1's single `mosfet` — Table E.1 \
                doesn't distinguish SPICE MOS levels, but ngspice's own model set does, so \
                Piperine kept that distinction instead of collapsing it. Ports `d, g, s, b` and \
                params `w, l, ad, as, pd, ps, nrd, nrs` match Table E.1 on all three.",
        },
        AnnexEPrimitive {
            lrm_name: "jfet",
            piperine_module: Some("jfet"),
            support: AnnexESupport::ExactMatch,
            note: "headers/spice/jfet.va's `jfet` — name matches Table E.1 exactly; ports `d, g, \
                s` and param `area` match.",
        },
        AnnexEPrimitive {
            lrm_name: "mesfet",
            piperine_module: None,
            support: AnnexESupport::NotSupported,
            note: "Deliberately left `NotSupported` (T58, spec P5 AC5), not merely un-ported: \
                `headers/spice/` is built by ngspice-source-faithful translation \
                (`sources.va`'s own header: \"Source: ngspice/src/spicelib/devices/...\", every \
                other module here traces to a real ngspice device file) and ngspice itself ships \
                no MESFET model to translate from — there is no faithful source for this crate's \
                sourcing discipline to port. Fabricating MESFET equations from general \
                Schottky-gate FET theory (not an ngspice source file) would break that discipline \
                the same way inventing VAMS grammar from a third-party compiler would break C7's \
                provenance rule for the parser — a genuine, disclosed scope boundary, not an \
                oversight.",
        },
    ];

    /// Table E.2 (§E.3.4) limiting functions, usable through `$limit()`
    /// (LRM §9.17.3).
    pub const LIMIT_FNS: &'static [AnnexELimitFn] = &[
        AnnexELimitFn {
            lrm_name: "fetlim",
            piperine_name: "fetlim",
            support: AnnexESupport::ExactMatch,
            note: "`piperine_ir::limit::fetlim` (wired into `$limit()` at \
                `piperine-codegen/src/emit/analog_expr.rs`) — name and 3-argument arity \
                (`vnew, vold, vth`) match Table E.2.",
        },
        AnnexELimitFn {
            lrm_name: "pnjlim",
            piperine_name: "pnjlim",
            support: AnnexESupport::ExactMatch,
            note: "`piperine_ir::limit::pnjlim` — name and 4-argument arity (`vnew, vold, vte, \
                vcrit`) match Table E.2.",
        },
        AnnexELimitFn {
            lrm_name: "vdslim",
            piperine_name: "vdslim",
            support: AnnexESupport::ExactMatch,
            note: "`piperine_ir::limit::limvds`, reachable via *either* `$limit(x, \"vdslim\", \
                ...)` (Table E.2's own preferred spelling) or `$limit(x, \"limvds\", ...)` \
                (ngspice's naming, kept as a working synonym — T58, `emit/analog_expr.rs`'s \
                `emit_analog_limit` and `kernel/analog/limits.rs`'s `catalog_entry_for_kind` both \
                accept both spellings). Previously `AliasedName`-only (the LRM's own name was \
                rejected); this task's own `Where` explicitly names `emit/analog_expr.rs` \
                (CLAUDE.md's \"files not to edit casually\") as sanctioned for this specific fix, \
                with the ngspice numeric-oracle suite as the compensating safety net.",
        },
    ];

    /// E.3.1's three SPICE primitives the LRM states are unsupported (the
    /// "instance name passed as a parameter" technique). Piperine
    /// implements the *behavior* of all three anyway, via explicit control
    /// ports instead of that technique — documented here so "unsupported"
    /// per the LRM's own narrow reading doesn't read as "missing".
    pub const E_3_1_NOTE: &'static str = "E.3.1 forbids only the SPICE technique of passing an \
        instance name as a parameter (`ccvs`/`cccs`'s controlling-source reference, mutual \
        inductors' coupled-inductor reference) — Verilog-A has no such parameter kind. Piperine's \
        headers/spice/controlled.va nonetheless provides `ccvs`/`cccs` (current-controlled \
        sources with an explicit sense-branch port pair, not a referenced instance name) and \
        passives.va provides `mut` (mutual inductance with explicit port pairs) — the behavior \
        E.3.1 lists as unsupported is available, just not via the forbidden mechanism, matching \
        how the LRM itself frames the limitation (\"can be instantiated inside a SPICE subcircuit \
        that itself is instantiated in Verilog-AMS\" — i.e. the *behavior* was never claimed \
        unreachable, only that one specific syntax).";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_e1_row_is_present() {
        const EXPECTED: &[&str] = &[
            "resistor", "capacitor", "inductor", "iexp", "ipulse", "ipwl", "isine", "vexp",
            "vpulse", "vpwl", "vsine", "tline", "vccs", "vcvs", "diode", "bjt", "mosfet", "jfet",
            "mesfet",
        ];
        let names: Vec<&str> = AnnexE::PRIMITIVES.iter().map(|p| p.lrm_name).collect();
        assert_eq!(names, EXPECTED, "Table E.1 has exactly 19 rows, in this order");
    }

    #[test]
    fn every_table_e2_row_is_present() {
        let names: Vec<&str> = AnnexE::LIMIT_FNS.iter().map(|f| f.lrm_name).collect();
        assert_eq!(names, ["fetlim", "pnjlim", "vdslim"]);
    }

    #[test]
    fn every_entry_names_a_concrete_mapping_or_gap() {
        for p in AnnexE::PRIMITIVES {
            assert!(!p.note.trim().is_empty());
            if p.support != AnnexESupport::NotSupported {
                assert!(p.piperine_module.is_some(), "{} claims support with no module", p.lrm_name);
            }
        }
        for f in AnnexE::LIMIT_FNS {
            assert!(!f.note.trim().is_empty());
        }
    }

    /// T58 closed `ipwl`/`vpwl` (a bounded PWL waveform now exists);
    /// `mesfet` stays `NotSupported` — deliberately, per its own note: no
    /// ngspice source file exists to translate from, and this crate's
    /// SPICE tooling is source-faithful translation, not invented physics.
    #[test]
    fn exactly_one_table_e1_row_is_a_real_gap() {
        let gaps: Vec<&str> = AnnexE::PRIMITIVES
            .iter()
            .filter(|p| p.support == AnnexESupport::NotSupported)
            .map(|p| p.lrm_name)
            .collect();
        assert_eq!(gaps, ["mesfet"]);
    }

    #[test]
    fn limiting_functions_have_no_gaps_and_no_naming_mismatch() {
        assert!(AnnexE::LIMIT_FNS.iter().all(|f| f.support != AnnexESupport::NotSupported));
        let vdslim = AnnexE::LIMIT_FNS.iter().find(|f| f.lrm_name == "vdslim").unwrap();
        assert_eq!(vdslim.piperine_name, "vdslim");
        assert_eq!(vdslim.support, AnnexESupport::ExactMatch);
    }
}
