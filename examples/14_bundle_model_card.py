import os, sys, math, piperine
P = os.path.join(os.path.dirname(__file__), "14_bundle_model_card.phdl")
def M(): return piperine.load(P).module("BiasCell")
# test_measured_drop_matches_the_model_card
# (Python can't construct a DiodeModel bundle + call forward_drop; replicate
#  the analytic prediction directly: i = isat*(exp(vd/vt)-1), i = (5-vd)/R.)
m = M(); vd = m.op().v("out","gnd")
isat, vt, R = 1e-14, 0.02585, 1e3
predicted = vt * math.log((5.0 - vd) / R / isat)
assert abs(vd - predicted) < 1e-3, "solver agrees with analytic model"
# test_staging_a_model_field
m1 = M(); v1 = m1.op().v("out","gnd")
m2 = M(); m2.set("d1","model_isat",1e-12); v2 = m2.op().v("out","gnd")
shift = v1 - v2
assert abs(shift - 0.119) < 5e-3, "isat scaling shifts drop by vt*ln(100)"
print("14_bundle_model_card: PASS"); sys.stdout.flush()
