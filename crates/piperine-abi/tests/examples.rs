//! The worked examples of abi.md §11.1 to §11.6 as fixtures (feature
//! `solver-abi-core`, SAC-11, SAC-09 AC 2): each device, called through
//! `module::<D>` and `caller::Handle`, produces the values and partials its
//! text states at one hand-computed point, the flip-flop and the comparator
//! commit what they state, every shape passes `validate`, and every function
//! with a partial passes the finite-difference verifier.

#[path = "support/examples.rs"]
mod examples;

use std::sync::OnceLock;

use examples::{Capacitor, Comparator, Dac, Diode, FlipFlop, Resistor, module_of};
use piperine_abi::caller::{Commit, Handle};
use piperine_abi::device::{Access, Flow, cell_lens};
use piperine_abi::layout::{HIGH, LOW};
use piperine_abi::raw::{Module, status};
use piperine_abi::validate::validate;
use piperine_abi::verify::{Verdict, verify_partials};

static M_RESISTOR: OnceLock<Module> = OnceLock::new();
static M_DIODE: OnceLock<Module> = OnceLock::new();
static M_CAPACITOR: OnceLock<Module> = OnceLock::new();
static M_FLIP_FLOP: OnceLock<Module> = OnceLock::new();
static M_COMPARATOR: OnceLock<Module> = OnceLock::new();
static M_DAC: OnceLock<Module> = OnceLock::new();

fn modules() -> [&'static Module; 6] {
    [
        module_of::<Resistor>(&M_RESISTOR, "Resistor"),
        module_of::<Diode>(&M_DIODE, "Diode"),
        module_of::<Capacitor>(&M_CAPACITOR, "Capacitor"),
        module_of::<FlipFlop>(&M_FLIP_FLOP, "FlipFlop"),
        module_of::<Comparator>(&M_COMPARATOR, "Comparator"),
        module_of::<Dac>(&M_DAC, "Dac"),
    ]
}

/// The bytes of one read cell.
fn f(v: f64) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// Calls function `k` with every partial seeded; returns each write's bytes and `jac`.
fn eval(h: &Handle, k: u32, reads: &[Vec<u8>]) -> (Vec<Vec<u8>>, Vec<f64>) {
    let func = *h.shape().functions.get(k as usize).expect("the function exists");
    let lens = cell_lens(h.shape(), &func.writes, Access::FunctionWrite).expect("valid writes");
    let mut out: Vec<Vec<u8>> = lens.iter().map(|n| vec![0u8; *n]).collect();
    let n = func.partials.len();
    let mut jac = vec![0.0; n];
    let input: Vec<&[u8]> = reads.iter().map(Vec::as_slice).collect();
    let mut outs: Vec<&mut [u8]> = out.iter_mut().map(Vec::as_mut_slice).collect();
    h.eval(k, &input, &mut outs, &vec![1; n], &mut jac).expect("eval");
    (out, jac)
}

fn as_f64(b: &[u8]) -> f64 {
    f64::from_le_bytes(b[..8].try_into().unwrap())
}

/// `got` equals `want` to 1e-12 relative.
fn close(got: f64, want: f64, what: &str) {
    assert!((got - want).abs() <= 1e-12 * want.abs().max(f64::MIN_POSITIVE), "{what}: {got} != {want}");
}

/// The commits a reaction made: (statement, offset, bytes, after).
type Commits = Vec<(u32, u32, Vec<u8>, f64)>;

/// Runs reaction `k`; returns its flow and its commits.
fn react(h: &mut Handle, k: u32, reads: &[Vec<u8>]) -> (Flow, Commits) {
    let mut seen = Vec::new();
    let mut sink = |c: Commit<'_>| {
        seen.push((c.statement, c.offset, c.bytes.to_vec(), c.after));
        status::OK
    };
    let input: Vec<&[u8]> = reads.iter().map(Vec::as_slice).collect();
    let flow = h.react(k, &input, &mut sink).expect("react");
    (flow, seen)
}

