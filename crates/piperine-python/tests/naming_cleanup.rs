//! T26 — Naming cleanup + `__len__` + properties (HOST-24). Spec-derived
//! from tasks.md's "Done when": `design["amp"]`, `design.top` (prop),
//! `amp.ports` (prop), `pip.load_str`, `len(wf)`; `const` replaces
//! `const_`; property-vs-method consistent.

use piperine_python::embed::run_script;

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

const RC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 5.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    v1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = out) { .r = 1e3 };
}
";

/// `design["amp"]` (`__getitem__`), `design.top` (property, no parens),
/// `amp.ports`/`.nets`/`.instances`/`.params`/`.behaviors` (properties),
/// `const` replaces `const_`, `pip.load_str`, and `len(wf)`.
#[test]
fn naming_cleanup_and_properties() {
    let phdl = write_temp("piperine_t26_rc.phdl", RC_PHDL);
    let script = format!(
        r#"
import piperine as pip

design = pip.load("{phdl}")

# ── design["amp"] (__getitem__) ──
top_via_index = design["Top"]
assert type(top_via_index).__name__ == "Module"
top_via_method = design.module("Top")
assert top_via_index.name == top_via_method.name == "Top"

# unknown name still raises UnknownModule through __getitem__
raised = None
try:
    design["DoesNotExist"]
except pip.UnknownModule as e:
    raised = type(e).__name__
assert raised == "UnknownModule", raised

# ── design.top is a property (no parens) ──
top_prop = design.top
assert top_prop is not None
assert top_prop.name == "Top"

# ── module reflection accessors are properties ──
amp = design["Top"]
assert isinstance(amp.ports, list)
assert isinstance(amp.nets, list)
assert isinstance(amp.instances, list)
assert isinstance(amp.params, list)
assert isinstance(amp.behaviors, list)
assert len(amp.instances) == 2, [i.label for i in amp.instances]

# ── const (not const_) ──
assert not hasattr(pip.Design, "const_"), "const_ must be gone from the facade"
assert design.const("does_not_exist") is None

# ── pip.load_str ──
design2 = pip.load_str(open("{phdl}").read())
assert design2["Top"].name == "Top"

# ── len(wf) ──
op = amp.op()
trace = amp.tran(pip.TranConfig(stop=1e-3, step=1e-5))
wf = trace.v("out")
assert len(wf) == len(wf.values)
assert len(wf) > 0
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_t26_naming.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "naming cleanup script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}
