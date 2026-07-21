import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "21_shift_register_tran.phdl")
m = piperine.load(P).module("ShiftTran")
tr = m.tran(piperine.TranConfig(stop=5.0e-6, step=20.0e-9))
w0, w1, w2, w3 = tr.v("q0"), tr.v("q1"), tr.v("q2"), tr.v("q3")
assert abs(w0.at(0.7e-6) - 1.0) < 0.5, "after 1 clock: q0 = 1"
assert abs(w1.at(0.7e-6) - 0.0) < 0.5, "q1 still 0"
assert abs(w1.at(1.7e-6) - 1.0) < 0.5, "after 2 clocks: q1 = 1"
assert abs(w2.at(2.7e-6) - 1.0) < 0.5, "after 3 clocks: q2 = 1"
assert abs(w3.at(3.7e-6) - 1.0) < 0.5, "after 4 clocks: q3 = 1"
print("21_shift_register_tran: PASS"); sys.stdout.flush()