// Diode point: is = 1e-14, n = 1, cj0 = 1e-12, T = 300 K, so nvt = 0.025851999786435535 V
// and vcrit = 0.7299118350988295 V (computed by hand from K_B, Q).
const IS: f64 = 1e-14;
const CJ: f64 = 1e-12;
const T: f64 = 300.0;
const NVT: f64 = 0.025851999786435535;

#[test]
fn worked_examples_produce_their_stated_values() {
    let [m_r, m_d, m_c, m_ff, m_cmp, m_dac] = modules();

    // 11.1: i = v / r at v = 1.5 V, r = 1 kΩ.
    let r = Handle::create(m_r, &[]).unwrap();
    let (out, jac) = eval(&r, 0, &[f(1.2), f(-0.3), f(1e3)]);
    close(as_f64(&out[0]), 1.5e-3, "resistor i");
    assert_eq!(jac, [1e-3, -1e-3, -1.5e-6]);

    // 11.2 fn 0: v = 0.8 above vcrit, LAST = 0.6 held (AGE 1, not STATIC): limited to
    // 0.6 + nvt·ln(1 + 0.2/nvt) = 0.6560340016200957.
    let d = Handle::create(m_d, &[]).unwrap();
    let (out, jac) = eval(&d, 0, &[f(0.8), f(0.0), f(IS), f(1.0), f(T), f(0.6), vec![1], vec![0]]);
    close(as_f64(&out[0]), 0.8, "diode PROPOSED");
    close(as_f64(&out[1]), 0.6560340016200957, "diode LIMITED");
    assert_eq!(jac, [1.0, -1.0]);
    // AGE 0: the seed is vcrit, and 0.8 is limited from it.
    let (out, _) = eval(&d, 0, &[f(0.8), f(0.0), f(IS), f(1.0), f(T), f(0.6), vec![0], vec![0]]);
    let from_vcrit = 0.7299118350988295 + NVT * (1.0 + (0.8 - 0.7299118350988295) / NVT).ln();
    close(as_f64(&out[1]), from_vcrit, "diode LIMITED from the seed");
    // fn 1 at vd = 0.65: id = 8.308438347670988e-4, qj = cj·vd.
    let (out, jac) = eval(&d, 1, &[f(0.65), f(IS), f(1.0), f(CJ), f(T), f(0.0), f(0.0)]);
    close(as_f64(&out[0]), 8.308438347670988e-4, "diode id");
    close(as_f64(&out[1]), 0.65e-12, "diode qj");
    close(jac[0], 0.03213847445616335, "diode gd");
    assert_eq!(&jac[1..], [CJ, 1.0, 1.0]);
    // fn 2 shot, fn 3 flicker at 1 kHz, fn 4 the watch gd.
    let (out, _) = eval(&d, 2, &[f(0.65), f(IS), f(1.0), f(T)]);
    close(as_f64(&out[0]), 2.662317157133605e-22, "diode shot PSD");
    let (out, _) = eval(&d, 3, &[f(0.65), f(IS), f(1.0), f(T), f(1e3)]);
    close(as_f64(&out[0]), 8.308438347670988e-23, "diode flicker PSD");
    let (out, _) = eval(&d, 4, &[f(0.65), f(IS), f(1.0), f(T)]);
    close(as_f64(&out[0]), 0.03213847445616335, "diode W0 gd");

    // 11.3: q = c·v at c = 2 nF, v = 0.6 V.
    let c = Handle::create(m_c, &[]).unwrap();
    let (out, jac) = eval(&c, 0, &[f(0.7), f(0.1), f(2e-9)]);
    close(as_f64(&out[0]), 1.2e-9, "capacitor q");
    close(jac[2], 0.6, "capacitor dq/dc");
    assert_eq!(&jac[..2], [2e-9, -2e-9]);

    // 11.4: a rising, changed clock fires `rise`; anything else does not.
    let mut ff = Handle::create(m_ff, &[]).unwrap();
    let (out, _) = eval(&ff, 0, &[HIGH.to_vec(), LOW.to_vec(), vec![1]]);
    assert_eq!(out[0], [1]);
    for (now, prev, changed) in [(HIGH, LOW, 0u8), (LOW, HIGH, 1), (HIGH, HIGH, 1)] {
        let (out, _) = eval(&ff, 0, &[now.to_vec(), prev.to_vec(), vec![changed]]);
        assert_eq!(out[0], [0], "{now:?} after {prev:?}, changed {changed}");
    }
    // q <- d at the next microstep.
    assert_eq!(react(&mut ff, 0, &[HIGH.to_vec()]), (Flow::Continue, vec![(0, 0, HIGH.to_vec(), 0.0)]));
    assert_eq!(react(&mut ff, 0, &[LOW.to_vec()]), (Flow::Continue, vec![(0, 0, LOW.to_vec(), 0.0)]));

    // 11.5: W0 = inp − inn, W1 = −W0; `up` commits HIGH, `down` LOW.
    let mut cmp = Handle::create(m_cmp, &[]).unwrap();
    let (out, jac) = eval(&cmp, 0, &[f(0.3), f(0.1)]);
    close(as_f64(&out[0]), 0.2, "comparator W0");
    close(as_f64(&out[1]), -0.2, "comparator W1");
    assert_eq!(jac, [1.0, -1.0, -1.0, 1.0]);
    assert_eq!(react(&mut cmp, 0, &[]), (Flow::Continue, vec![(0, 0, HIGH.to_vec(), 0.0)]));
    assert_eq!(react(&mut cmp, 1, &[]), (Flow::Continue, vec![(0, 0, LOW.to_vec(), 0.0)]));

    // 11.6: code 128 of 255 against vref 2 V: T0 = f, T1 = v(out) − k·vref.
    let dac = Handle::create(m_dac, &[]).unwrap();
    let k = 128.0 / 255.0;
    let (out, jac) = eval(&dac, 0, &[vec![128], f(0.4), f(1e-3), f(2.0)]);
    close(as_f64(&out[0]), 1e-3, "dac T0");
    close(as_f64(&out[1]), -0.6039215686274509, "dac T1");
    assert_eq!(jac, [1.0, 1.0, -k]);
}

