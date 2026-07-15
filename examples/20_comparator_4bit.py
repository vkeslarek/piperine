import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "20_comparator_4bit.phdl")
m = piperine.load(P).module("Comparator4")
def bit(v, i): return float((int(v) >> i) & 1)
for a in range(16):
    for b in range(16):
        for i in range(4):
            m.stage(f"da{i}","level",bit(a,i)); m.stage(f"db{i}","level",bit(b,i))
        r = m.op()
        assert abs(r.v("gt")-(1.0 if a>b else 0.0))<1e-9, f"gt {a}>{b}"
        assert abs(r.v("eq")-(1.0 if a==b else 0.0))<1e-9, f"eq {a}=={b}"
print("20_comparator_4bit: PASS"); sys.stdout.flush()
