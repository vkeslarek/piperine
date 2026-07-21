import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "11_resistor_string.phdl")
m = piperine.load(P).module("ParallelBank")
r = m.op()
assert abs(r.v("mid","gnd") - 1.0) < 1e-6, "4x 1k parallel = 250 ohm"
assert abs(r["r_top"].i("p","n") - 4e-3) < 1e-8, "string current 4mA"
print("11_resistor_string: PASS"); sys.stdout.flush()