#[test]
fn worked_examples_pass_validation() {
    for m in modules() {
        let proto = m.prototype();
        assert_eq!(validate(proto), Ok(()), "prototype of {}", m.name().as_str());
        let h = Handle::create(m, &[]).unwrap_or_else(|e| panic!("create {}: {e}", m.name().as_str()));
        assert_eq!(validate(h.shape()), Ok(()), "shape of {}", m.name().as_str());
    }
}

#[test]
fn worked_examples_pass_the_partials_verifier() {
    let [m_r, m_d, m_c, _, m_cmp, m_dac] = modules();
    let cases: [(&'static Module, u32, Vec<Vec<u8>>); 6] = [
        (m_r, 0, vec![f(1.2), f(-0.3), f(1e3)]),
        // fn 0 where no limit is active: 0.62 against LAST 0.6 is within 2·nvt.
        (m_d, 0, vec![f(0.62), f(0.0), f(IS), f(1.0), f(T), f(0.6), vec![1], vec![0]]),
        (m_d, 1, vec![f(0.65), f(IS), f(1.0), f(CJ), f(T), f(0.0), f(0.0)]),
        (m_c, 0, vec![f(0.7), f(0.1), f(2e-9)]),
        (m_cmp, 0, vec![f(0.3), f(0.1)]),
        (m_dac, 0, vec![vec![128], f(0.4), f(1e-3), f(2.0)]),
    ];
    for (m, k, point) in cases {
        let h = Handle::create(m, &[]).unwrap();
        assert_eq!(verify_partials(&h, k, &point, 1e-6), Ok(Verdict::Exact), "{} fn {k}", m.name().as_str());
    }
    // The diode's limiter at an active point is skipped, not compared.
    let d = Handle::create(m_d, &[]).unwrap();
    let active = vec![f(0.8), f(0.0), f(IS), f(1.0), f(T), f(0.6), vec![1], vec![0]];
    assert_eq!(verify_partials(&d, 0, &active, 1e-6), Ok(Verdict::Skipped { limit: 0 }));
}
