//! host-library T21 (HOST-19): Python `session.sweep_grid({"r1.r": [...],
//! "r2.r": [...]})` -> `Grid`, iterating every combination; `grid.map(fn)`
//! -> an axis-shaped `numpy.ndarray`. Same divider fixture as
//! `tests/session_sweep_grid.rs` (Rust side) — `mid = 10·r2/(r1+r2)`, a
//! closed form evaluated directly in the script as ground truth.

use piperine_python::embed::run_script;

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 10.0; }
analog Vsrc { V(p, n) <- dc; }

mod Top() {
    wire gnd : Electrical;
    wire top : Electrical;
    wire mid : Electrical;
    v1 : Vsrc(.p = top, .n = gnd) {};
    r1 : R(.p = top, .n = mid) {};
    r2 : R(.p = mid, .n = gnd) {};
}
";

/// HOST-19 AC2/AC3: `session.sweep_grid({"r1.r": [...], "r2.r": [...]})`
/// iterates every combination as a `SweepPoint` with a `.index` tuple
/// matching row-major order, and `grid.map(lambda p: p.op().v("mid"))`
/// returns a `numpy.ndarray` shaped `(len(r1_values), len(r2_values))`
/// whose entries match the closed-form divider voltage.
#[test]
fn python_grid_iterates_and_maps_to_a_shaped_ndarray() {
    let phdl = write_temp("piperine_host_sweep_grid_rc.phdl", DIVIDER_PHDL);
    let script = format!(
        r#"import numpy as np
import piperine

design = piperine.load("{phdl}")
s = design.compile()

r1_values = [1e3, 2e3]
r2_values = [1e3, 3e3, 5e3]
grid = s.sweep_grid({{"r1.r": r1_values, "r2.r": r2_values}})
assert grid.shape == (2, 3), grid.shape
assert len(grid) == 6

# __iter__: every combination visited, index in row-major order.
seen_index = [point.index for point in grid]
expected_index = [(i, j) for i in range(2) for j in range(3)]
assert seen_index == expected_index, seen_index

# map(): axis-shaped ndarray matching the closed-form divider voltage.
arr = grid.map(lambda p: p.op().v("mid"))
assert isinstance(arr, np.ndarray)
assert arr.shape == (2, 3), arr.shape
for i, r1 in enumerate(r1_values):
    for j, r2 in enumerate(r2_values):
        expected = 10.0 * r2 / (r1 + r2)
        rel_err = abs(arr[i, j] - expected) / expected
        assert rel_err < 1e-9, (i, j, arr[i, j], expected, rel_err)
"#
    );
    let script_path = write_temp("piperine_host_sweep_grid_script.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "sweep_grid script must pass: {:?}", result.err());

    let _ = std::fs::remove_file(&phdl);
    let _ = std::fs::remove_file(&script_path);
}
