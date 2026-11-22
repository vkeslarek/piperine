# Moved mixed-signal values (D12)

Each value of a mixed-signal golden that a task of this feature moves, with the old value, the new
value and the requirement (TP-03, TP-04 or TP-05; D12 was widened to TP-03 by the orchestrator at the B3 report: a listing whose rows move only because removed landings changed the time grid, with equal values at the shared rows and equal records). No record and no digital column is listed.

| Task | Golden | Old | New | Requirement |
| ---- | ------ | --- | --- | ----------- |

T9 (TP-04): none. `book_mixed`, the parity targets, the twins and `parity_baseline` pass with their
sources unedited; `buck_long` keeps every limit (largest `vout` 0.808723 V, settling inside
±10 mV from 263.74 µs, ripple 1.270 mV, reference 128, mode Regulate, `pgood` true).

T10 (TP-05): no pinned golden moves (`book_mixed`, the parity targets, the twins and `parity_baseline` pass with their sources unedited). The buck's measured quantities of `buck_long`, which are limits and not pinned values, changed as the step that reads a jump is now error-controlled; every limit still holds:

| Quantity (600 µs start) | Old (T9) | New (T10) | Limit | Requirement |
| ----------------------- | -------- | --------- | ----- | ----------- |
| largest `buck.vout` | 0.808723 V at 288.41 µs | 0.807500 V at 284.44 µs | 0.815 V | TP-05 |
| settling inside ±10 mV from | 263.74 µs | 262.87 µs | 207.5 to 307.5 µs | TP-05 |
| ripple over the last 40 µs | 1.270 mV | 1.486 mV | 10 mV | TP-05 |

B3 (T16, the observable rule becomes the default; the cause is TP-02/TP-03, which D12 does not list, so the record names the real cause): parity targets, twins and `parity_baseline` pass with their sources unedited, and every ngspice twin was compared (no SKIP outside the deliberate skip-line tests). Two mixed-signal values moved:

| Task | Golden | Old | New | Requirement |
| ---- | ------ | --- | --- | ----------- |
| T16 | `docs/book/content.html`, listing `piperine run monitor.msl --analysis transient --stop 3e-6 --stream zone` (project `monitor`) | the print rows of the 1 us to 1.98 us and 1.5 us to 2 us spans lie on the grid that lands on the 1 us and 2 us clock edges (rows 1.000000e-6, 1.030000e-6, ..., 1.530000e-6 ...) | the same rows lie on the controller's own grid (1.006484e-6, 1.036484e-6, ..., 1.520273e-6 ...); 66 rows differ in time and in the interpolated analog column `x`; every `clk` and `zone` value at a row both listings share is equal, and the three `zone` records are equal | TP-03 (no landing at a clock edge nothing observes), not TP-04/TP-05 |
| T17 | `buck_long` limits (largest `buck.vout`, settling, ripple) | 0.807500 V at 284.44 us; inside 10 mV from 262.87 us; ripple 1.486 mV | 0.807500 V at 284.44 us; 262.87 us; ripple 1.494 mV | TP-03; every limit still holds |

B4 (T22 no instant at an idle step, T25 the tick epoch, T18 to T21 per-step cost): no mixed-signal golden moves. `book_mixed`, `book_buck`, the parity targets, the twins and `parity_baseline` pass with their sources unedited; `buck_long` keeps its 210 188 steps and every limit (ripple 1.494 mV, largest `vout` 0.807500 V at 284.44 us, settling from 262.87 us) and `buck_accuracy` its 0.000136 mV.
