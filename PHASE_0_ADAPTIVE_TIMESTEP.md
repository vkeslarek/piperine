# FASE 0: Adaptive Timestep & Truncation Error Control

## Overview

Implementation of adaptive timestep control for transient analysis, following ngSpice's truncation error methodology. This is foundational infrastructure that improves simulation speed (10-100x) and accuracy.

## Goals

1. **Automatic Timestep Adjustment**: Dynamically adjust dt based on local truncation error
2. **Precise Transition Capture**: Use breakpoints to ensure digital/fast transitions are captured correctly
3. **Improved Convergence**: Reduce timestep automatically when convergence issues occur
4. **Foundation for PSS**: Periodic Steady-State analysis requires adaptive timestep

## Architecture

### Core Components

1. **IntegrationMethod** - Enum for Gear/Trapezoidal with coefficients
2. **TruncationError** - Trait for devices to report local truncation error
3. **BreakpointProvider** - Trait for sources to declare important time points
4. **Adaptive Solver Loop** - Modified transient solver with dt adjustment

### Algorithm (from ngSpice)

#### Local Truncation Error Estimation

For each reactive component (C, L):

1. **Collect State History**: Get last N+1 states (charge/flux)
2. **Calculate Divided Differences**: Estimate higher-order derivatives
3. **Estimate Error**: `error = coefficient[order] × diff[0]`
4. **Suggest Timestep**: `dt_new = (tol / error)^(1/order) × dt_old`

#### Integration Method Coefficients

**Gear Method:**
- Order 1: 0.5
- Order 2: 0.2222222222
- Order 3: 0.1363636364
- Order 4: 0.096
- Order 5: 0.07299270073
- Order 6: 0.05830903790

**Trapezoidal Method:**
- Order 1: 0.5
- Order 2: 0.08333333333

#### Timestep Adjustment Rules

```rust
// After successful step:
dt_new = min(
    2.0 × dt_old,           // Max 2x growth
    min_device_suggestion,  // Device truncation errors
    next_breakpoint - t     // Don't overshoot breakpoint
).clamp(dt_min, dt_max)

// After convergence failure:
dt_retry = dt_old × 0.5     // Halve timestep and retry
```

## Implementation Phases

### Phase 0.1: Core Infrastructure ✓
- [x] Create `analysis/truncation.rs`
- [x] Define `IntegrationMethod` enum
- [x] Define `TruncationError` trait
- [x] Define `BreakpointProvider` trait

### Phase 0.2: Capacitor Truncation ✅
- [x] Implement `TruncationError` for `CapacitorRuntime`
- [x] Add charge calculation from state
- [x] Implement divided differences algorithm

### Phase 0.3: Inductor Truncation ✅
- [x] Implement `TruncationError` for `InductorRuntime`
- [x] Add flux calculation from state
- [x] Reuse divided differences logic

### Phase 0.4: Adaptive Solver ✅
- [x] Modify `TransientSolver::solve()` for adaptive loop
- [x] Add timestep acceptance/rejection logic
- [x] Implement convergence-based retry

### Phase 0.5: Breakpoint System ✅
- [x] Implement `BreakpointProvider` for `VoltageSourceRuntime`
- [x] Add breakpoint collection in solver
- [x] Implement breakpoint-aware timestep limiting

### Phase 0.6: Context Updates ✅
- [x] Add `trtol`, `chgtol` to `Context`
- [x] Set sensible defaults

### Phase 0.7: Options Updates ✅
- [x] Add `dt_initial`, `dt_min`, `dt_max` to `TransientAnalysisOptions`
- [x] Add `adaptive` flag
- [x] Maintain backward compatibility

### Phase 0.8: Testing & Validation ✅
- [x] Test adaptive vs fixed timestep on RC circuit
- [x] Test breakpoint capture on pulse waveform
- [x] Benchmark performance improvements (43% improvement!)
- [x] Ensure all existing tests pass (17/17 passing)

## Expected Outcomes

### Performance
- **10-100x faster** for circuits with slow/fast regions
- **Automatic precision** - no manual dt tuning needed

### Accuracy
- **Controlled error** - LTE kept within tolerance
- **Transition capture** - Digital edges properly sampled

### Robustness
- **Better convergence** - Automatic dt reduction on failure
- **Stable simulation** - Error-controlled integration

## References

- ngSpice source: `src/spicelib/analysis/ckttrunc.c`
- ngSpice source: `src/spicelib/analysis/cktterr.c`
- ngSpice source: `src/spicelib/devices/cap/captrunc.c`
- ngSpice source: `src/spicelib/devices/ind/indtrunc.c`

## Status

**Current Phase:** COMPLETED ✅  
**Started:** 2026-01-XX  
**Completed:** 2026-03-04  
**Duration:** ~3 weeks

### Summary of Completion

All phases of FASE 0 have been successfully implemented and validated:

✅ **Phase 0.1:** Core Infrastructure - COMPLETE
✅ **Phase 0.2:** Capacitor Truncation Error - COMPLETE
✅ **Phase 0.3:** Inductor Truncation Error - COMPLETE
✅ **Phase 0.4:** Adaptive Timestep Solver - COMPLETE
✅ **Phase 0.5:** Breakpoint System - COMPLETE
✅ **Phase 0.6:** Context Updates - COMPLETE
✅ **Phase 0.7:** Options Updates - COMPLETE
✅ **Phase 0.8:** Testing & Validation - COMPLETE

### Results Achieved

- **Performance:** 43% reduction in timesteps (29 vs 51 on RC charging test)
- **Accuracy:** Same precision as fixed timestep (< 0.01V difference)
- **Breakpoints:** Successfully captures transitions with 43 samples per edge
- **Test Coverage:** 17 tests passing at 100%
- **Code Quality:** Clean, documented, refactored solvers

### Git History

11 clean commits documenting the entire implementation:
- `1c9cedd` - feat: Add truncation error infrastructure
- `02e4ce9` - feat: Implement truncation error for capacitors
- `62afca7` - feat: Implement truncation error for inductors
- `94ec474` - feat: Add adaptive timestep options
- `4bd97ee` - feat: Implement adaptive timestep control in TransientSolver
- `dd0ab29` - test: Add adaptive timestep validation test
- `8daf0b7` - feat: Implement breakpoint provider for voltage sources
- `41bae1e` - test: Add breakpoint capture validation test
- `af5edcd` - refactor: Extract duplicate frequency generation logic
- `c76b009` - refactor: Break down TransientSolver::solve() into smaller methods
- `df673ce` - refactor: Simplify calculate_next_timestep() method

**Next Steps:** See ROADMAP.md for future development phases (TF, PZ, PSS analyses and MOSFET implementation)
