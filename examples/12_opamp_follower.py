import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "12_opamp_follower.phdl")
def M(): return piperine.load(P).module("Follower")
# test_dc_tracks_input
m = M(); assert abs(m.op().v("out","gnd") - 1.0) < 5e-4, "follower tracks input"
# test_step_settles_at_gbw
m = M(); t = m.tran(piperine.TranConfig(stop=1e-5, step=2e-8, ic={"comp": 0.0}))
v = t.v("out","gnd")
assert abs(v.at(1.59e-6) - 0.632) < 0.05, "one tau reaches 63%"
assert v.at(1e-5) > 0.99, "six tau settles"
print("12_opamp_follower: PASS"); sys.stdout.flush()
