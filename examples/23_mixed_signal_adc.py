import os, sys, piperine
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

P = os.path.join(os.path.dirname(__file__), "23_mixed_signal_adc.phdl")
m = piperine.load(P).module("AdcDemo")

# Ramp 0 → 4 V over 10 µs; A2D samples the ramp each step, D2A reconstructs.
t = m.tran(piperine.TranConfig(stop=1e-5, step=2e-7))
axis = np.array(t.v("vin", "gnd").axis) * 1e6          # µs
vin  = np.array(t.v("vin", "gnd").values)              # analog ramp
rec  = np.array(t.v("rec", "gnd").values)              # analog reconstruction
d0   = np.array(t.v("d0").values)                      # digital bit (LSB)
d1   = np.array(t.v("d1").values)
d2   = np.array(t.v("d2").values)                      # digital bit (MSB)

fig, (ax_a, ax_d) = plt.subplots(2, 1, figsize=(11, 7), sharex=True,
                                 gridspec_kw={"height_ratios": [1.5, 1], "hspace": 0.12})

# ── Top: analog domain (ramp + reconstructed staircase) ───────────────────
ax_a.plot(axis, vin, linewidth=2.2, color="#1f77b4", label="vin  (analog ramp)")
ax_a.step(axis, rec, where="post", linewidth=2.0, color="#d62728",
          label="rec  (D2A reconstruction)")
for thr, col in [(1.0, "#2ca02c"), (2.0, "#9467bd"), (3.0, "#8c564b")]:
    ax_a.axhline(thr, color=col, linestyle=":", alpha=0.5)
    ax_a.annotate(f"{thr:.0f} V", xy=(axis[-1] * 0.99, thr + 0.05),
                  fontsize=8, color=col, ha="right")
ax_a.set_ylabel("Voltage (V)", fontsize=12)
ax_a.set_title("Mixed-Signal Transient — Ramp-Driven 3-Comparator ADC\n"
               "(A2D bridge: digital block reads analog · D2A bridge: analog block reads digital)",
               fontsize=12)
ax_a.legend(loc="upper left", fontsize=10)
ax_a.set_ylim(-0.2, 4.4)
ax_a.grid(True, alpha=0.3)

# ── Bottom: digital domain (comparator output bits) ───────────────────────
for i, (bit, name, col) in enumerate([(d2, "d2 (MSB, >3 V)", "#8c564b"),
                                      (d1, "d1      (>2 V)", "#9467bd"),
                                      (d0, "d0 (LSB, >1 V)", "#2ca02c")]):
    ax_d.step(axis, bit + i * 2.4, where="post", linewidth=2.0, color=col, label=name)
    ax_d.axhline(i * 2.4, color="gray", linewidth=0.5, alpha=0.3)
    ax_d.text(axis[0] - 0.15, i * 2.4 + 0.5, name, fontsize=9, color=col, va="center")
ax_d.set_ylabel("Logic level", fontsize=12)
ax_d.set_xlabel("Time (µs)", fontsize=12)
ax_d.set_yticks([0, 1, 2.4, 3.4, 4.8, 5.8])
ax_d.set_yticklabels(["0", "1", "0", "1", "0", "1"])
ax_d.set_ylim(-0.4, 6.6)
ax_d.grid(True, axis="x", alpha=0.3)

fig.tight_layout()
out = os.path.join(os.path.dirname(__file__), "23_mixed_signal_adc.png")
fig.savefig(out, dpi=150)
print(f"Plot saved to {out}")
sys.stdout.flush()
