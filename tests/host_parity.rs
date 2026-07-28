//! HOST-06 — parity test scaffold: the canonical analysis-method list is a
//! single source of truth checked against **both** public surfaces.
//!
//! Rust has no runtime reflection, so "the Rust `Session` has method `X`" is
//! proven the loudest way Rust can prove it: `assert_rust_session_has_every_
//! analysis` below calls every name in [`ANALYSES`] by hand — if a future
//! change removed one of `Session`'s analysis methods, this file would fail
//! to **compile**, not just fail a runtime assertion. The Python side is
//! checked at runtime (`hasattr`) against the same canonical list, embedded
//! through `piperine_python::embed::run_script` (mirrors
//! `piperine-python/tests/facade_hygiene.rs`'s native/facade parity
//! technique, extended cross-crate). `cargo test -p piperine host_parity`
//! (Phase 1 / T8 full gate).

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

/// The uniform analysis-method surface (HOST-02): every name here must
/// exist, spelled identically, as a method on both hosts' compiled session.
/// This is the parity oracle both checks below read from.
const ANALYSES: &[&str] = &["op", "tran", "ac", "noise", "sens", "pss", "pz", "disto", "sp", "tf", "dc"];

/// The uniform object-model surface (CLA-19 / MD-22): the methods both
/// hosts' `Design`/`Module`/`InstanceView` carry, spelled identically. The
/// Rust side is proven at compile time ([`call_every_rust_model_method`]);
/// the Python side is probed at runtime — both directions: a name in the
/// list missing from Python is `missing:<name>`, a public name Python
/// carries beyond the list is `extra:<name>` (so a method added to either
/// host without the other trips the guard).
const MODEL_DESIGN_METHODS: &[&str] = &["top", "module", "modules", "const_", "select"];
const MODEL_MODULE_METHODS: &[&str] = &[
    "name", "ports", "nets", "instances", "params", "behaviors", "op", "tran", "ac", "noise",
    "sens", "pss", "pz", "disto", "sp", "set", "compile",
];
const MODEL_INSTANCE_VIEW_METHODS: &[&str] = &[
    "label", "terminal_connections", "v", "i", "opvar", "opvars", "model", "terminals",
    "observables", "param", "params",
];

/// The uniform solver-config field surface (DVC-12 / MD-22): the fields both
/// hosts' solver config carries, spelled identically — the Rust
/// `SolverConfig` and the Python `Solver` dataclass.
const SOLVER_CONFIG_FIELDS: &[&str] = &[
    "temperature", "reltol", "abstol", "gmin", "max_iter", "dc_damp_tolerance", "checks",
    "require_scope",
];

const RLC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; param acmag: Real = 1.0; }
analog V { V(p, n) <+ dc + ac_stim(acmag, 0.0); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod L(inout p: Electrical, inout n: Electrical) { param l: Real = 1e-3; }
analog L { V(p, n) <- l * ddt(I(p, n)); }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire a   : Electrical;
    wire b   : Electrical;
    V1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = a) { .r = 10.0 };
    l1 : L(.p = a, .n = b) { .l = 1e-3 };
    c1 : C(.p = b, .n = gnd) { .c = 1e-6 };
}
";

/// Every name in [`ANALYSES`], called on the Rust `Session` — a compile-time
/// proof (not merely a runtime one) that the Rust host carries the full set.
fn call_every_rust_analysis(session: &mut Session) {
    let config = SolverConfig::default();
    let _ = session.op(&config, None);
    let _ = session.tran(1e-3, Some(1e-5), 0.0, &config, None, false, &[]);
    let _ = session.ac(1.0, 1e6, 5, true, &config);
    let _ = session.noise("a", "gnd", (1.0, 1e6), 5, true, &config);
    let _ = session.sens(&["a"], &[("r1".to_string(), "r".to_string())], 1e-6, &config);
    let _ = session.pss(1e-3, 0.0, &config);
    let _ = session.pz("V1", "b", None, &config);
    let _ = session.disto(1e6, None, 0.1, "a", None, &config);
    let _ = session.sp(1e3, 1e9, 5, true, &config);
    let _ = session.tf("a", None, "V1", &config);
    let _ = session.dc("V1", "dc", &[1.0, 2.0], &config, None);
}

