import os, sys, math, piperine
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

P = os.path.join(os.path.dirname(__file__), "22_rc_step_response.phdl")
m = piperine.load(P).module("RcStep")

# Step response: 1 V source, cap starts at 0 V, charges through R.
# tau = R*C = 1e3 * 100e-9 = 100e-6 s.  Simulate 6 tau.
TAU = 1e3 * 100e-9
t = m.tran(piperine.TranConfig(stop=6e-4, step=2e-6, ic={"out": 0.0}))
v = t.v("out", "gnd")

axis = np.array(v.axis)
vals = np.array(v.values)
analytic = 1.0 - np.exp(-axis / TAU)

fig, ax = plt.subplots(figsize=(10, 5.5))
ax.plot(axis * 1e6, vals, linewidth=2.2, color="#1f77b4", label="Piperine (TR-BDF2)")
ax.plot(axis * 1e6, analytic, linewidth=1.4, linestyle="--", color="#d62728",
        label=r"Analytic  $1 - e^{-t/\tau}$")
for k, col in [(1, "#2ca02c"), (2, "#9467bd"), (5, "#8c564b")]:
    ax.axvline(k * TAU * 1e6, color=col, linestyle=":", alpha=0.6)
    ax.annotate(f"{k}τ", xy=(k * TAU * 1e6, 0.06), fontsize=10, color=col,
                ha="center")
ax.axhline(1.0, color="gray", linestyle="-", alpha=0.3)
ax.axhline(0.6321, color="#2ca02c", linestyle=":", alpha=0.35)
ax.annotate("63.2% @ 1τ", xy=(1.5 * TAU * 1e6, 0.6321), fontsize=9, color="#2ca02c")

ax.set_xlabel("Time (µs)", fontsize=12)
ax.set_ylabel("V_out (V)", fontsize=12)
ax.set_title("RC Low-Pass Step Response (τ = 100 µs)", fontsize=14)
ax.legend(loc="lower right", fontsize=11)
ax.set_ylim(-0.02, 1.08)
ax.grid(True, alpha=0.3)
fig.tight_layout()

out = os.path.join(os.path.dirname(__file__), "22_rc_step_response.png")
fig.savefig(out, dpi=150)
print(f"Plot saved to {out}")
sys.stdout.flush()
