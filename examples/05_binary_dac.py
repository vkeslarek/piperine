import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "05_binary_dac.phdl")
def M(): return piperine.load(P).module("DacBoard")
def bits(m, b3=0, b2=0, b1=0, b0=0):
    m.stage("dac","b3",b3); m.stage("dac","b2",b2); m.stage("dac","b1",b1); m.stage("dac","b0",b0)
# test_zero_and_full_scale
m = M(); assert abs(m.op().v("out","gnd")) < 1e-9, "code 0 = 0V"
m = M(); bits(m,1,1,1,1); assert abs(m.op().v("out","gnd")-1.5) < 1e-6, "code 15 = 1.5V"
# test_midscale_step
m7 = M(); bits(m7,0,1,1,1); v7 = m7.op().v("out","gnd")
m8 = M(); bits(m8,1,0,0,0); v8 = m8.op().v("out","gnd")
assert abs((v8-v7)-0.1) < 1e-6, "midscale step is 1 LSB"
# test_lsb_staircase
prev = -1.0
for b in [0.0, 1.0]:
    m = M(); bits(m,0,0,0,b); v = m.op().v("out","gnd")
    assert v > prev, "monotonic in LSB"
    prev = v
print("05_binary_dac: PASS"); sys.stdout.flush()