/// Run a Python script (through the embedded facade) that builds a `Session`
/// on the same fixture and checks `hasattr` for every name in [`ANALYSES`],
/// writing the space-separated list of **missing** names to `out_path`
/// (empty file = full parity).
fn python_missing_analyses(out_path: &std::path::Path) {
    let phdl_path = std::env::temp_dir().join("piperine_host_parity_rlc.phdl");
    std::fs::write(&phdl_path, RLC_PHDL).expect("write phdl fixture");
    let names_py = ANALYSES.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>().join(", ");
    let script = format!(
        r#"
import piperine

def _probe():
    design = piperine.load({phdl:?})
    session = design.compile()
    names = [{names}]
    missing = [n for n in names if not hasattr(session, n)]
    with open({out:?}, "w") as f:
        f.write(" ".join(missing))

_probe()
"#,
        phdl = phdl_path.to_str().unwrap(),
        names = names_py,
        out = out_path.to_str().unwrap(),
    );
    let script_path = std::env::temp_dir().join("piperine_host_parity_script.py");
    std::fs::write(&script_path, script).expect("write probe script");
    piperine_python::embed::run_script(script_path.to_str().unwrap()).expect("python parity probe runs");
    let _ = std::fs::remove_file(&phdl_path);
    let _ = std::fs::remove_file(&script_path);
}

/// HOST-06's positive case: every analysis in [`ANALYSES`] exists on the
/// Rust `Session` (compile-time, see [`call_every_rust_analysis`]) AND the
/// Python `Session` (runtime `hasattr`) — both hosts, same names.
#[test]
fn host_parity_analyses_match_on_both_hosts() {
    let design = piperine_lang::parse_and_elaborate(RLC_PHDL, &SourceMap::dummy()).expect("RLC elaborates");
    let mut session =
        Session::builder(&design, "Top").disto(true).compile().expect("session compiles");
    call_every_rust_analysis(&mut session); // fails to COMPILE if Rust drifts

    let out = std::env::temp_dir().join("piperine_host_parity_missing.txt");
    python_missing_analyses(&out);
    let missing = std::fs::read_to_string(&out).expect("read missing-analyses probe output");
    let _ = std::fs::remove_file(&out);
    assert!(
        missing.trim().is_empty(),
        "Python Session is missing analyses the Rust Session has: {missing} (MD-22 breach)"
    );
}

/// HOST-06's negative case (the "fails loud on a synthetic drift" done-when
/// criterion): checking for a bogus, never-implemented analysis name is
/// reported as missing by the same mechanism — proving the parity probe
/// actually discriminates rather than vacuously reporting "all present".
#[test]
fn host_parity_probe_flags_a_synthetic_missing_analysis() {
    let bogus = "definitely_not_a_real_analysis_xyz";
    let out = std::env::temp_dir().join("piperine_host_parity_synthetic.txt");
    let phdl_path = std::env::temp_dir().join("piperine_host_parity_synthetic.phdl");
    std::fs::write(&phdl_path, RLC_PHDL).expect("write phdl fixture");
    let script = format!(
        r#"
import piperine

def _probe():
    design = piperine.load({phdl:?})
    session = design.compile()
    missing = [] if hasattr(session, {bogus:?}) else [{bogus:?}]
    with open({out:?}, "w") as f:
        f.write(" ".join(missing))

_probe()
"#,
        phdl = phdl_path.to_str().unwrap(),
        bogus = bogus,
        out = out.to_str().unwrap(),
    );
    let script_path = std::env::temp_dir().join("piperine_host_parity_synthetic_script.py");
    std::fs::write(&script_path, script).expect("write probe script");
    piperine_python::embed::run_script(script_path.to_str().unwrap()).expect("python synthetic probe runs");
    let missing = std::fs::read_to_string(&out).expect("read synthetic probe output");
    for p in [phdl_path, script_path, out] {
        let _ = std::fs::remove_file(p);
    }
    assert_eq!(missing.trim(), bogus, "the probe must flag a genuinely absent analysis, not pass silently");
}

