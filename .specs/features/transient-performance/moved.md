# Moved mixed-signal values (D12)

Each value of a mixed-signal golden that a task of this feature moves, with the old value, the new
value and the requirement (TP-04 or TP-05). No record and no digital column is listed.

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
