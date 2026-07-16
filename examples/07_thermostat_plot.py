import os, sys, piperine
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

P = os.path.join(os.path.dirname(__file__), "07_thermostat.phdl")
m = piperine.load(P).module("Thermostat")

t = m.tran(piperine.TranConfig(stop=0.2, step=1e-4))
temp = t.v("temp", "gnd")

axis = np.array(temp.axis)
vals = np.array(temp.values)

fig, ax = plt.subplots(figsize=(10, 5))
ax.plot(axis * 1000, vals, linewidth=1.5, color="#d62728", label="Temperatura")
ax.axhline(25.0, color="gray", linestyle="--", alpha=0.7, label="Threshold ON (25 °C)")
ax.axhline(22.0, color="blue", linestyle="--", alpha=0.5, label="Threshold OFF (22 °C)")
ax.axhline(20.0, color="green", linestyle=":", alpha=0.4, label="Ambiente (20 °C)")
ax.fill_between(axis * 1000, 22, 25, alpha=0.08, color="orange")

ax.set_xlabel("Tempo (ms)", fontsize=12)
ax.set_ylabel("Temperatura (°C)", fontsize=12)
ax.set_title("Termostato Bang-Bang — Ciclo Limite com Histerese", fontsize=14)
ax.legend(loc="lower right", fontsize=10)
ax.set_ylim(19, 27)
ax.grid(True, alpha=0.3)
fig.tight_layout()

out = os.path.join(os.path.dirname(__file__), "07_thermostat_plot.png")
fig.savefig(out, dpi=150)
print(f"Plot salvo em {out}")
