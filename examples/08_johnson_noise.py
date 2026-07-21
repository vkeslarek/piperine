import os, sys, piperine
P = os.path.join(os.path.dirname(__file__), "08_johnson_noise.phdl")
m = piperine.load(P).module("NoiseCell")
n = m.noise(piperine.NoiseConfig(out="out", fstart=1.0, fstop=1e7, points=200))
psd = n.psd(); plateau = psd.at(100.0)
assert 1.3e-17 < plateau < 2.0e-17, "plateau is 4kTR"
assert 1.8e-6 < n.total() < 2.2e-6, "total RMS is sqrt(kT/C)"
print("08_johnson_noise: PASS"); sys.stdout.flush()
