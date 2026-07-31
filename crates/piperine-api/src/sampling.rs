//! Monte Carlo sampling over the authored instance tree (dv-gradients T13,
//! DVG-16): walk the authored POM, classify each `tol`-carrying param by
//! its process (`global`) vs mismatch scope, and produce draws where a
//! `global` parameter gets ONE draw shared by every instance while a plain
//! (mismatch) parameter gets an INDEPENDENT draw per instance — the
//! distinction that makes analog yield mean anything.
//!
//! # The authored-tree walk (UNBREAKABLE)
//!
//! [`SamplePlan::collect`] reads `design.module(...)` — the **authored**
//! POM hierarchy — and never `design.flat_module(...)` (MD-25). The
//! mismatch/process distinction is a structural question that lives on
//! the authored declarations: a `global` parameter is one draw shared by
//! every instance of the same `(module, param)` group, regardless of how
//! the flattener later splices them; a mismatch parameter is one draw
//! per authored instance. The flat form is a codegen concern the sampler
//! must never read.
//!
//! # Draw reproducibility (T14, DVG-17)
//!
//! [`SamplePlan::draws`] is a pure function of `(seed, index, plan)` — the
//! PRNG stream for any one draw is keyed by `(seed, sample_index,
//! draw_group_id)`, where the group id is `(module, param)` for a `global`
//! draw and `(label, param)` for a mismatch draw. The same `(seed, index)`
//! produces byte-identical draws across runs and across processes; a named
//! failing sample replays as an ordinary session.

use std::collections::HashMap;

use piperine_lang::pom::{Design, Distribution, Module};
use piperine_lang::value::Value;

use crate::error::Error;

// `Session::set` is on the entry module; reach it through the session
// re-export so this module owns no `Session` re-export of its own.
use crate::Session;

/// The scope of a sampled parameter (DVG-16): a `global` (`tol … global`)
/// declaration is process variation — one draw shared by every instance of
/// the same `(module, param)` group; a plain `tol …` declaration is
/// mismatch variation — one independent draw per instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleScope {
    /// Process variation (`tol … global`) — one draw shared by every
    /// instance of the same `(module, param)` group.
    Global,
    /// Mismatch variation (the default) — one independent draw per
    /// instance.
    Mismatch,
}

/// One sampling entry: a `(label, param)` pair on the authored instance
/// tree that carries a `tol`-declared distribution, with its nominal value
/// and scope classification. Collected once per `(design, top)` by
/// [`SamplePlan::collect`].
#[derive(Debug, Clone)]
pub struct SampleEntry {
    /// The flat-label form usable with [`Session::set`] — the authored
    /// instance path (`m1.r1` for a hierarchical design, `r1` for a flat
    /// one). Matches the label the flattener produces, so the host's
    /// restamp addressing and the sampler's addressing agree.
    pub label: String,
    /// The authored module name of the instance the param lives on.
    pub module: String,
    /// The param name on that module.
    pub param: String,
    /// The nominal (declared) value of the param — the value a draw
    /// perturbs and the value a post-sampling nominal analysis returns to
    /// (DVG-18 AC5).
    pub default: f64,
    /// The declared distribution (`gauss`/`uniform`, its evaluated args,
    /// the `global` marker).
    pub distribution: Distribution,
}

impl SampleEntry {
    /// The scope of this entry, derived from its distribution's `global`
    /// marker (DVG-16): `global` → process; otherwise mismatch.
    pub fn scope(&self) -> SampleScope {
        if self.distribution.global {
            SampleScope::Global
        } else {
            SampleScope::Mismatch
        }
    }
}

/// One draw produced by [`SamplePlan::draws`]: the `(label, param, value)`
/// write to apply to a session through the ordinary restamp path
/// ([`Session::set`]).
#[derive(Debug, Clone, PartialEq)]
pub struct SampleDraw {
    /// The instance label the draw writes to.
    pub label: String,
    /// The param name the draw writes to.
    pub param: String,
    /// The drawn value — the nominal plus the sampled perturbation.
    pub value: f64,
}

/// A walk of the authored instance tree collecting every `tol`-carrying
/// param (dv-gradients T13, DVG-16). Built once per `(design, top)`; the
/// plan is plain data and [`Self::draws`] is a pure function of
/// `(seed, index, plan)` — any sample is replayable from those alone.
#[derive(Debug, Clone)]
pub struct SamplePlan {
    entries: Vec<SampleEntry>,
}