/// Every name in the model lists, called on the api model through the root
/// crate — a compile-time proof (not merely a runtime one) that the Rust
/// host carries the full lifted object-model surface.
fn call_every_rust_model_method(design: &piperine::model::Design) {
    let _ = design.top();
    let module = design.module("Top").expect("Top present");
    let _ = design.modules();
    let _ = design.const_("NOT_A_CONST");
    let _ = design.select("/r1");

    let _ = module.name();
    let _ = module.ports();
    let _ = module.nets();
    let _ = module.instances();
    let _ = module.params();
    let _ = module.behaviors();
    let _ = module.op(None, None);
    let _ = module.tran(1e-3, Some(1e-5), 0.0, None, None, false, &[]);
    let _ = module.ac(1.0, 1e6, 5, true, None);
    let _ = module.noise("a", "gnd", (1.0, 1e6), 5, true, None);
    let _ = module.sens(&["a"], &[("r1".to_string(), "r".to_string())], 1e-6, None);
    let _ = module.pss(1e-3, 0.0, None);
    let _ = module.pz("V1", "b", None, None);
    let _ = module.disto(1e6, None, 0.1, "a", None, None);
    let _ = module.sp(1e3, 1e9, 5, true, None);
    module.set("r1", "r", 20.0);
    let _ = module.compile();

    let op = std::rc::Rc::new(module.op(None, None).expect("op solves"));
    let resolver = piperine::model::InstanceResolver::new(design.shared(), "Top".to_string());
    let view = piperine::model::InstanceView::new_op(op, resolver, "r1").expect("r1 binds");
    let _ = view.label();
    let _ = view.terminal_connections();
    let _ = view.v("p", None);
    let _ = view.i("p", Some("n"));
    let _ = view.opvar("g");
    let _ = view.opvars();
    let _ = view.model();
    let _ = view.terminals();
    let _ = view.observables();
    let _ = view.param("r");
    let _ = view.params();
}

/// Run a Python script (through the embedded facade) that reflects over the
/// native `_Design`/`_Module`/`_InstanceView` objects and writes the parity
/// gaps to `out_path`: `missing:<name>` for a listed method the native class
/// lacks, `extra:<name>` for a public method it carries beyond the list.
/// Empty file = full parity in both directions.
fn python_model_surface_gaps(out_path: &std::path::Path) {
    let phdl_path = std::env::temp_dir().join("piperine_host_parity_model.phdl");
    std::fs::write(&phdl_path, RLC_PHDL).expect("write phdl fixture");
    let design_names =
        MODEL_DESIGN_METHODS.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>().join(", ");
    let module_names =
        MODEL_MODULE_METHODS.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>().join(", ");
    let view_names =
        MODEL_INSTANCE_VIEW_METHODS.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>().join(", ");
    let script = format!(
        r#"
import piperine

def _probe():
    design = piperine.load({phdl:?})
    nd = design._native
    m = nd.module("Top")
    op = m.op()
    view = op["r1"]
    checks = [(nd, [{design_names}]), (m, [{module_names}]), (view, [{view_names}])]
    gaps = []
    for obj, names in checks:
        for n in names:
            if not hasattr(obj, n):
                gaps.append("missing:" + n)
        for n in dir(obj):
            if not n.startswith("_") and n not in names:
                gaps.append("extra:" + n)
    with open({out:?}, "w") as f:
        f.write(" ".join(gaps))

_probe()
"#,
        phdl = phdl_path.to_str().unwrap(),
        design_names = design_names,
        module_names = module_names,
        view_names = view_names,
        out = out_path.to_str().unwrap(),
    );
    let script_path = std::env::temp_dir().join("piperine_host_parity_model_script.py");
    std::fs::write(&script_path, script).expect("write probe script");
    piperine_python::embed::run_script(script_path.to_str().unwrap()).expect("python model probe runs");
    let _ = std::fs::remove_file(&phdl_path);
    let _ = std::fs::remove_file(&script_path);
}

