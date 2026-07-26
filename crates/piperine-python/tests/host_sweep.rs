//! host-library T20 (HOST-18): Python `session.sweep(label, param, values)`
//! → `for point in sweep: ...` yields `SweepPoint`s, each a `Session` view
//! (attribute delegation) that runs any analysis. Same divider fixture as
//! the Rust `tests/session_sweep.rs` (`piperine-codegen/tests/live_params.rs`'s
//! presence-flipping oracle): `r2.ns` is `Real?` never given at build, so
//! writing it at all is a structural rebuild (LIVE-14) — the sweep's first
//! point rebuilds once; the rest restamp.
//!
//! Expected values come from an independent ground-truth path: a **fresh**
//! `design.compile()` per point with `ns` supplied directly in the source,
//! never touching `Sweep`/`SweepPoint`.

use piperine_python::embed::run_script;

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

fn divider_phdl(r2_override: &str) -> String {
    format!(
        "\
discipline Electrical {{ potential v: Real; flow i: Real; }}

mod G(inout p: Electrical, inout n: Electrical) {{
    param g: Real = 1.0e-3;
    param ns: Real? = none;
}}
analog G {{ I(p, n) <+ (g + ns.get_or(0.0) * 1.0e-3) * V(p, n); }}

mod Vsrc(inout p: Electrical, inout n: Electrical) {{
    param dc: Real = 10.0;
}}
analog Vsrc {{ V(p, n) <- dc; }}

mod Top() {{
    wire gnd : Electrical;
    wire top : Electrical;
    wire mid : Electrical;
    v1 : Vsrc(.p = top, .n = gnd) {{}};
    r1 : G(.p = top, .n = mid) {{}};
    r2 : G(.p = mid, .n = gnd) {{ {r2_override} }};
}}
"
    )
}

/// HOST-18: iterating `session.sweep("r2", "ns", [1.0, 2.0, 3.0])` yields
/// three `SweepPoint`s with the right `.index`/`.value`, `session.rebuilds`
/// goes 0 -> 1 (the presence flip on the first point, then plain restamps),
/// and every point's `.op().v("mid")` matches an independent fresh
/// `design.compile()` with `ns` given directly in the source.
#[test]
fn python_sweep_rebuilds_once_and_matches_fresh_builds() {
    let never_given = write_temp("piperine_host_sweep_never_given.phdl", &divider_phdl(""));
    let given = |ns: f64| divider_phdl(&format!(".ns = {ns}"));
    let fresh_paths: Vec<String> = [1.0_f64, 2.0, 3.0]
        .iter()
        .enumerate()
        .map(|(i, &ns)| write_temp(&format!("piperine_host_sweep_fresh_{i}.phdl"), &given(ns)))
        .collect();
    let fresh_paths_py = fresh_paths.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>().join(", ");

    let script = format!(
        r#"import piperine

design = piperine.load({never_given:?})
s = design.compile()
assert s.rebuilds == 0

values = [1.0, 2.0, 3.0]
fresh_paths = [{fresh_paths_py}]
seen = []
sweep = s.sweep("r2", "ns", values)
assert len(sweep) == 3
for point in sweep:
    assert isinstance(point, piperine.SweepPoint)
    assert point.value in values
    mid = point.op().v("mid")
    seen.append((point.index, point.value, mid))

assert len(seen) == 3
assert [idx for idx, _, _ in seen] == [0, 1, 2]
assert s.rebuilds == 1, s.rebuilds

for i, (idx, value, mid) in enumerate(seen):
    fresh = piperine.load(fresh_paths[i]).compile()
    expected = fresh.op().v("mid")
    rel_err = abs(mid - expected) / expected
    assert rel_err < 1e-9, (i, value, mid, expected, rel_err)
"#
    );
    let script_path = write_temp("piperine_host_sweep_script.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "sweep script must pass: {:?}", result.err());

    let _ = std::fs::remove_file(&never_given);
    let _ = std::fs::remove_file(&script_path);
    for i in 0..3 {
        let _ = std::fs::remove_file(std::env::temp_dir().join(format!("piperine_host_sweep_fresh_{i}.phdl")));
    }
}
