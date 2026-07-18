import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "15_full_adder.phdl")
m = piperine.load(P).module("AdderBoard")
for a in [0.0,1.0]:
    for b in [0.0,1.0]:
        for cin in [0.0,1.0]:
            m.set("da","level",a); m.set("db","level",b); m.set("dc","level",cin)
            r = m.op()
            total = r.v("nsum") + 2.0*r.v("ncout")
            assert abs(total - (a+b+cin)) < 1e-9, f"{a}+{b}+{cin}={total}"
print("15_full_adder: PASS"); sys.stdout.flush()
