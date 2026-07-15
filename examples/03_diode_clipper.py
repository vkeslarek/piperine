import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "03_diode_clipper.phdl")
def M(): return piperine.load(P).module("Clipper")
# test_clamps_to_forward_drop
m = M(); vd = m.op().v("out", "gnd")
assert 0.6 < vd < 0.75, "output pins near one diode drop"
# test_clip_level_barely_moves_with_drive
m1 = M(); v1 = m1.op().v("out", "gnd")
m2 = M(); m2.stage("source", "voltage", 10.0); v2 = m2.op().v("out", "gnd")
assert v2 > v1, "harder drive pushes further"
assert v2 - v1 < 0.05, "doubling drive moves clamp by millivolts only"
print("03_diode_clipper: PASS"); sys.stdout.flush()