/// CLA-19 (spec P1 story 5 AC4/AC5): the lifted object model carries the
/// same method names on both hosts. Rust is proven at compile time (see
/// [`call_every_rust_model_method`]); Python is probed at runtime in both
/// directions (`missing:` and `extra:`), so a method added to either host
/// without the other fails this test naming the drift.
#[test]
fn host_parity_model_surface_matches_on_both_hosts() {
    let design = piperine::model::Design::load_str(RLC_PHDL).expect("RLC elaborates");
    call_every_rust_model_method(&design); // fails to COMPILE if Rust drifts

    let out = std::env::temp_dir().join("piperine_host_parity_model_gaps.txt");
    python_model_surface_gaps(&out);
    let gaps = std::fs::read_to_string(&out).expect("read model-parity probe output");
    let _ = std::fs::remove_file(&out);
    assert!(
        gaps.trim().is_empty(),
        "the object-model surface drifted between hosts (MD-22 breach): {gaps}"
    );
}

/// Every name in [`SOLVER_CONFIG_FIELDS`], read on the Rust `SolverConfig`
/// — a compile-time proof (not merely a runtime one) that the Rust host
/// carries the full solver-config field set. The posture fields carry the
/// DVC-12 / D2 defaults (`Strict`, `EveryAnalysis`).
fn read_every_rust_solver_config_field() {
    let config = SolverConfig::default();
    let _ = config.temperature;
    let _ = config.reltol;
    let _ = config.abstol;
    let _ = config.gmin;
    let _ = config.max_iter;
    let _ = config.dc_damp_tolerance;
    assert_eq!(config.checks, piperine::prelude::CheckPosture::Strict, "DVC-12 default");
    assert_eq!(
        config.require_scope,
        piperine::prelude::RequireScope::EveryAnalysis,
        "D2 default"
    );
}

/// DVC-12 / MD-22: the solver-config field set is identical on both hosts —
/// the Rust `SolverConfig` (compile-time, see
/// [`read_every_rust_solver_config_field`]) and the Python `Solver`
/// dataclass (runtime `inspect.signature`, both directions: a field present
/// on one host but not the other trips the guard).
#[test]
fn host_parity_solver_config_fields_match_on_both_hosts() {
    read_every_rust_solver_config_field(); // fails to COMPILE if Rust drifts

    let names_py =
        SOLVER_CONFIG_FIELDS.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>().join(", ");
    let out = std::env::temp_dir().join("piperine_host_parity_solver_config.txt");
    let script = format!(
        r#"
import inspect
import piperine

def _probe():
    names = [{names}]
    params = set(inspect.signature(piperine.Solver).parameters.keys())
    gaps = []
    for n in names:
        if n not in params:
            gaps.append("missing:" + n)
    for n in params:
        if n not in names:
            gaps.append("extra:" + n)
    with open({out:?}, "w") as f:
        f.write(" ".join(gaps))

_probe()
"#,
        names = names_py,
        out = out.to_str().unwrap(),
    );
    let script_path = std::env::temp_dir().join("piperine_host_parity_solver_config_script.py");
    std::fs::write(&script_path, script).expect("write probe script");
    piperine_python::embed::run_script(script_path.to_str().unwrap()).expect("python config probe runs");
    let gaps = std::fs::read_to_string(&out).expect("read config-parity probe output");
    for p in [&out, &script_path] {
        let _ = std::fs::remove_file(p);
    }
    assert!(
        gaps.trim().is_empty(),
        "the solver-config field set drifted between hosts (MD-22 breach): {gaps}"
    );
}

