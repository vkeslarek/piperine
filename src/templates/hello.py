import piperine as ppr

sess = ppr.NgspiceSession.from_file("hello/hello.ppr", module="divider")

op = sess.op()
print(f"V(vmid) = {op['vmid']:.3f} V")   # expect 5.000 V
