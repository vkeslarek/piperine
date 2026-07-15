import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "07_thermostat.phdl")
m = piperine.load(P).module("Thermostat")
t = m.tran(piperine.TranConfig(stop=0.2, step=1e-4, start=0.05))
temp = t.v("temp","gnd")
assert temp.min() > 20.5, "heater on before ambient"
assert temp.max() < 26.5, "heater off near top"
assert temp.peak_to_peak() > 1.0, "controller cycles"
print("07_thermostat: PASS"); sys.stdout.flush()
