import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "01_voltage_divider.phdl")
def M(): return piperine.load(P).module("DividerBoard")
# test_dc_ratio
m = M(); r = m.op()
assert abs(r.v("mid", "gnd") - 2.0) < 1e-6, "divider ratio is R2/(R1+R2)"
assert abs(r["r_top"].i("p", "n") - 1e-3) < 1e-9, "string current is V/(R1+R2)"
# test_loading_sweep
for rl in [2e3, 1e3, 500.0, 250.0]:
    m = M(); m.set("r_bot", "r", rl); r = m.op()
    expected = 5.0 * rl / (3e3 + rl)
    assert abs(r.v("mid", "gnd") - expected) < 1e-6, f"divider eq at rl={rl}"
print("01_voltage_divider: PASS"); sys.stdout.flush()
