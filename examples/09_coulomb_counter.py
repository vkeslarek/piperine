import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "09_coulomb_counter.phdl")
m = piperine.load(P).module("Gauge")
t = m.tran(piperine.TranConfig(stop=10.0, step=0.05))
soc = t.v("soc","gnd")
assert abs(soc.at(0.0) - 1.0) < 1e-6, "starts full"
assert abs(soc.at(10.0) - 0.9) < 5e-3, "10% discharge over 10s"
t95 = soc.cross(0.95)
assert t95 is not None, "95% level is crossed"
assert abs(t95 - 5.0) < 0.2, "95% at t=5s"
print("09_coulomb_counter: PASS"); sys.stdout.flush()
