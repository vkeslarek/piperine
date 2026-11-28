//! The overlaps of the drives of one carrier, judged per instance (§5.2, §5.2.6, §8.2.20): two definitions that share an
//! element (`E0335`), a definition and a commit that share one (`E0221`), and an element handed to an `out` port that a
//! second `out` port or this module drives too (`E0220`). The checker judges every drive whose elements it can fold; a
//! drive whose element reads a generic length or a generation index, or one written inside generation (which exists in
//! the iterations and arms elaboration takes only, §8.2.23, §8.2.24), is judged here, where every index is folded and
//! every generation unrolled (`Types::defers_drive_overlaps`).

use piperine_ir::{CarrierWindow, Projection};

use super::elaborator::Elaboration;
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, FileId};

/// The elements of an array carrier (or the one value of a scalar carrier) a drive reaches, flat and row-major.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ElementReach {
    /// Every element: the whole carrier.
    Whole,
    /// The elements `[first, end)`.
    Run(u64, u64),
    /// An element decided at run time: the checker judged it against every other drive already.
    Decided,
}

impl ElementReach {
    /// The reach of a drive of the element `constant` of an array carrier, of the whole carrier when it names none, or of an
    /// element decided at run time when it names one that does not fold.
    pub(super) fn of_element(named: bool, constant: Option<u64>) -> ElementReach {
        match (named, constant) {
            (false, _) => ElementReach::Whole,
            (true, Some(at)) => ElementReach::Run(at, at + 1),
            (true, None) => ElementReach::Decided,
        }
    }

    /// The reach of the window an `out` port is given of a carrier (`None`: the whole carrier).
    pub(super) fn of_window(window: Option<CarrierWindow>) -> ElementReach {
        match window {
            None => ElementReach::Whole,
            Some(CarrierWindow { first, length: None }) => ElementReach::Run(first, first + 1),
            Some(CarrierWindow { first, length: Some(length) }) => ElementReach::Run(first, first + length),
        }
    }

    /// Whether two reaches share an element; one decided at run time is the checker's to judge.
    fn shares(self, other: ElementReach) -> bool {
        match (self, other) {
            (ElementReach::Decided, _) | (_, ElementReach::Decided) => false,
            (ElementReach::Run(first, end), ElementReach::Run(other_first, other_end)) => first < other_end && other_first < end,
            _ => true,
        }
    }
}

/// How a drive reaches its carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DriveWay {
    /// `place = value;` of this module.
    Definition,
    /// `place <- value;` of a reaction of this module.
    Commit,
    /// The carrier, or a window of it, handed to an `out` port of a child.
    Handed,
}

/// One drive of a carrier of the body, where it is written.
#[derive(Clone, Debug)]
pub(super) struct DriveSite {
    /// The carrier of the body.
    pub(super) carrier: u32,
    /// The elements it reaches.
    pub(super) reach: ElementReach,
    /// The fields below the element it names, outermost first: two drives of different fields compose (§8.2.20).
    pub(super) fields: Vec<u32>,
    /// How it drives.
    pub(super) way: DriveWay,
    /// The file and the span of its place.
    pub(super) file: FileId,
    pub(super) span: Span,
}

impl DriveSite {
    /// The field indexes of a place's projections, or `None` when one is an index into the element's value (such a part
    /// is the checker's to judge).
    pub(super) fn fields_of(path: &[Projection]) -> Option<Vec<u32>> {
        path.iter()
            .map(|step| match step {
                Projection::Field(index) => Some(*index),
                Projection::Index(_) => None,
            })
            .collect()
    }

    /// Whether this drive and `other` reach a common level of their carrier.
    fn shares(&self, other: &DriveSite) -> bool {
        let prefix = self.fields.iter().zip(&other.fields).all(|(mine, theirs)| mine == theirs);
        self.carrier == other.carrier && prefix && self.reach.shares(other.reach)
    }
}

impl Elaboration<'_, '_> {
    /// `E0335`, `E0221` and `E0220` for the carriers of the body whose drives the checker left to each instance, as the
    /// instance `instance` elaborates them: each drive is judged against the drives before it, in the order lowered.
    pub(super) fn judge_drive_overlaps(&mut self, instance: &str) {
        let mut deferred: Vec<(u32, DeclId)> = Vec::new();
        for frame in &self.state.structure.frames {
            for (decl, carrier) in &frame.carriers {
                if self.state.types.defers_drive_overlaps(*decl) && !deferred.iter().any(|(held, _)| held == carrier) {
                    deferred.push((*carrier, *decl));
                }
            }
        }
        let sites = self.state.digital.structure.drive_sites.clone();
        for (carrier, decl) in deferred {
            let of_carrier: Vec<&DriveSite> = sites.iter().filter(|site| site.carrier == carrier).collect();
            for (position, site) in of_carrier.iter().enumerate() {
                let code = of_carrier[..position].iter().filter(|earlier| earlier.shares(site)).find_map(|earlier| Self::conflict(earlier.way, site.way));
                if let Some(code) = code {
                    self.report_overlap(decl, site, code, instance);
                }
            }
        }
    }

    /// The code two drives of one element are, in the order written: none for two commits (a carrier is committed by any
    /// number of reactions, §12.1.2).
    fn conflict(earlier: DriveWay, later: DriveWay) -> Option<&'static str> {
        match (earlier, later) {
            (DriveWay::Commit, DriveWay::Commit) => None,
            (DriveWay::Definition, DriveWay::Definition) => Some("E0335"),
            (DriveWay::Handed, _) | (_, DriveWay::Handed) => Some("E0220"),
            _ => Some("E0221"),
        }
    }

    /// The diagnostic `code` at `site`, a drive of the carrier `decl` that shares an element with an earlier one.
    fn report_overlap(&mut self, decl: DeclId, site: &DriveSite, code: &'static str, instance: &str) {
        let name = self.shared_element_name(decl, site.reach);
        let (message, label) = match code {
            "E0335" => (format!("two definitions of `{name}` in `{instance}` share an element (§8.2.20)"), "defined twice"),
            "E0221" => (format!("`{name}` has a definition and a commit in `{instance}`: an element is driven by one or the other (§5.2, §8.2.20)"), "definition and commit"),
            _ => (format!("`{name}` is handed to an `out` port and driven a second time in `{instance}`: a one-driver carrier has one drive (§5.2.1, §5.2.6)"), "a second drive"),
        };
        self.report(site.file, Diagnostic::new(code, message, site.span, label));
    }

    /// The carrier `decl` as the element `reach` names when it is one element of an array (`g[1][1]`), else its name.
    pub(super) fn shared_element_name(&mut self, decl: DeclId, reach: ElementReach) -> String {
        // a field of an array of bundles names its element before the field (`lanes[1].a`, `pom/bundles.rs`)
        let naming = self.state.design.element_naming(decl);
        let ElementReach::Run(first, end) = reach else { return naming.name("") };
        let shape = self.state.consts.typed_carrier_array(decl).and_then(|array| self.shape_in(&array, 0));
        match shape {
            Some(shape) if end == first + 1 => naming.name(&shape.subscripts(first)),
            _ => naming.name(""),
        }
    }
}
