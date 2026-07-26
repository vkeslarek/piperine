//! T23 — SI unit helpers (HOST-21). Spec-derived from tasks.md's "Done
//! when": `pip.Hz("10M") == 1e7`; raw floats do NOT string-parse; garbage
//! fails loud.

use piperine_python::embed::run_script;

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

#[test]
fn hz_parses_si_suffixed_strings() {
    let script = r#"
import piperine as pip

assert pip.Hz("10M") == 1e7, pip.Hz("10M")
assert pip.Hz("10MHz") == 1e7, pip.Hz("10MHz")
assert pip.Hz("1kHz") == 1e3, pip.Hz("1kHz")
assert pip.Hz(1e6) == 1e6, pip.Hz(1e6)
"#;
    let script_path = write_temp("piperine_t23_hz.py", script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "Hz script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}

/// Raw floats do NOT string-parse (spec edge case): `Hz(1e6)` is exactly
/// `1e6`, never scaled/reinterpreted the way a string would be.
#[test]
fn raw_floats_do_not_string_parse() {
    let script = r#"
import piperine as pip

# A float that would parse very differently as a string ("1e6" has no SI
# suffix so a string-parse would also give 1e6, so use a value where a
# string reading WOULD differ: the digit "1" is not "M"/"k"/etc, so the
# float path must not attempt to strip trailing digits as if they were an
# SI prefix character.
assert pip.Hz(5.0) == 5.0
assert pip.ns(5.0) == 5e-9
assert pip.mV(5.0) == 5e-3
"#;
    let script_path = write_temp("piperine_t23_raw_float.py", script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "raw float script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}

#[test]
fn ns_mv_c_helpers_convert_to_si_base_units() {
    let script = r#"
import piperine as pip

assert pip.ns(10) == 10e-9, pip.ns(10)
assert pip.ns("10n") == 10e-9, pip.ns("10n")
assert pip.mV(300) == 0.3, pip.mV(300)
assert pip.C(27) == 300.15, pip.C(27)
"#;
    let script_path = write_temp("piperine_t23_helpers.py", script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "helpers script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}

/// Garbage input fails loud (`ValueError`), never a silent `0.0`/`NaN`.
#[test]
fn hz_garbage_string_fails_loud() {
    let script = r#"
import piperine as pip

try:
    pip.Hz("banana")
    raised = False
except ValueError:
    raised = True
assert raised, "Hz('banana') must raise ValueError"
"#;
    let script_path = write_temp("piperine_t23_hz_garbage.py", script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "garbage script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}
