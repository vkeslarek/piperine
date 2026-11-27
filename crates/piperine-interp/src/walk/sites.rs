//! The sites of an analog walk: the `limit` and `ac_stim` calls, the phasor and the held limiter state.

use piperine_abi::device::Fault;
use piperine_ir::ExprId;

use super::{Held, Limited, Value, Walk};

impl<'a> Walk<'a> {
    /// The `LAST`, `AGE` and `STATIC` a `Limit` walk reads.
    pub(crate) fn hold(&mut self, held: Held) {
        if let Some(a) = self.analog.as_mut() {
            a.held = Some(held);
        }
    }

    /// The fault code that ends a site walk once its site has written: `run` turns it into
    /// success and it never leaves the walk.
    pub(super) const DONE: u32 = u32::MAX;

    /// What a `Stim` walk wrote at its site: `(RE, IM)`.
    pub(crate) fn phasor(&self) -> Option<(f64, f64)> {
        self.analog.as_ref().and_then(|a| a.phasor)
    }

    /// The site of a `Stim(k)` walk: `mag` and `phase` over plain reals; `RE = mag·cos(phase)`,
    /// `IM = mag·sin(phase)`, and exactly `RE = mag`, `IM = 0` with no phase. Ends the walk.
    pub(super) fn stim_site(&mut self, mag: ExprId, phase: Option<ExprId>) -> Result<Value, Fault> {
        let mag = self.eval_real(mag)?.v;
        let phasor = match phase {
            Some(p) => {
                let phase = self.eval_real(p)?.v;
                (mag * phase.cos(), mag * phase.sin())
            }
            None => (mag, 0.0),
        };
        if let Some(a) = self.analog.as_mut() {
            a.phasor = Some(phasor);
        }
        Err(Fault::new(Self::DONE, "the site has written"))
    }

    /// What a `Limit` walk wrote at its site.
    pub(crate) fn limited(&self) -> Option<&Limited> {
        self.analog.as_ref().and_then(|a| a.limited.as_ref())
    }

    /// The site of a `Limit(k)` walk (§18 rules 1 and 2, abi.md A2): `x` over duals, `old` the
    /// seed (or `x` with no seed) when `AGE` is 0 or `STATIC` and `AGE` is 1, else `LAST`;
    /// `LIMITED = f(x, old)` over plain reals, raised to the seed when it starts from the seed
    /// and falls below it. Ends the walk.
    pub(super) fn limit_site(&mut self, id: ExprId, x: ExprId, f: ExprId, seed: Option<ExprId>) -> Result<Value, Fault> {
        let held = match self.analog.as_ref().and_then(|a| a.held) {
            Some(h) => h,
            None => return Err(self.fault(id, "a `limit` walk without its LAST, AGE and STATIC")),
        };
        let x = self.eval_real(x)?;
        let Value::Closure(closure) = self.eval(f)? else {
            return Err(self.fault(f, "the function of a `limit` is not a closure"));
        };
        let seed = match seed {
            Some(s) => Some(self.eval_real(s)?.v),
            None => None,
        };
        let from_seed = held.age == 0 || (held.is_static && held.age == 1);
        let old = if from_seed { seed.unwrap_or(x.v) } else { held.last };
        let mut limited = self.call_closure(closure, &[x.v, old])?;
        if let (true, Some(s)) = (from_seed, seed) {
            if limited < s {
                limited = s;
            }
        }
        if let Some(a) = self.analog.as_mut() {
            a.limited = Some(Limited { proposed: x, limited });
        }
        Err(Fault::new(Self::DONE, "the site has written"))
    }
}
