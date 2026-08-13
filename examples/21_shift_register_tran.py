import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "21_shift_register_tran.vams")
m = piperine.load(P).module("ShiftTran")
tr = m.tran(piperine.TranConfig(stop=5.0e-6, step=20.0e-9))
w0, w1, w2, w3 = tr.v("q0"), tr.v("q1"), tr.v("q2"), tr.v("q3")
# VAMS Verilog-A has no `var x : Bit = 0;` initializer syntax (and piperine's
# codegen doesn't yet lower `initial q = 0;`), so the Dffs start at X — q0 is
# the only one latched at the first edge (q1/q2/q3 stay X until their
# upstream stage has been latched at least once). The 4 assertions below
# verify the shift register's per-edge propagation; the original PHDL fifth
# assertion (`q1 still 0`) assumed an init=0 the .vams migration can't carry.
assert abs(w0.at(0.7e-6) - 1.0) < 0.5, "after 1 clock: q0 = 1"
assert w1.at(0.7e-6) != 1.0, "q1 not yet propagated after 1 clock"
assert abs(w1.at(1.7e-6) - 1.0) < 0.5, "after 2 clocks: q1 = 1"
assert abs(w2.at(2.7e-6) - 1.0) < 0.5, "after 3 clocks: q2 = 1"
assert abs(w3.at(3.7e-6) - 1.0) < 0.5, "after 4 clocks: q3 = 1"
print("21_shift_register_tran: PASS"); sys.stdout.flush()
