//! T54 (VAMS-21): every LRM ch. 12 VPI routine, grouped by ch. 11 §11.4's
//! ten functional categories, has a native Rust equivalent or a
//! documented, typed not-supported result — proved from outside the crate
//! (the `piperine_plugin` public surface), not just via `vpi.rs`'s own
//! unit tests.

use piperine_plugin::{VpiCategories, VpiCategory, VpiSupport};

const ALL_CATEGORIES: &[VpiCategory] = &[
    VpiCategory::SimulationCallbacks,
    VpiCategory::SystfCallbacks,
    VpiCategory::AnalogSystfCallbacks,
    VpiCategory::HdlHierarchyTraversal,
    VpiCategory::ObjectProperties,
    VpiCategory::ObjectsFromProperties,
    VpiCategory::DelayProcessing,
    VpiCategory::ValueProcessing,
    VpiCategory::TimeProcessing,
    VpiCategory::MiscUtilities,
]; // Table 11-1 through Table 11-10, §11.4.

#[test]
fn every_lrm_11_4_category_is_audited() {
    let audited: Vec<VpiCategory> = VpiCategories::all().iter().map(|e| e.category).collect();
    for cat in ALL_CATEGORIES {
        assert!(audited.contains(cat), "{cat:?} (LRM §11.4) has no VpiCategories entry");
    }
    assert_eq!(audited.len(), ALL_CATEGORIES.len(), "no category should be listed twice");
}

#[test]
fn every_category_carries_a_typed_support_level_and_named_mechanism() {
    for entry in VpiCategories::all() {
        // The support level is always one of the three typed variants —
        // there is no "unknown"/absent fourth state (C6: never silently
        // absent).
        match entry.support {
            VpiSupport::Native | VpiSupport::Partial | VpiSupport::NotSupported => {}
        }
        assert!(
            entry.mechanism.len() > 20,
            "{:?}'s mechanism text reads like a placeholder, not a real citation",
            entry.category
        );
    }
}

#[test]
fn systf_and_analog_systf_route_through_t53s_native_abi() {
    // These two categories (Table 11-2, Table 11-3) are the ones T53
    // built the actual dispatch mechanism for (PluginHost::
    // register_system_task/resolve_system_task/call_system_task) — confirm
    // they are the two categories marked fully Native, not merely claimed.
    assert_eq!(VpiCategories::support(VpiCategory::SystfCallbacks), VpiSupport::Native);
    assert_eq!(VpiCategories::support(VpiCategory::AnalogSystfCallbacks), VpiSupport::Native);
}

/// T59 (LRMC-C10-11): `SimulationCallbacks` (Table 11-1,
/// `vpi_register_cb`/`vpi_remove_cb`) flipped from `Partial` to `Native` —
/// `PluginHost::register_cb`/`remove_cb` is a real, event-grained,
/// deregisterable runtime mechanism now, proved separately in
/// `tests/callback_registry.rs`.
#[test]
fn simulation_callbacks_is_native_after_t59() {
    assert_eq!(VpiCategories::support(VpiCategory::SimulationCallbacks), VpiSupport::Native);
}

/// T60/T61 (LRMC-C10-12): `DelayProcessing` (Table 11-7,
/// `vpi_get_delays`/`vpi_put_delays`) flipped from `NotSupported` to
/// `Native` — proved separately in `tests/delay_processing.rs`.
#[test]
fn delay_processing_is_native_after_t60_t61() {
    assert_eq!(VpiCategories::support(VpiCategory::DelayProcessing), VpiSupport::Native);
}

#[test]
fn no_category_is_silently_dropped_from_the_public_surface() {
    // Regression guard for the exact defect C6 forbids: a category that
    // used to exist quietly disappearing from `VpiCategories::all()`
    // (e.g. during a future refactor) rather than being explicitly
    // reclassified.
    assert_eq!(VpiCategories::all().len(), 10, "LRM §11.4 defines exactly 10 categories");
}
