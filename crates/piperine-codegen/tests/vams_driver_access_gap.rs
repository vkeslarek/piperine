//! LRM-COVERS: 7.9, 9.22, 9.22.1, 9.22.5, 9.22.6
//! lrm-conformance-wave3 (Phase 6, T38): §9.22.1's own return-value
//! contract (a driver count reflecting the live number of drivers) is
//! `the_driver_count_data_source_is_live_for_a_real_multiply_driven_net`
//! below (count == 2 for two scheduled drivers). §9.22.5 ("Receiver net
//! resolution" — "By not specifying the receiver value directly in the
//! connect module driver, receiver segregation will be ignored, which is
//! the default case") and §9.22.6 ("Connect module example using driver
//! access functions") are both descriptive/example sections, not
//! standalone callables: this file's own `bridge` connectmodule fixture
//! (`connectmodule bridge (inout logic a, inout electrical b)`) is
//! exactly their subject — a mixed digital/electrical connectmodule port
//! calling a `$driver_*` function, with no `assign` statement overriding
//! the receiver value (§9.22.5's own "default case").
//! LRMC-C7-05 (T35, updated by T54/LRMW2-46): the seven `$driver_*`
//! functions parse and elaborate (proven in `piperine-lang`'s own
//! `vams_system_tasks.rs`). T54 closed the "nothing populates
//! `DriverTable`" half of the gap this target used to document in full:
//! `piperine_solver::digital::scheduler` now calls
//! `DriverTable::set_driver` for every digital device that drives a net,
//! so `$driver_count`/`$driver_state`/`$driver_strength`'s *data source*
//! is live (proven end to end in `piperine-solver`'s own
//! `tests/digital_driver_segregation.rs`).
//!
//! What remains unwired (SPEC_DEVIATION, disclosed, not this task's
//! `Where`): reaching that live table from *inside a JIT-compiled
//! `connectmodule` analog body* needs a `piperine_solver::digital::
//! DriverTable` pointer threaded through `piperine-codegen`'s analog JIT
//! ABI (`emit::abi::SimCtx`, the same way `io`/`SimOutput` already is) —
//! a codegen-side ABI extension. This target now proves two things
//! together: the CLAUDE.md-mandated fail-loud behavior for that
//! still-open ABI gap (compiling a `connectmodule` body that calls
//! `$driver_count` fails loud, naming the call, instead of silently
//! compiling to a wrong always-zero result), AND that the underlying
//! data source it would eventually read from is no longer the "nothing
//! ever writes to this table" gap that made T35's own wiring
//! semantically untestable.

use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, DigitalState, LogicValue, Region};

const SRC: &str = r#"
    connectmodule bridge (inout logic a, inout electrical b);
        integer n;
        analog begin
            n = $driver_count(a);
        end
    endmodule

    discipline logic;
        domain discrete;
    enddiscipline
"#;

#[test]
fn driver_count_fails_loud_at_compile_instead_of_silently_returning_zero() {
    let design = parse_and_elaborate(SRC, &piperine_lang::SourceMap::dummy()).expect("bridge elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let Err(err) = piperine_codegen::kernel::analog::AnalogKernel::compile(&bodies["bridge"]) else {
        panic!("$driver_count must fail loud at compile time, not silently JIT to a wrong value");
    };
    let msg = format!("{err}").to_lowercase();
    assert!(msg.contains("driver_count"), "error should name the unrecognized syscall, got: {msg}");
}

/// T54: the data source `$driver_count` is specified against
/// (§9.22.1) is no longer permanently empty — a real multiply-driven
/// net, evaluated through `piperine-solver`'s live scheduler (not a
/// mock), reports real per-driver records. This is the fix half of the
/// gap the test above still documents on the JIT-ABI side.
#[test]
fn the_driver_count_data_source_is_live_for_a_real_multiply_driven_net() {
    let mut state = DigitalState::new(1);
    let net = DigitalNet(0);
    state.schedule(DigitalEvent { time: 1e-9, net, value: LogicValue::One, source: 0, seq: 0, region: Region::Active });
    state.schedule(DigitalEvent { time: 1e-9, net, value: LogicValue::Z, source: 1, seq: 1, region: Region::Active });
    state.evaluate_until_stable(1e-9, &mut [], Default::default(), &[]).expect("settles");

    assert_eq!(
        state.driver_table.driver_count(net),
        2,
        "DriverTable::driver_count -- $driver_count's data source -- must reflect both live drivers"
    );
    assert_eq!(state.nets[0], LogicValue::One, "the Strong driver wins over the HighZ one, per Table 7-3");
}
