//! The element coverage of a carrier the checker could not judge (§3.3.4, §5.2.6, §8.2.20): a generic length
//! (`[Bit; N]`) or a param length (`[Bit; n]`) is known once the instance is, and a drive written inside generation exists
//! in the iterations and arms elaboration takes only (§8.2.23, §8.2.24), so the elements no driver names are judged here,
//! per instance, where lengths, generation indices and generation arms are folded; a scalar carrier is one element. The
//! checker judged the rest itself and handed these over (`Types::defers_missing_initial_value`,
//! `Types::defers_handed_initial_value`, `Types::defers_defined_initial_value`).

use piperine_ir::{CarrierWindow, Drive};

use super::elaborator::Elaboration;
use crate::msl::Diagnostic;
use crate::pom::DeclId;
use crate::types::{ElementCoverage, Ty};

impl Elaboration<'_, '_> {
    /// `E0187` and `E0223` for the arrays of carriers of `module`, as the instance `instance` of the body has them: an
    /// element no undelayed definition and no `out` port of a child names, with no initial value, is `E0187` naming
    /// it; an initial value over elements every `out` port holds is `E0223`.
    pub(super) fn judge_element_coverage(&mut self, module: DeclId, instance: &str) {
        // an item refused inside generation is the one diagnostic: it names elements the refusal dropped, no cascade
        if !self.generated.refused.is_empty() {
            return;
        }
        // the carriers declared inside generation are the body's too, one array each (§8.2.23)
        let mut children = self.design.decl_data(module).children.clone();
        children.extend(self.generated.elaborated());
        for decl in children {
            let missing = self.types.defers_missing_initial_value(decl);
            let handed = self.types.defers_handed_initial_value(decl);
            let defined = self.types.defers_defined_initial_value(decl);
            if !missing && !handed && !defined {
                continue;
            }
            // a scalar carrier is one element: a drive written inside generation is judged here as well
            let (Some(carrier), Some(array)) = (self.frames[0].carriers.get(&decl).copied(), self.consts.typed_carrier(decl)) else { continue };
            let Some(shape) = self.shape_in(&array, 0) else { continue };
            let total = shape.count();
            if missing {
                self.judge_missing_initial_value(decl, carrier, &array, total, instance);
            }
            if handed {
                self.judge_handed_initial_value(decl, carrier, total, instance);
            }
            if defined {
                self.judge_defined_initial_value(decl, carrier, total, instance);
            }
        }
    }

    /// `E0187` naming the elements of the array `decl` (carrier `carrier` of the body, `total` elements) that no driver names.
    fn judge_missing_initial_value(&mut self, decl: DeclId, carrier: u32, array: &Ty, total: u64, instance: &str) {
        let mut coverage = self.handed_coverage(carrier, total);
        self.cover_definitions(&mut coverage, carrier);
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let Some(unnamed) = coverage.unnamed(&name) else { return };
        let Ty::Carrier { value, .. } = array.innermost() else { return };
        let found = value.name(self.design);
        let message = format!("`{unnamed}` in `{instance}` has no initial value and its type `{found}` does not implement `Default`: a carrier has a value from (0, 0) on (§3.3.4)");
        let help = format!("write `= value` after the type of `{name}`, or drive every element");
        self.report(data.file, Diagnostic::new("E0187", message, data.span, "no initial value").with_help(help));
    }

    /// `E0223` for the array `decl` when `out` ports of children hold every one of its `total` elements.
    fn judge_handed_initial_value(&mut self, decl: DeclId, carrier: u32, total: u64, instance: &str) {
        if !self.handed_coverage(carrier, total).is_complete() {
            return;
        }
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = format!("`{name}` has an initial value and every element is handed to an `out` port in `{instance}`: the instance holds the drive, so the declaring module may give none (§5.2.6)");
        self.report(data.file, Diagnostic::new("E0223", message, data.span, "initial value without the drive"));
    }

    /// `E0338` for the array `decl` when undelayed definitions name every one of its `total` elements: the initial value
    /// would apply to none (§8.2.21). With an element left, the initial value is that element's.
    fn judge_defined_initial_value(&mut self, decl: DeclId, carrier: u32, total: u64, instance: &str) {
        let mut coverage = ElementCoverage::counted(total);
        self.cover_definitions(&mut coverage, carrier);
        if !coverage.is_complete() {
            return;
        }
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = format!("`{name}` has an initial value and an undelayed definition of every element in `{instance}`: the definitions would replace it in the microstep that commits them (§8.2.21)");
        self.report(data.file, Diagnostic::new("E0338", message, data.span, "initial value under an undelayed definition"));
    }

    /// The elements of carrier `carrier` that an undelayed definition names.
    fn cover_definitions(&self, coverage: &mut ElementCoverage, carrier: u32) {
        for drive in self.structure.drives.iter().filter(|drive| drive.carrier == carrier && drive.after.is_none()) {
            Self::cover_drive(coverage, drive);
        }
    }

    /// The elements of carrier `carrier` (`total` of them) that children hold at an `out` port.
    fn handed_coverage(&self, carrier: u32, total: u64) -> ElementCoverage {
        let mut coverage = ElementCoverage::counted(total);
        for (_, window) in self.structure.handed.iter().filter(|(handed, _)| *handed == carrier) {
            match window {
                None => coverage.cover_all(),
                Some(CarrierWindow { first, length: None }) => coverage.cover_run(*first, first + 1),
                Some(CarrierWindow { first, length: Some(length) }) => coverage.cover_run(*first, first + length),
            }
        }
        coverage
    }

    /// An undelayed definition names its element, or the whole carrier when it names none.
    fn cover_drive(coverage: &mut ElementCoverage, drive: &Drive) {
        match drive.element {
            Some(element) => coverage.cover_run(element, element + 1),
            None => coverage.cover_all(),
        }
    }
}
