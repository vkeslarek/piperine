"""Live-session optimization loop (LIVE-12): fit a series resistor so the
diode voltage hits a target, via bisection on a compiled LiveSession — one
JIT compilation for the whole fit, every point a solver-level restamp.

Then the perf/accuracy contract: >= 100 set+op iterations on the nonlinear
circuit equal per-point fresh builds within reltol 1e-3, and the live loop
is >= 10x faster than re-elaborating per point.

(The zero-recompile proof is the compile-count test,
crates/piperine-python/tests/live_session.rs; here `rebuilds == 0` is the
session's own visible notice that no re-elaboration happened.)
"""

import os
import sys
import time

import piperine

P = os.path.join(os.path.dirname(__file__), "live_optimize.phdl")
design = piperine.load(P)
module = design.module("Fitter")

# ── bisection: one compile, set + op per probe ────────────────────────────
session = design.compile()  # Fitter is the design's only root
target = 0.62  # V — diode drop wanted at `out`
lo, hi = 1e2, 1e6  # v(out) falls monotonically as r grows
for _ in range(60):
    mid = 0.5 * (lo + hi)
    session.set("r1", "r", mid)
    if session.op().v("out") > target:
        lo = mid
    else:
        hi = mid
r_fit = 0.5 * (lo + hi)
session.set("r1", "r", r_fit)
v_fit = session.op().v("out")
assert abs(v_fit - target) < 1e-4, (r_fit, v_fit)
assert session.rebuilds == 0, "pure value sets must never re-elaborate"

# ── LIVE-12: 100 set+op points == fresh builds (reltol 1e-3), >= 10x faster ─
# Both loops warm-start each point from the previous solution (standard
# sweep practice), so they run the identical Newton work — the timing delta
# is purely the per-point re-elaboration + re-JIT the live session skips.
rs = [1e3 * (1.0 + 0.05 * i) for i in range(100)]

t0 = time.perf_counter()
live = []
guess = {"vin": 5.0, "out": 0.65}
for r in rs:
    session.set("r1", "r", r)
    v = session.op(piperine.OpConfig(nodeset=guess)).v("out")
    guess = {"vin": 5.0, "out": v}
    live.append(v)
t_live = time.perf_counter() - t0

t0 = time.perf_counter()
fresh = []
guess = {"vin": 5.0, "out": 0.65}
for r in rs:
    module.set("r1", "r", r)  # each op() re-elaborates + re-JITs (fresh build)
    v = module.op(piperine.OpConfig(nodeset=guess)).v("out")
    guess = {"vin": 5.0, "out": v}
    fresh.append(v)
t_fresh = time.perf_counter() - t0

for r, a, b in zip(rs, live, fresh):
    assert abs(a - b) <= 1e-3 * max(abs(a), abs(b)) + 1e-9, (r, a, b)
speedup = t_fresh / t_live
assert speedup >= 10.0, f"live loop must be >= 10x faster, got {speedup:.1f}x"

print(f"live_optimize: PASS (r_fit={r_fit:.1f} ohm, v={v_fit:.4f} V, {speedup:.0f}x)")
sys.stdout.flush()