// ─── DVC-18: the margins + findings surface (dv-core T24) ───────────────────

/// The divider fixture with one violated `require` — the both-hosts value
/// fixture: `dc = 1.0` across r1 + dut1 puts 0.5 V on dut1, so
/// `require head : V(p, n) <= hi` (hi = 0.4) has margin −0.1 at the
/// operating point and emits one `Error` finding in `collect`.
const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; param acmag: Real = 1.0; }
analog Vsrc { V(p, n) <+ dc + ac_stim(acmag, 0.0); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Dut(inout p: Electrical, inout n: Electrical) { param hi: Real = 0.4; }
analog Dut { I(p, n) <+ V(p, n) / 1e3; }
constraint Dut { require head : V(p, n) <= hi; }

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1   : Vsrc(.p = in,  .n = gnd);
    r1   : R(.p = in,     .n = out);
    dut1 : Dut(.p = out,  .n = gnd);
}
";

/// The uniform dv result surface (DVC-18 / MD-22): the members every
/// analysis result, margins channel, margin entry, and validation finding
/// carries, spelled identically on both hosts. The Rust side is proven at
/// compile time ([`read_every_rust_dv_surface`]); the Python side is
/// probed at runtime in both directions (`missing:`/`extra:`).
const OP_RESULT_METHODS: &[&str] =
    &["v", "i", "stats", "margins", "violations", "requires_ok", "first_violation"];
const TRACE_METHODS: &[&str] =
    &["v", "i", "axis", "stats", "margins", "violations", "requires_ok", "first_violation"];
const AC_TRACE_METHODS: &[&str] =
    &["v", "axis", "margins", "violations", "requires_ok", "first_violation"];
const MARGINS_RESULT_METHODS: &[&str] =
    &["get", "entries", "len", "is_empty", "requires_ok", "first_violation"];
const MARGIN_ENTRY_SURFACE: &[&str] =
    &["label", "instance_path", "kind", "shape", "scalar", "argmin", "is_satisfied"];
const VALIDATION_FINDING_FIELDS: &[&str] =
    &["severity", "label", "message", "value", "time", "instance_path"];

/// The collect posture both fixtures run under — the violated `require`
/// records instead of aborting.
fn collect_config() -> SolverConfig {
    SolverConfig { checks: piperine::prelude::CheckPosture::Collect, ..Default::default() }
}

