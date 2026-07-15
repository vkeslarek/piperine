import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "19_multiplier_2x2.phdl")
m = piperine.load(P).module("Mult2")
for a1 in [0,1]:
 for a0 in [0,1]:
  for b1 in [0,1]:
   for b0 in [0,1]:
        m.stage("da0","level",a0);m.stage("da1","level",a1)
        m.stage("db0","level",b0);m.stage("db1","level",b1)
        r = m.op()
        prod = r.v("pp00")+2*r.v("p1")+4*r.v("p2")+8*r.v("p3")
        a = 2*a1+a0; b = 2*b1+b0
        assert abs(prod-a*b)<1e-9, f"{a}*{b}={prod}"
print("19_multiplier_2x2: PASS"); sys.stdout.flush()
