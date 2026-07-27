//! Scope: `piperine_api::model::InstanceView` (CLA-17) — the full per-instance
//! surface: `terminal_connections`, `v`/`i` over the connected nets (scalars
//! over an op result, waveforms over a trace), `opvar`/`opvars`, and the
//! static catalogs (`model`/`terminals`/`observables`/`param`/`params`), plus
//! the fail-loud paths (unknown label/port, readouts on an introspection-only
//! view, op-side accessors on a trace-bound view).

use std::rc::Rc;

use piperine_api::model::{Design, InstanceReadout, InstanceResolver, InstanceView};
use piperine_api::{Error, OpResult, Trace};

/// The introspect divider: `r_bot` computes opvar `cond = 1/r` (named via
/// `@name`), so both the opvar snapshot and the observable catalog are
/// non-empty; `r_top` is a plain resistor, because `OpResult::i` recomputes
/// the branch from the kernel with an empty var bank and cannot yet read a
/// var-bearing device (pre-existing gap — the v/i assertions stay on the
/// plain device). `mid = 5·2k/(3k+2k) = 2.0 V`; the drop over `r_top` is
/// 3.0 V and its branch carries 1 mA.
const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod OpvarResistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
    @name(value = \"cond\") var g : Real = 0.0;
}
analog OpvarResistor {
    g = 1.0 / r;
    I(p, n) <+ g * V(p, n);
}

mod Top() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 3e3 };
    r_bot : OpvarResistor (.p = mid, .n = gnd) { .r = 2e3 };
}
";

fn fixture() -> Design {
    Design::load_str(DIVIDER_PHDL).expect("divider elaborates")
}

fn resolver(design: &Design) -> InstanceResolver {
    InstanceResolver::new(design.shared(), "Top".to_string())
}

fn op_view(design: &Design, op: Rc<OpResult>, label: &str) -> InstanceView {
    InstanceView::new_op(op, resolver(design), label).expect("labeled instance binds")
}

fn scalar(readout: InstanceReadout) -> f64 {
    match readout {
        InstanceReadout::Scalar(v) => v,
        other => panic!("expected a scalar over an op result, got {other:?}"),
    }
}

#[test]
fn terminal_connections_map_ports_to_parent_nets_in_declaration_order() {
    let design = fixture();
    let op = Rc::new(design.module("Top").expect("Top").op(None, None).expect("op solves"));
    let view = op_view(&design, op, "r_top");
    let connections = view.terminal_connections().expect("bound view resolves connectivity");
    let pairs: Vec<(&str, &str)> = connections.iter().map(|t| (t.port(), t.net())).collect();
    assert_eq!(pairs, vec![("p", "vin"), ("n", "mid")], "r_top's ports wire to vin/mid as authored");
}

#[test]
fn v_and_i_read_scalars_over_an_op_result() {
    let design = fixture();
    let op = Rc::new(design.module("Top").expect("Top").op(None, None).expect("op solves"));
    let view = op_view(&design, op, "r_top");

    let vp = scalar(view.v("p", None).expect("v(p)"));
    assert!((vp - 5.0).abs() < 1e-9, "port p sits on vin = 5.0 V, got {vp}");

    let vpn = scalar(view.v("p", Some("n")).expect("v(p, n)"));
    assert!((vpn - 3.0).abs() < 1e-9, "the drop over r_top is 5.0 − 2.0 = 3.0 V, got {vpn}");

    let ipn = scalar(view.i("p", Some("n")).expect("i(p, n)"));
    assert!((ipn - 1e-3).abs() < 1e-12, "3.0 V over 3 kΩ carries 1 mA, got {ipn}");
}

#[test]
fn opvar_and_the_static_catalogs_reflect_the_device() {
    let design = fixture();
    let op = Rc::new(design.module("Top").expect("Top").op(None, None).expect("op solves"));
    let view = op_view(&design, op, "r_bot");

    let g = view.opvar("cond").expect("r_bot declares opvar cond");
    assert!((g - 1.0 / 2e3).abs() < 1e-12, "cond = 1/r_bot = 1/2e3, got {g}");
    assert_eq!(view.opvars().len(), 1, "OpvarResistor declares exactly one opvar (cond)");

    let model = view.model();
    assert_eq!(model.type_id, "OpvarResistor", "no @model => type_id falls back to the module name");
    assert_eq!(model.version, "", "no @model => version is empty");

    let terminals = view.terminals();
    let names: Vec<&str> = terminals.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"p") && names.contains(&"n"), "terminals p and n present: {names:?}");

    let observables = view.observables();
    assert!(
        observables.iter().any(|o| o.name == "cond"),
        "the @name(value = \"cond\") var surfaces as observable `cond`: {observables:?}"
    );

    let r = view.param("r").expect("r_top declares param r");
    assert_eq!(r.name, "r");
    assert!(view.params().iter().any(|p| p.name == "r"), "the full catalog carries r");
}

#[test]
fn trace_bound_view_returns_waveforms_and_guards_the_op_side() {
    let design = fixture();
    let module = design.module("Top").expect("Top");
    let trace: Rc<Trace> =
        Rc::new(module.tran(5e-3, Some(1e-5), 0.0, None, None, false, &[]).expect("tran runs"));
    let view = InstanceView::new_trace(trace, resolver(&design), "r_top");

    let waveform = match view.v("p", None).expect("v(p) over a trace") {
        InstanceReadout::Waveform(w) => w,
        other => panic!("expected a waveform over a trace, got {other:?}"),
    };
    assert!(!waveform.is_empty(), "the trace recorded points");
    let first = waveform.points()[0].1;
    assert!((first - 5.0).abs() < 1e-9, "vin sits at 5.0 V for the whole run, got {first}");

    let err = view.opvar("cond").expect_err("opvar on a trace view must fail loud");
    assert!(err.to_string().contains("not available on a trace view"), "got: {err}");
}

#[test]
fn introspection_only_view_fails_loud_on_terminal_readouts() {
    let design = fixture();
    let op = design.module("Top").expect("Top").op(None, None).expect("op solves");
    let view = op.instance("r_bot").expect("r_bot is a labeled instance");

    // The introspection surface keeps working…
    assert!((view.opvar("cond").expect("cond") - 1.0 / 2e3).abs() < 1e-12);

    // …but terminal readouts and connectivity need a bound view.
    let err = view.v("p", None).expect_err("v on an introspection-only view must fail loud");
    assert!(err.to_string().contains("introspection-only"), "got: {err}");
    let err = view.terminal_connections().expect_err("connectivity must fail loud");
    assert!(err.to_string().contains("introspection-only"), "got: {err}");
}

#[test]
fn unknown_label_and_unknown_port_fail_loud() {
    let design = fixture();
    let op = Rc::new(design.module("Top").expect("Top").op(None, None).expect("op solves"));

    let err = InstanceView::new_op(op.clone(), resolver(&design), "ghost")
        .expect_err("an unknown label must fail loud");
    assert!(err.to_string().contains("ghost"), "the diagnostic names the label: {err}");

    let view = op_view(&design, op, "r_top");
    let err = view.v("bogus", None).expect_err("an unknown port must fail loud");
    assert!(matches!(err, Error::NotFound(_)), "an unknown port is a lookup miss, got {err:?}");
    assert!(err.to_string().contains("bogus"), "the diagnostic names the port: {err}");
}
