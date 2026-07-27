import os, sys, math, piperine
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

P = os.path.join(os.path.dirname(__file__), "08_johnson_noise.phdl")
m = piperine.load(P).module("NoiseCell")

# R=1k, C=1n, T=300K.  Plateau 4kTR ≈ 1.657e-17 V²/Hz (4.07 nV/√Hz),
# corner fc = 1/(2π·R·C) ≈ 159 kHz, integrated RMS = √(kT/C) ≈ 2.03 µV.
n = m.noise(piperine.NoiseConfig(out="out", fstart=1.0, fstop=1e7, points=200))
psd = n.psd()                       # V²/Hz waveform over frequency
freqs = np.array(psd.axis)
psd_v2 = np.array(psd.values)       # V²/Hz
psd_vrthz = np.sqrt(psd_v2)         # V/√Hz
total_rms = n.total()               # V (RMS)

FC = 1.0 / (2 * math.pi * 1e3 * 1e-9)
PLATEAU = math.sqrt(1.657e-17)

# ── PNG (log-log, nV/√Hz) ────────────────────────────────────────────────
fig, ax = plt.subplots(figsize=(10, 5.5))
ax.loglog(freqs, psd_vrthz * 1e9, linewidth=2.2, color="#1f77b4", label="Piperine (adjoint)")
ax.axhline(PLATEAU * 1e9, color="#d62728", linestyle="--", alpha=0.7,
           label=f"Plateau √(4kTR) = {PLATEAU*1e9:.2f} nV/√Hz")
ax.axvline(FC, color="#2ca02c", linestyle=":", alpha=0.7, label=f"fc = {FC/1e3:.0f} kHz")
ax.fill_between(freqs, 1e-3, psd_vrthz * 1e9, where=(freqs <= FC), alpha=0.06, color="#1f77b4")
ax.annotate(f"Integrated noise = {total_rms*1e6:.2f} µVrms\n(√(kT/C))",
            xy=(FC*0.02, PLATEAU*1e9*0.4), fontsize=10, color="#333333",
            bbox=dict(boxstyle="round,pad=0.4", fc="#fff3cd", ec="#cca700"))
ax.set_xlabel("Frequency (Hz)", fontsize=12)
ax.set_ylabel("Spectral density (nV/√Hz)", fontsize=12)
ax.set_title("Johnson–Nyquist Noise — 1 kΩ Resistor ‖ 1 nF  (T = 300 K)", fontsize=13)
ax.legend(loc="lower left", fontsize=10)
ax.set_ylim(0.01, 20)
ax.grid(True, which="both", alpha=0.25)
fig.tight_layout()
out = os.path.join(os.path.dirname(__file__), "08_johnson_noise_plot.png")
fig.savefig(out, dpi=150)
print(f"Plot saved to {out}\n")

# ── ASCII log-log plot in the terminal ───────────────────────────────────
print("Johnson noise — PSD (nV/√Hz) vs freq (log)   █ = Piperine   ┊ = √(4kTR)")
print("─" * 72)
W = 60
def bar(v):
    # log scale 0.01 .. 20 nV/√Hz  → 0..W
    lo, hi = math.log10(0.01), math.log10(20.0)
    x = (math.log10(max(v, 1e-12)) - lo) / (hi - lo)
    return max(0, min(W, int(round(x * W))))
for i in range(0, len(freqs), 8):
    f = freqs[i]
    v = psd_vrthz[i] * 1e9
    line = "█" * bar(v)
    marker = " ┊ plateau" if abs(math.log10(v) - math.log10(PLATEAU*1e9)) < 0.15 else ""
    print(f"{f:>10.0f} Hz │{line:<{W}}│ {v:6.2f} nV/√Hz{marker}")
print("─" * 72)
print(f"  Plateau √(4kTR) = {PLATEAU*1e9:.2f} nV/√Hz   |   fc = {FC/1e3:.0f} kHz")
print(f"  Integrated noise = {total_rms*1e6:.3f} µVrms   (theoretical √(kT/C) = {math.sqrt(1.38e-23*300/1e-9)*1e6:.3f} µV)")
sys.stdout.flush()