impl SamplePlan {
    /// Walk the authored instance tree of `top` and collect every param
    /// carrying a `tol` declaration (DVG-16). Reads `design.module(...)`
    /// — the **authored** POM — and never `design.flat_module(...)` (MD-25,
    /// the UNBREAKABLE rule). The walk descends through sub-instances,
    /// producing path-prefixed labels (`m1.r1`) that match the labels the
    /// flattener produces, so the host's restamp addressing and the
    /// sampler's addressing agree.
    pub fn collect(design: &Design, top: &str) -> Result<Self, Error> {
        let top_module = design.module(top).ok_or_else(|| {
            Error::Measurement(format!(
                "monte carlo: top module `{top}` not found in the authored POM"
            ))
        })?;
        let mut entries = Vec::new();
        let mut walker = AuthoredTreeWalker { design, entries: &mut entries };
        walker.walk_instances(top_module, "");
        Ok(Self { entries })
    }

    /// Every collected entry, in authored-tree walk order (top-down,
    /// declaration order at each level).
    pub fn entries(&self) -> &[SampleEntry] {
        &self.entries
    }

    /// `true` when the plan collected no `tol`-carrying params — a design
    /// with no declared distributions yields no draws and `monte_carlo`
    /// reports every sample as identical to the nominal.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Compute the draws for sample `index` given `seed` (DVG-16): a
    /// `global` parameter gets ONE draw shared by every instance of the
    /// same `(module, param)` group; a plain (mismatch) parameter gets an
    /// INDEPENDENT draw per instance. Deterministic in
    /// `(seed, index, plan)` — replays produce byte-identical draws (T14).
    pub fn draws(&self, seed: u64, index: usize) -> Vec<SampleDraw> {
        // First pass: compute ONE draw per `global` group — every entry
        // in the group will read this same value. The PRNG stream is
        // keyed by `(seed, sample_index, "module.param")`, so two
        // instances of the same module+param share the draw no matter
        // where the walker placed them in entry order.
        let mut global_draws: HashMap<String, f64> = HashMap::new();
        for entry in &self.entries {
            if entry.scope() == SampleScope::Global {
                let key = group_key(entry);
                if !global_draws.contains_key(&key) {
                    let group_seed = mix(seed, index, &key);
                    global_draws.insert(key.clone(), entry.draw_from(group_seed));
                }
            }
        }
        // Second pass: emit one draw per entry — `global` reads the shared
        // group draw, mismatch computes a fresh instance-keyed draw.
        self.entries
            .iter()
            .map(|entry| {
                let value = match entry.scope() {
                    SampleScope::Global => {
                        let key = group_key(entry);
                        global_draws[&key]
                    }
                    SampleScope::Mismatch => {
                        // The instance stream is keyed by (seed, index,
                        // "label.param") — every instance gets its own.
                        let inst_key = instance_key(entry);
                        let inst_seed = mix(seed, index, &inst_key);
                        entry.draw_from(inst_seed)
                    }
                };
                SampleDraw {
                    label: entry.label.clone(),
                    param: entry.param.clone(),
                    value,
                }
            })
            .collect()
    }

    /// Apply the draws for sample `index` to `session` through the ordinary
    /// restamp path ([`Session::set`]) — no new mutation route (DVG-16).
    /// Each draw is one `set(label, param, value)` call; the circuit is
    /// not re-elaborated (MD-18 — compile count stays at 1).
    pub fn apply(&self, session: &mut Session, seed: u64, index: usize) -> Result<(), Error> {
        for draw in self.draws(seed, index) {
            session.set(&draw.label, &draw.param, draw.value)?;
        }
        Ok(())
    }
}

impl SampleEntry {
    /// Draw one value from this entry's distribution, seeded by `seed`.
    /// The nominal is the baseline; the drawn perturbation is added to it.
    fn draw_from(&self, seed: u64) -> f64 {
        let mut rng = SplitMix64::new(seed);
        let args = &self.distribution.args;
        let arg = |name: &str| -> Option<f64> {
            args.iter()
                .find(|(n, _)| n == name)
                .and_then(|(_, v)| v.as_real())
        };
        let delta = match self.distribution.kind.as_str() {
            "gauss" => {
                if let Some(sigma) = arg("sigma") {
                    sigma * rng.next_std_normal()
                } else if let Some(sigma_rel) = arg("sigma_rel") {
                    self.default * sigma_rel * rng.next_std_normal()
                } else {
                    return self.default;
                }
            }
            "uniform" => {
                if let Some(half) = arg("half") {
                    half * (2.0 * rng.next_f64() - 1.0)
                } else if let Some(rel) = arg("rel") {
                    self.default * rel * (2.0 * rng.next_f64() - 1.0)
                } else {
                    return self.default;
                }
            }
            _ => return self.default,
        };
        self.default + delta
    }
}

// ── the authored-tree walker ──────────────────────────────────────────────

struct AuthoredTreeWalker<'a> {
    design: &'a Design,
    entries: &'a mut Vec<SampleEntry>,
}

