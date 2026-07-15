import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "04_sine_source.phdl")
def M(): return piperine.load(P).module("SineDemo")
# test_sine_shape
m = M(); t = m.tran(piperine.TranConfig(stop=2e-3, step=1e-5)); v = t.v("out", "gnd")
assert abs(v.at(0.25e-3) - 2.0) < 0.01, "peak at T/4"
assert abs(v.at(0.75e-3) + 2.0) < 0.01, "negative peak at 3T/4"
assert abs(v.rms() - 1.4142) < 0.05, "RMS = A/sqrt(2)"
assert abs(v.mean()) < 0.05, "zero mean"
assert abs(v.peak_to_peak() - 4.0) < 0.05, "peak-to-peak = 2A"
# test_load_current_follows_ohm
m = M(); t = m.tran(piperine.TranConfig(stop=1e-3, step=1e-5))
i = t["r_load"].i("p", "n")
assert abs(i.max() - 1e-3) < 1e-5, "peak string current is A/(2R)"
assert abs(t.v("mid", "gnd").max() - 1.0) < 0.01, "midpoint peak is A/2"
# test_dc_point_is_zero
m = M(); assert abs(m.op().v("out", "gnd")) < 1e-9, "DC sits at 0"
print("04_sine_source: PASS"); sys.stdout.flush()
