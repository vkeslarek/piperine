import os, sys, math, piperine
P = os.path.join(os.path.dirname(__file__), "17_ripple_adder_4bit.phdl")
m = piperine.load(P).module("Adder4")
def bit(v, i): return float((int(v) >> i) & 1)
for a in range(16):
    for b in range(16):
        m.set("da0","level",bit(a,0)); m.set("da1","level",bit(a,1))
        m.set("da2","level",bit(a,2)); m.set("da3","level",bit(a,3))
        m.set("db0","level",bit(b,0)); m.set("db1","level",bit(b,1))
        m.set("db2","level",bit(b,2)); m.set("db3","level",bit(b,3))
        r = m.op()
        s = r.v("s0")+2*r.v("s1")+4*r.v("s2")+8*r.v("s3")+16*r.v("cout")
        assert abs(s-(a+b))<1e-9, f"{a}+{b}={s}"
print("17_ripple_adder_4bit: PASS"); sys.stdout.flush()