fn divider_session() -> Session {
    let design =
        piperine_lang::parse_and_elaborate(DIVIDER_PHDL, &SourceMap::dummy()).expect("divider elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

/// Every name in the dv surface lists, read on the Rust host's result
/// objects — a compile-time proof (not merely a runtime one) that the
/// Rust host carries the full margins + findings surface.
fn read_every_rust_dv_surface() {
    let mut session = divider_session();
    let config = collect_config();
    let op = session.op(&config, None).expect("op solves");
    let _ = op.v("out");
    let _ = op.i(("in", "out"));
    let _ = op.stats();
    let margins = op.margins();
    let violations = op.violations();
    let _ = op.requires_ok();
    let _ = op.first_violation();

    let entry = margins.get("head").expect("head is in the channel");
    let _ = &entry.label;
    let _ = &entry.instance_path;
    let _ = entry.kind;
    let _ = entry.shape();
    let _ = entry.scalar();
    let _ = entry.argmin();
    let _ = entry.is_satisfied();
    let _ = margins.entries();
    let _ = margins.len();
    let _ = margins.is_empty();
    let _ = margins.requires_ok();
    let _ = margins.first_violation();

    let finding = violations.first().expect("the violated require recorded a finding");
    let _ = finding.severity;
    let _ = &finding.label;
    let _ = &finding.message;
    let _ = finding.value;
    let _ = finding.time;
    let _ = &finding.instance_path;

    let trace = session.tran(1e-6, Some(1e-7), 0.0, &config, None, false, &[]).expect("tran solves");
    let _ = trace.v("out");
    let _ = trace.i(("in", "out"));
    let _ = trace.axis();
    let _ = trace.stats();
    let _ = trace.margins();
    let _ = trace.violations();
    let _ = trace.requires_ok();
    let _ = trace.first_violation();

    let ac = session.ac(1.0, 1e3, 3, true, &config).expect("ac solves");
    let _ = ac.v("out");
    let _ = ac.axis();
    let _ = ac.margins();
    let _ = ac.violations();
    let _ = ac.requires_ok();
    let _ = ac.first_violation();
}

/// The Python side of the dv-surface probe: drive the divider fixture
/// through the facade and reflect over the native result objects, writing
/// `missing:<name>` / `extra:<name>` gaps to `out_path` (empty = parity).
fn python_dv_surface_gaps(out_path: &std::path::Path) {
    let phdl_path = std::env::temp_dir().join("piperine_host_parity_divider.phdl");
    std::fs::write(&phdl_path, DIVIDER_PHDL).expect("write phdl fixture");
    let lists = [
        ("op", OP_RESULT_METHODS),
        ("trace", TRACE_METHODS),
        ("ac", AC_TRACE_METHODS),
        ("margins", MARGINS_RESULT_METHODS),
        ("entry", MARGIN_ENTRY_SURFACE),
        ("finding", VALIDATION_FINDING_FIELDS),
    ];
    let checks_py = lists
        .iter()
        .map(|(obj, names)| {
            let names_py = names.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>().join(", ");
            format!("({obj}, [{names_py}])")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let script = format!(
        r#"
import piperine

def _probe():
    design = piperine.load({phdl:?})
    session = design.compile()
    solver = piperine.Solver(checks="collect")
    op = session.op(piperine.OpConfig(solver=solver))
    trace = session.tran(piperine.TranConfig(stop=1e-6, step=1e-7, solver=solver))
    ac = session.ac(piperine.AcConfig(fstart=1.0, fstop=1e3, points=3, solver=solver))
    margins = op.margins()
    entry = margins.get("head")
    finding = op.violations()[0]
    gaps = []
    for obj, names in [{checks}]:
        for n in names:
            if not hasattr(obj, n):
                gaps.append("missing:" + n)
        for n in dir(obj):
            if not n.startswith("_") and n not in names:
                gaps.append("extra:" + n)
    with open({out:?}, "w") as f:
        f.write(" ".join(gaps))

_probe()
"#,
        phdl = phdl_path.to_str().unwrap(),
        checks = checks_py,
        out = out_path.to_str().unwrap(),
    );
    let script_path = std::env::temp_dir().join("piperine_host_parity_dv_script.py");
    std::fs::write(&script_path, script).expect("write probe script");
    piperine_python::embed::run_script(script_path.to_str().unwrap()).expect("python dv probe runs");
    let _ = std::fs::remove_file(&phdl_path);
    let _ = std::fs::remove_file(&script_path);
}

/// DVC-18 / MD-22: the margins + findings surface is identical on both
/// hosts — every member name exists on both, and neither host carries a
/// public name the other lacks. Rust is proven at compile time (see
/// [`read_every_rust_dv_surface`]); Python is probed at runtime in both
/// directions, so a member added to either host without the other fails
/// this test naming the drift.
#[test]
fn host_parity_dv_surface_matches_on_both_hosts() {
    read_every_rust_dv_surface(); // fails to COMPILE if Rust drifts

    let out = std::env::temp_dir().join("piperine_host_parity_dv_gaps.txt");
    python_dv_surface_gaps(&out);
    let gaps = std::fs::read_to_string(&out).expect("read dv-parity probe output");
    let _ = std::fs::remove_file(&out);
    assert!(
        gaps.trim().is_empty(),
        "the margins/findings surface drifted between hosts (MD-22 breach): {gaps}"
    );
}

/// DVC-18's independent test: one fixture driven through both hosts
/// asserts identical margins, argmins, and violations. The Python side
/// writes its reads as `key=value` lines; the Rust side asserts each
/// against its own reads of the same fixture.
#[test]
fn host_parity_dv_values_match_on_both_hosts() {
    let mut session = divider_session();
    let op = session.op(&collect_config(), None).expect("op solves");

    let phdl_path = std::env::temp_dir().join("piperine_host_parity_dv_values.phdl");
    std::fs::write(&phdl_path, DIVIDER_PHDL).expect("write phdl fixture");
    let out = std::env::temp_dir().join("piperine_host_parity_dv_values.txt");
    let script = format!(
        r#"
import piperine

def _probe():
    design = piperine.load({phdl:?})
    session = design.compile()
    op = session.op(piperine.OpConfig(solver=piperine.Solver(checks="collect")))
    m = op.margins().get("head")
    v = op.violations()
    fv = op.first_violation()
    lines = [
        "scalar=" + repr(m.scalar()),
        "argmin=" + repr(m.argmin()),
        "kind=" + m.kind,
        "shape=" + m.shape,
        "instance_path=" + m.instance_path,
        "is_satisfied=" + repr(m.is_satisfied()),
        "requires_ok=" + repr(op.requires_ok()),
        "first_violation=" + fv.label,
        "violations=" + repr(len(v)),
        "severity=" + v[0].severity,
        "label=" + v[0].label,
        "value=" + repr(v[0].value),
        "time=" + repr(v[0].time),
        "finding_instance_path=" + v[0].instance_path,
        "message=" + v[0].message,
    ]
    with open({out:?}, "w") as f:
        f.write("\n".join(lines))

_probe()
"#,
        phdl = phdl_path.to_str().unwrap(),
        out = out.to_str().unwrap(),
    );
    let script_path = std::env::temp_dir().join("piperine_host_parity_dv_values_script.py");
    std::fs::write(&script_path, script).expect("write probe script");
    piperine_python::embed::run_script(script_path.to_str().unwrap()).expect("python dv values probe runs");
    for p in [&phdl_path, &script_path] {
        let _ = std::fs::remove_file(p);
    }
    let python: std::collections::HashMap<String, String> = std::fs::read_to_string(&out)
        .expect("read dv values probe output")
        .lines()
        .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect();
    let _ = std::fs::remove_file(&out);

    let py_bool = |b: bool| if b { "True" } else { "False" };
    let entry = op.margins().get("head").expect("head is in the channel");
    let finding = &op.violations()[0];
    let rust: Vec<(&str, String)> = vec![
        ("scalar", format!("{:?}", entry.scalar())),
        ("argmin", format!("{:?}", entry.argmin().expect("pointwise carries its argmin"))),
        ("kind", "require".to_string()),
        ("shape", "pointwise".to_string()),
        ("instance_path", entry.instance_path.clone()),
        ("is_satisfied", py_bool(entry.is_satisfied()).to_string()),
        ("requires_ok", py_bool(op.requires_ok()).to_string()),
        ("first_violation", op.first_violation().expect("a violation").label.clone()),
        ("violations", format!("{:?}", op.violations().len())),
        ("severity", "error".to_string()),
        ("label", finding.label.clone()),
        ("value", format!("{:?}", finding.value)),
        ("time", format!("{:?}", finding.time)),
        ("finding_instance_path", finding.instance_path.clone()),
        ("message", finding.message.clone()),
    ];
    for (key, want) in rust {
        let got = python.get(key).unwrap_or_else(|| panic!("python probe missing key {key}"));
        assert_eq!(*got, want, "dv value `{key}` differs between hosts (MD-22 breach)");
    }
}
