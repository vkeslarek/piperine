//! T47 (VAMS-15) — LRM chapter 9 general system tasks/functions, exercised
//! through `piperine_lang`'s public surface (`piperine_lang::eval::tasks`).
//! Mirrors T47's "Done when": the `$display`-family formats per LRM Table
//! 9-1, `$abstime`/`$temperature`/`$bound_step` execute against a supplied
//! [`SimEnvironment`], the §9.13 conversion functions round-trip, and an
//! unknown `$name` fails loud naming near matches.

use piperine_lang::eval::tasks::{Conversions, SimEnvironment, SystemTaskError, SystemTaskKind, SystemTasks};
use piperine_lang::eval::{Value, format_display};

#[test]
fn display_family_names_all_resolve() {
    for name in ["$write", "$strobe", "$display", "$monitor"] {
        assert!(SystemTasks::resolve(name).is_ok(), "{name} must resolve");
    }
}

#[test]
fn diagnostic_task_names_all_resolve() {
    for name in ["$info", "$warning", "$error", "$fatal", "$finish", "$stop", "$discontinuity"] {
        assert!(SystemTasks::resolve(name).is_ok(), "{name} must resolve");
    }
}

#[test]
fn display_format_executes_per_lrm_table_9_1() {
    let out = format_display(
        "v=%d ns=%s h=%h",
        &[Value::Int(-3), Value::Str("ok".into()), Value::Int(255)],
        "top.dut",
    )
    .unwrap();
    assert_eq!(out, "v=-3 ns=ok h=ff");
}

#[test]
fn bound_step_resolves_and_executes() {
    assert_eq!(SystemTasks::resolve("$bound_step").unwrap(), SystemTaskKind::BoundStep);
    let env = SimEnvironment::new(0.0, 300.0, 1e-9);
    assert_eq!(env.bound_step(5e-9).unwrap(), 1e-9);
}

#[test]
fn abstime_and_temperature_execute() {
    assert_eq!(SystemTasks::resolve("$abstime").unwrap(), SystemTaskKind::Abstime);
    assert_eq!(SystemTasks::resolve("$temperature").unwrap(), SystemTaskKind::Temperature);
    let env = SimEnvironment::new(2.5e-3, 273.15, 1e-6);
    assert_eq!(env.abstime(), Value::Real(2.5e-3));
    assert_eq!(env.temperature(), Value::Real(273.15));
}

#[test]
fn conversion_functions_execute() {
    assert_eq!(Conversions::itor(&Value::Int(5)).unwrap(), Value::Real(5.0));
    assert_eq!(Conversions::rtoi(&Value::Real(5.9)).unwrap(), Value::Int(5));
    assert_eq!(Conversions::unsigned(&Value::Int(-2)).unwrap().as_natural(), Some(u64::MAX - 1));
    let bits = Conversions::realtobits(&Value::Real(3.25)).unwrap();
    assert_eq!(Conversions::bitstoreal(&bits).unwrap(), Value::Real(3.25));
}

#[test]
fn unknown_system_task_fails_loud_listing_near_matches() {
    let err = SystemTasks::resolve("$displya").unwrap_err();
    let SystemTaskError::Unknown { name, suggestions, .. } = &err else {
        panic!("expected Unknown, got {err:?}");
    };
    assert_eq!(name, "$displya");
    assert!(suggestions.iter().any(|s| s == "$display"), "{suggestions:?}");
    // E-STD: diagnostic code + help text present.
    let rendered = format!("{err}");
    assert!(rendered.contains("$displya"));
}
