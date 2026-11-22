# Moved mixed-signal values (D12)

Each value of a mixed-signal golden that a task of this feature moves, with the old value, the new
value and the requirement (TP-04 or TP-05). No record and no digital column is listed.

| Task | Golden | Old | New | Requirement |
| ---- | ------ | --- | --- | ----------- |

T9 (TP-04): none. `book_mixed`, the parity targets, the twins and `parity_baseline` pass with their
sources unedited; `buck_long` keeps every limit (largest `vout` 0.808723 V, settling inside
±10 mV from 263.74 µs, ripple 1.270 mV, reference 128, mode Regulate, `pgood` true).
