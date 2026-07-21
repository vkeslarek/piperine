import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "10_pwm_dimmer.phdl")
m = piperine.load(P).module("Dimmer")
t = m.tran(piperine.TranConfig(stop=10e-3, step=5e-6, start=5e-3))
v = t.v("out","gnd")
assert abs(v.mean() - 2.5) < 0.2, "50% duty averages to VDD/2"
assert v.peak_to_peak() < 0.5, "RC swallows 10kHz ripple"
print("10_pwm_dimmer: PASS"); sys.stdout.flush()
