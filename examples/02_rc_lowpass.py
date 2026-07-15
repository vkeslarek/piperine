import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "02_rc_lowpass.phdl")
def M(): return piperine.load(P).module("RcFilter")
# test_frequency_response
m = M(); r = m.ac(piperine.AcConfig(fstart=1.0, fstop=1e6, points=600))
cw = r.v("out", "gnd")
assert abs(cw.mag.at(10.0) - 1000.0) < 20.0, "passband magnitude is R"
assert abs(cw.mag.at(1591.5) - 707.1) < 25.0, "corner magnitude is R/sqrt(2)"
rolloff = cw.db.at(159155.0) - cw.db.at(1591.5)
assert -45.0 < rolloff < -35.0, "-20 dB/decade above the corner"
# test_discharge_follows_exponential
m = M(); t = m.tran(piperine.TranConfig(stop=5e-4, step=1e-6, ic={"out": 1.0}))
v = t.v("out", "gnd")
assert abs(v.at(1e-4) - 0.3679) < 0.02, "one tau leaves e^-1"
assert v.at(5e-4) < 0.02, "five tau fully discharges"
print("02_rc_lowpass: PASS"); sys.stdout.flush()
