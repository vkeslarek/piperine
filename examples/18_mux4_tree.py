import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "18_mux4_tree.phdl")
m = piperine.load(P).module("Mux4")
for v0 in [0,1]:
 for v1 in [0,1]:
  for v2 in [0,1]:
   for v3 in [0,1]:
    for s0 in [0,1]:
     for s1 in [0,1]:
        m.set("d0","level",v0);m.set("d1","level",v1)
        m.set("d2","level",v2);m.set("d3","level",v3)
        m.set("ds0","level",s0);m.set("ds1","level",s1)
        r = m.op()
        exp = v0*(1-s0)*(1-s1)+v1*s0*(1-s1)+v2*(1-s0)*s1+v3*s0*s1
        assert abs(r.v("out")-exp)<1e-9, f"mux {v0}{v1}{v2}{v3} sel={s1}{s0}"
print("18_mux4_tree: PASS"); sys.stdout.flush()
