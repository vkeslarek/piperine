//! `$limit` limiter formulas — the JIT-emitted `fetlim`/`limvds` must match
//! the ngspice C reference (devsup.c) value-for-value (SC-19).

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;

/// ngspice `DEVlimvds(vnew, vold)` — the reference (devsup.c).
fn ref_limvds(vnew: f64, vold: f64) -> f64 {
    let mut vnew = vnew;
    if vold >= 3.5 {
        if vnew > vold {
            vnew = vnew.min(3.0 * vold + 2.0);
        } else if vnew < 3.5 {
            vnew = vnew.max(2.0);
        }
    } else if vnew > vold {
        vnew = vnew.min(4.0);
    } else {
        vnew = vnew.max(-0.5);
    }
    vnew
}

/// ngspice `DEVfetlim(vnew, vold, vto)` — the reference (devsup.c).
fn ref_fetlim(vnew: f64, vold: f64, vto: f64) -> f64 {
    let vtsthi = (2.0 * (vold - vto)).abs() + 2.0;
    let vtstlo = (vold - vto).abs() + 1.0;
    let vtox = vto + 3.5;
    let delv = vnew - vold;
    let mut vnew = vnew;
    if vold >= vto {
        if vold >= vtox {
            if delv <= 0.0 {
                if vnew >= vtox {
                    if -delv > vtstlo {
                        vnew = vold - vtstlo;
                    }
                } else {
                    vnew = vnew.max(vto + 2.0);
                }
            } else if delv >= vtsthi {
                vnew = vold + vtsthi;
            }
        } else if delv <= 0.0 {
            vnew = vnew.max(vto - 0.5);
        } else {
            vnew = vnew.min(vto + 4.0);
        }
    } else if delv <= 0.0 {
        if -delv > vtsthi {
            vnew = vold - vtsthi;
        }
    } else {
        let vtemp = vto + 0.5;
        if vnew <= vtemp {
            if delv > vtstlo {
                vnew = vold + vtstlo;
            }
        } else {
            vnew = vtemp;
        }
    }
    vnew
}

/// Compile a one-limiter device and return `eval_limit_update(vnew | vold)`.
/// The device forces `$limit(kind, V(d,s), 0, vto, 0)` so `vnew = V(d,s)` is
/// controlled directly by the terminal voltages and `vold` seeds a state slot.
fn jit_limit(kind: &str, vnew: f64, vold: f64, vto: f64) -> f64 {
    let src = format!(
        "discipline Electrical {{ potential v: Real; flow i: Real; }}
mod L (inout d: Electrical, inout s: Electrical) {{ param vto: Real = 1.0; }}
analog L {{ I(d, s) <+ $limit(\"{kind}\", V(d, s), 0.0, vto, 0.0); }}"
    );
    let elab = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .expect("PHDL parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    let kernel = AnalogKernel::compile(&bodies["L"]).expect("compile limiter device");

    assert_eq!(kernel.num_limits(), 1, "exactly one $limit slot");
    let mut state = vec![0.0; kernel.num_state_slots()];
    state[kernel.limit_base()] = vold;
    let params = vec![vto];
    let vars = vec![0.0; kernel.num_vars()];
    // Terminal order = port order (d, s); V(d,s) = volts[0] - volts[1].
    let volts = vec![vnew, 0.0];
    let sim = SimCtx::default();
    let mut out = vec![0.0; 1];
    kernel.eval_limit_update(&volts, &params, &state, &vars, &sim, &mut out);
    out[0]
}

#[test]
fn limvds_matches_ngspice_reference() {
    // Cover every branch: vold ≥ 3.5 (rising clamp / falling floor / pass),
    // vold < 3.5 (rising clamp to 4 / falling floor at −0.5).
    let cases = [
        (20.0, 4.0),   // hi, rising, clamp bites at 3·4+2 = 14
        (10.0, 4.0),   // hi, rising, under the clamp, pass
        (3.0, 4.0),    // hi, falling below 3.5, above the floor, pass
        (1.0, 4.0),    // hi, falling below 3.5, floor bites at 2
        (3.6, 4.0),    // hi, falling but ≥ 3.5, pass
        (5.0, 1.0),    // lo, rising, clamp to 4
        (3.0, 1.0),    // lo, rising, below clamp, pass
        (-2.0, 1.0),   // lo, falling, floor to −0.5
        (2.0, 2.5),    // lo, falling, above floor, pass
    ];
    for (vnew, vold) in cases {
        let got = jit_limit("limvds", vnew, vold, 1.0);
        let want = ref_limvds(vnew, vold);
        assert!(
            (got - want).abs() < 1e-9,
            "limvds(vnew={vnew}, vold={vold}): jit={got} ref={want}"
        );
    }
}

#[test]
fn fetlim_matches_ngspice_reference() {
    let vto = 1.0;
    // Cover on/high, on/mid, and off regions with rising & falling steps.
    let cases = [
        (20.0, 5.0),   // on, high, staying on, clamp bites at vold+vtsthi = 15
        (10.0, 5.0),   // on, high, staying on, under vtsthi, pass
        (4.6, 5.0),    // on, high, going off, vnew ≥ vtox, small step, pass
        (2.0, 5.0),    // on, high, going off, vnew < vtox, floor at vto+2
        (5.0, 3.0),    // on, mid, increasing, clamp at vto+4
        (-1.0, 3.0),   // on, mid, decreasing, floor at vto−0.5
        (-5.0, 0.5),   // off, decreasing big step, clamp at vold−vtsthi
        (0.0, 0.5),    // off, decreasing small step, pass
        (3.0, 0.5),    // off, increasing above vto+0.5, clamp at vto+0.5
        (1.2, 0.5),    // off, increasing below vto+0.5, small step, pass
    ];
    for (vnew, vold) in cases {
        let got = jit_limit("fetlim", vnew, vold, vto);
        let want = ref_fetlim(vnew, vold, vto);
        assert!(
            (got - want).abs() < 1e-9,
            "fetlim(vnew={vnew}, vold={vold}, vto={vto}): jit={got} ref={want}"
        );
    }
}
