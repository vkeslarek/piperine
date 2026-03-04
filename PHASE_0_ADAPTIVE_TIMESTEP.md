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

### Phase 0.2: Capacitor Truncation
- [ ] Implement `TruncationError` for `CapacitorRuntime`
- [ ] Add charge calculation from state
- [ ] Implement divided differences algorithm

### Phase 0.3: Inductor Truncation
- [ ] Implement `TruncationError` for `InductorRuntime`
- [ ] Add flux calculation from state
- [ ] Reuse divided differences logic

### Phase 0.4: Adaptive Solver
- [ ] Modify `TransientSolver::solve()` for adaptive loop
- [ ] Add timestep acceptance/rejection logic
- [ ] Implement convergence-based retry

### Phase 0.5: Breakpoint System
- [ ] Implement `BreakpointProvider` for `VoltageSourceRuntime`
- [ ] Add breakpoint collection in solver
- [ ] Implement breakpoint-aware timestep limiting

### Phase 0.6: Context Updates
- [ ] Add `trtol`, `chgtol` to `Context`
- [ ] Set sensible defaults

### Phase 0.7: Options Updates
- [ ] Add `dt_initial`, `dt_min`, `dt_max` to `TransientAnalysisOptions`
- [ ] Add `adaptive` flag
- [ ] Maintain backward compatibility

### Phase 0.8: Testing & Validation
- [ ] Test adaptive vs fixed timestep on RC circuit
- [ ] Test breakpoint capture on pulse waveform
- [ ] Benchmark performance improvements
- [ ] Ensure all existing tests pass

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

**Current Phase:** 0.1 (Core Infrastructure) - IN PROGRESS
**Started:** 2025-01-XX
**Target Completion:** 2-3 weeks
