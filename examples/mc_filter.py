"""Monte Carlo filter example — Python testbench using the Piperine bridge.

Hardware: hardware/lpf.ppr (R=1kΩ, C=100nF, fc≈1590 Hz)
Run:      python examples/mc_filter.py   (after `maturin develop`)
"""

import math, numpy as np
import piperine
from piperine import NgspiceSession, join_all

N_RUNS   = 200
N_WORKERS = 8   # parallel ngspice sessions

def bandwidth_3db(freq: np.ndarray, gain: np.ndarray) -> float:
    """Return the -3dB frequency interpolated from AC sweep data."""
    gain_db = 20 * np.log10(np.abs(gain) / gain[0])
    idx = np.searchsorted(-gain_db, 3.0)
    if idx == 0 or idx >= len(freq):
        return float("nan")
    f_lo, f_hi = freq[idx - 1], freq[idx]
    g_lo, g_hi = gain_db[idx - 1], gain_db[idx]
    return float(f_lo + (f_hi - f_lo) * (3.0 - g_lo) / (g_hi - g_lo))

def run_one(sess: NgspiceSession, rng: np.random.Generator) -> float:
    lot = rng.standard_normal()
    r_val = rng.standard_normal() * (30.0 / 3.0) + 1000.0
    r_val *= 1.0 + 0.02 * lot
    c_val = rng.standard_normal() * (5e-9 / 3.0) + 100e-9
    c_val *= 1.0 + 0.01 * lot

    sess.alter("R1", "resistance", r_val)
    sess.alter("C1", "capacitance", c_val)
    r = sess.ac("dec", 50, 100.0, 100e3)
    return bandwidth_3db(r["frequency"], r["v(out).re"])

def main():
    rng = np.random.default_rng(42)

    # Spawn N_WORKERS sessions
    sessions = [NgspiceSession.from_file("hardware/lpf.ppr", module="lpf")
                for _ in range(N_WORKERS)]

    fc_samples = []
    for batch_start in range(0, N_RUNS, N_WORKERS):
        batch = sessions[:min(N_WORKERS, N_RUNS - batch_start)]
        futures = [s.tran_async("1n", f"{rng.random():.3e}") for s in batch]

        # Launch parallel AC sweeps (tran was just a placeholder — use ac_async when added)
        fc_vals = [run_one(s, rng) for s in batch]
        fc_samples.extend(fc_vals)

    arr = np.array(fc_samples)
    print(f"MC results ({N_RUNS} runs)")
    print(f"  Mean fc   = {arr.mean():.1f} Hz")
    print(f"  Sigma fc  = {arr.std(ddof=1):.1f} Hz")
    print(f"  5th pct   = {np.percentile(arr, 5):.1f} Hz")
    print(f"  95th pct  = {np.percentile(arr, 95):.1f} Hz")
    print(f"  Yield <2k = {(arr < 2000).mean() * 100:.1f}%")

if __name__ == "__main__":
    main()
