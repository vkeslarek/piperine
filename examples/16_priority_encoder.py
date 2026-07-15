import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "16_priority_encoder.phdl")
def M(): return piperine.load(P).module("IrqBoard")
def read_code(m):
    r = m.op()
    return 2.0*r.v("y1") + r.v("y0") + 4.0*r.v("ok")
# test_idle_reports_invalid
m = M(); assert abs(read_code(m)) < 1e-9, "no request: invalid"
# test_highest_request_wins
m = M(); m.stage("d1","level",1.0)
assert abs(read_code(m) - 5.0) < 1e-9, "r1: code 1+valid"
m.stage("d3","level",1.0)
assert abs(read_code(m) - 7.0) < 1e-9, "r3 outranks r1"
m.stage("d3","level",0.0); m.stage("d2","level",1.0)
assert abs(read_code(m) - 6.0) < 1e-9, "r2 outranks r1"
print("16_priority_encoder: PASS"); sys.stdout.flush()
