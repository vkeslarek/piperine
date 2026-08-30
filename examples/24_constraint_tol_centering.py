"""`constraint begin ... end` + `tol` (Piperine's own extension surface,
docs/spec/part_iv_reflection_selector.md, appendix_c_host_surface.md) read
back through the Python host: the two `require`s on `Dut` produce real
margin entries at the nominal operating point, and the nominal design sits
exactly on `head_lo`'s boundary (zero margin) -- known off-center geometry.

The design-centering *driver* itself (`Driver`/`Policy::Center`) has no
Python binding yet (a declared Rust-only asymmetry, appendix_c_host_surface.md
section 3 / spec.md's B5) -- `tests/example_constraint_centering.rs` runs
that half on the Rust host against this exact same .vams file and asserts
the centering pass genuinely improves on the boundary condition this script
establishes.
"""

import os
import sys

import piperine

P = os.path.join(os.path.dirname(__file__), "24_constraint_tol_centering.vams")
design = piperine.load(P)
session = design.compile()

solver = piperine.Solver(checks="collect")
op = session.op(piperine.OpConfig(solver=solver))

margins = op.margins()
assert len(margins) == 2, f"two requires declared, got {len(margins)}"

hi = margins.get("head_hi")
lo = margins.get("head_lo")
assert hi.instance_path == "dut1" and lo.instance_path == "dut1"
assert hi.kind == "require" and lo.kind == "require"

v_out = session.set  # no-op reference to keep session alive for op() above
vout = op.v("out", "gnd")
assert abs(vout - 0.4) < 1e-9, f"nominal r1=1500 should put V(out) exactly at the head_lo floor, got {vout}"

# head_hi = hi - V(out) = 0.6 - 0.4 = 0.2 (satisfied, comfortable margin)
assert abs(hi.scalar() - 0.2) < 1e-9, hi.scalar()
assert hi.is_satisfied()

# head_lo = V(out) - lo = 0.4 - 0.4 = 0.0 (exactly on the boundary)
assert abs(lo.scalar() - 0.0) < 1e-9, lo.scalar()

print("24_constraint_tol_centering: PASS")
sys.stdout.flush()
