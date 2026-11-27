//! Every function of every M1 body against the partials verifier (feature `ir-interp`, INT-03
//! AC 4): at three bias points, with `rel_tol` 1e-6, each declared partial agrees with its
//! central difference; the diode's limit is probed where `LIMITED = PROPOSED`, `LAST` held at
//! the proposed value so `pnjlim` returns it.

use piperine_abi::caller::Handle;
use piperine_abi::device::{Access, cell_lens};
use piperine_abi::raw::{Cell, Shape, ambient, cell, part, role};
use piperine_abi::verify::{Verdict, verify_partials};

use crate::models;

use models::Models;

/// One bias point: the two node potentials and the flow of a branch's private pin.
struct Bias {
    first: f64,
    second: f64,
}

impl Bias {
    const POINTS: [Bias; 3] = [Bias { first: 0.30, second: 0.0 }, Bias { first: 0.55, second: 0.05 }, Bias { first: 0.62, second: 0.10 }];

    /// The value of a param by name, within what every body accepts.
    fn param(name: &str) -> f64 {
        match name {
            "is" => 1e-14,
            "n" => 1.0,
            "cj0" => 1e-12,
            "r" => 1000.0,
            "c" => 1e-9,
            "dc" => 1.5,
            "ac" => 1.0,
            other => panic!("no bias for param `{other}`"),
        }
    }

    /// The bytes of read `c` of a function of `shape`, `len` long.
    fn bytes(&self, shape: &Shape<'_>, c: &Cell, len: usize) -> Vec<u8> {
        let real = |v: f64| v.to_le_bytes().to_vec();
        let vd = self.first - self.second;
        let value = match c.kind {
            cell::PIN => {
                let pin = shape.pins.get(c.index as usize).expect("a pin of the shape");
                match pin.role {
                    role::PARAM => Self::param(pin.name.as_str()),
                    role::GLOBAL => 300.15,
                    role::PORT => {
                        let ports = shape.pins.iter().filter(|p| p.role == role::PORT);
                        let at = ports.take_while(|p| p.name.as_str() != pin.name.as_str()).count();
                        if at == 0 { self.first } else { self.second }
                    }
                    _ => 0.002,
                }
            }
            cell::INPUT => match c.part {
                part::LAST => vd,
                part::AGE => 2.0,
                _ => vd,
            },
            cell::AMBIENT => match c.index {
                ambient::GMIN => 1e-12,
                ambient::SOURCE_SCALE => 1.0,
                _ => 0.0,
            },
            other => panic!("a read of kind {other}"),
        };
        match len {
            8 => real(value),
            1 => vec![value as u8],
            n => panic!("a read cell of {n} bytes"),
        }
    }
}

#[test]
fn every_m1_function_passes_verify_partials() {
    let m = Models::all();
    let mut checked = 0;
    for body in ["Resistor", "Capacitor", "Vsource", "Isource", "Diode"] {
        let handle = Handle::create(m.module(body), &[]).unwrap_or_else(|e| panic!("create {body}: {e}"));
        let shape = handle.shape();
        for (k, f) in shape.functions.iter().enumerate() {
            let lens = cell_lens(shape, &f.reads, Access::FunctionRead).expect("lens");
            let mut exact = 0;
            for (i, bias) in Bias::POINTS.iter().enumerate() {
                let point: Vec<Vec<u8>> = f.reads.iter().zip(&lens).map(|(c, len)| bias.bytes(shape, c, *len)).collect();
                match verify_partials(&handle, k as u32, &point, 1e-6) {
                    Ok(Verdict::Exact) => exact += 1,
                    Ok(other) => panic!("{body} {} at point {i}: {other:?}", f.name.as_str()),
                    Err(e) => panic!("{body} {} at point {i} faulted: {e}", f.name.as_str()),
                }
            }
            assert_eq!(exact, 3, "{body} {}", f.name.as_str());
            checked += 1;
        }
    }
    // resistor 2, capacitor 2, vsource 2, isource 2, diode 5
    assert_eq!(checked, 13);
}
