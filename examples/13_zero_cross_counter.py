import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "13_zero_cross_counter.phdl")
m = piperine.load(P).module("Counter")
t = m.tran(piperine.TranConfig(stop=3e-3, step=5e-6))
n = t.v("count","gnd")
assert abs(n.at(3e-3) - 6.0) < 1.1, "six crossings in three periods"
first = t.v("sig","gnd").cross(0.0, "Falling")
assert first is not None, "sine crosses zero"
assert abs(first - 5e-4) < 5e-5, "first falling at T/2"
print("13_zero_cross_counter: PASS"); sys.stdout.flush()
