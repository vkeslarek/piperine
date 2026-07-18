//! BRM-12/14 — facade hygiene gate: every public class and method on the
//! Python surface carries a docstring, no bench-era vocabulary leaks into
//! the public docs, and the facade/native surfaces are in parity (every
//! native public class is surfaced under its public name; every facade
//! export resolves).

use piperine_python::embed::run_script;

#[test]
fn facade_is_fully_documented_and_in_parity() {
    let script = r#"
import inspect
import _piperine
import piperine

failures = []

# ── docstring walk: every facade export + public member is documented ──
def check_doc(owner, name, obj):
    if obj is None:
        failures.append(f"{owner}.{name}: unresolvable")
        return
    doc = inspect.getdoc(obj) or ""
    if not doc.strip():
        failures.append(f"{owner}.{name}: missing docstring")

for name in piperine.__all__:
    obj = getattr(piperine, name, None)
    check_doc("piperine", name, obj)
    if not inspect.isclass(obj):
        continue
    for mname in dir(obj):
        if mname.startswith("__"):
            continue
        member = getattr(obj, mname, None)
        if inspect.isroutine(member) or isinstance(member, property):
            check_doc(name, mname, member.fget if isinstance(member, property) else member)

# ── no bench-era vocabulary in public names or docs ──
for name in piperine.__all__:
    lowered = name.lower()
    if "bench" in lowered or "stage" in lowered:
        failures.append(f"piperine.{name}: bench-era name")
    doc = (inspect.getdoc(getattr(piperine, name)) or "").lower()
    if "bench block" in doc or "$op" in doc or "$tran" in doc or "$ac(" in doc:
        failures.append(f"piperine.{name}: bench-era doc leakage")

# ── parity: every native public class surfaces under its public name ──
for n in dir(_piperine):
    if n.startswith("__"):
        continue
    if n.startswith("_"):
        public = n.lstrip("_")
        if not hasattr(piperine, public):
            failures.append(f"native {n} not surfaced as piperine.{public}")
    else:
        if not hasattr(piperine, n):
            failures.append(f"native function {n} not surfaced on piperine")

# ── parity: every facade wrapper method forwards to a real native method ──
for facade_cls, native_cls in [("Design", "_Design"), ("Module", "_Module"), ("LiveSession", "_LiveSession")]:
    f = getattr(piperine, facade_cls)
    n = getattr(_piperine, native_cls)
    for mname in dir(f):
        if mname.startswith("_"):
            continue
        member = getattr(f, mname)
        if not inspect.isfunction(member):
            continue
        if mname == "compile" and facade_cls == "Design":
            continue  # facade-level convenience over Module.compile
        if not hasattr(n, mname):
            failures.append(f"{facade_cls}.{mname} has no native {native_cls}.{mname}")

assert not failures, "facade hygiene:\n" + "\n".join(failures)
print("facade hygiene: PASS")
"#;
    run_script_string(script);
}

/// Run an inline script through the embedded host (facade + native module
/// registered), failing with the python traceback on error.
fn run_script_string(script: &str) {
    let path = std::env::temp_dir().join("piperine_facade_hygiene.py");
    std::fs::write(&path, script).expect("write probe script");
    run_script(path.to_str().expect("utf8 path")).expect("facade hygiene script");
}
