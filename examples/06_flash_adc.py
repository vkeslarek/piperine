import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "06_flash_adc.phdl")
def M(): return piperine.load(P).module("AdcBoard")
# test_transfer_staircase
for level in [0.5, 1.5, 2.5, 3.5]:
    m = M(); m.set("src","voltage",level)
    t = m.tran(piperine.TranConfig(stop=1e-5, step=1e-6))
    code = t.v("rec","gnd").at(1e-5)
    expected = level - 0.5
    assert abs(code - expected) < 1e-6, f"vin={level} code={code}"
print("06_flash_adc: PASS"); sys.stdout.flush()