impl<'a> AuthoredTreeWalker<'a> {
    /// Walk `module`'s authored instances, descending recursively. The
    /// path prefix `parent_path` is the dotted instance path from the
    /// top module down to this module's parent (`"m1"` for `m1.r1`,
    /// empty at the top).
    fn walk_instances(&mut self, module: &Module, parent_path: &str) {
        for instance in module.instances() {
            let label = if parent_path.is_empty() {
                instance.name().to_string()
            } else {
                format!("{parent_path}.{}", instance.name())
            };
            // Read the instance's module from the AUTHORED POM
            // (`design.module(...)`) — never `flat_module` (MD-25). A
            // missing module (e.g. an unresolved plugin device) is
            // skipped: it carries no PHDL-declared `tol`.
            let Some(child) = self.design.module(instance.module_name()) else {
                continue;
            };
            // Collect every param on this instance's module that carries
            // a `tol` declaration.
            for param in child.params() {
                let Some(dist) = param.distribution() else { continue };
                let Some(default) = param.default().and_then(Value::as_real) else {
                    continue;
                };
                self.entries.push(SampleEntry {
                    label: label.clone(),
                    module: instance.module_name().to_string(),
                    param: param.name().to_string(),
                    default,
                    distribution: dist.clone(),
                });
            }
            // Recurse into sub-instances — the authored walk descends the
            // full hierarchy, not just the top level.
            self.walk_instances(child, &label);
        }
    }
}

/// The group id for a `global` draw: `(module, param)` — every instance
/// of the same module+param shares one draw (DVG-16).
fn group_key(entry: &SampleEntry) -> String {
    format!("{}.{}", entry.module, entry.param)
}

/// The instance id for a mismatch draw: `(label, param)` — every instance
/// gets its own draw (DVG-16).
fn instance_key(entry: &SampleEntry) -> String {
    format!("{}.{}", entry.label, entry.param)
}

/// Mix `(seed, sample_index, key)` into a per-draw PRNG seed. A fixed
/// mixing scheme (not a cryptographic hash) — its job is to give every
/// draw a distinct, deterministic stream so two different draw ids
/// produce independent-looking values while the same id always produces
/// the same value.
fn mix(seed: u64, index: usize, key: &str) -> u64 {
    // SplitMix64-style mixing — a few rounds is enough to decorrelate.
    let mut h = seed
        .wrapping_add((index as u64).wrapping_mul(0x9E3779B97F4A7C15));
    for byte in key.bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x100000001B3);
    }
    // Final SplitMix64 output mixer for avalanching.
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D049BB133111EB);
    h ^ (h >> 31)
}

// ── SplitMix64: a small deterministic PRNG ───────────────────────────────
//
// Pure Rust stdlib, no `rand` dependency. SplitMix64 is the same algorithm
// `rand` uses to seed its PRNGs from a u64; using it directly gives us a
// deterministic, cross-process stream from a `u64` seed — exactly what
// `(seed, index)` reproducibility (T14) needs. The algorithm is fixed and
// will not change across Rust releases.

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        // A zero seed would make the first output zero — replace it.
        Self { state: if seed == 0 { 0xDEADBEEFCAFEBABE } else { seed } }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)` using the top 53 bits (the f64 significand).
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0f64 / ((1u64 << 53) as f64))
    }

    /// A standard-normal sample via Box–Muller on two fresh uniforms.
    fn next_std_normal(&mut self) -> f64 {
        loop {
            let u1 = self.next_f64();
            if u1 > 1e-300 {
                let u2 = self.next_f64();
                let r = (-2.0 * u1.ln()).sqrt();
                let theta = 2.0 * std::f64::consts::PI * u2;
                return r * theta.cos();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The PRNG stream is deterministic across constructions and the
    /// uniforms span `[0, 1)` — a baseline for the higher-level draw
    /// reproducibility tests (T14).
    #[test]
    fn splitmix64_is_deterministic_and_uniform() {
        let mut a = SplitMix64::new(42);
        let mut b = SplitMix64::new(42);
        for _ in 0..32 {
            let x = a.next_f64();
            assert_eq!(x, b.next_f64(), "same seed → same stream");
            assert!((0.0..1.0).contains(&x), "uniform in [0, 1): got {x}");
        }
    }

    /// The mixing scheme gives distinct seeds for distinct keys — the
    /// whole point of mismatch variation.
    #[test]
    fn mix_distinguishes_keys() {
        let seed_a = mix(7, 0, "r1.r");
        let seed_b = mix(7, 0, "r2.r");
        assert_ne!(seed_a, seed_b, "distinct keys must produce distinct seeds");
        // And stable across calls.
        assert_eq!(seed_a, mix(7, 0, "r1.r"));
    }
}
