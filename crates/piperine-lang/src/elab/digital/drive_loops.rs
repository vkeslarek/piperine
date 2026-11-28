//! Loops of definitions through elements (part III §8.2.3): a drive that depends on itself with no reaction and no
//! `after` in between is a loop, and one that passes through a one-driver carrier is an error, `E0339` naming its
//! drives. The checker finds the loops among definitions of whole carriers; a definition of an element, one written in
//! generation, and a read at a run-time element are known only here, where every element is folded: the drives of the
//! body form a graph (a drive reaches the drives that read what it writes), and every cycle through a one-driver carrier
//! is reported, once, at its first drive, naming each element (`b[1]`, `b[2]`) as the source writes it.

use super::DigitalLowering;
use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::{Expr, ExprId as LoweredExpr};

use super::drive_overlaps::ElementReach;
use crate::msl::Diagnostic;
use crate::pom::DeclId;

/// What one drive writes or reads of one carrier of the body: an element, or the whole carrier (a read at a run-time
/// element may be any of them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CarrierPart {
    /// The carrier of the body.
    carrier: u32,
    /// The element; `None` for the whole carrier.
    element: Option<u64>,
}

impl CarrierPart {
    /// Whether a write of this part reaches a read of `read`.
    fn feeds(self, read: CarrierPart) -> bool {
        self.carrier == read.carrier && (self.element.is_none() || read.element.is_none() || self.element == read.element)
    }
}

impl DigitalLowering<'_, '_, '_> {
    /// `E0339` for each loop of the undelayed drives of the body through a one-driver carrier, as the instance `instance`
    /// elaborates them. A loop whose drives the checker judged already (whole carriers only) never reaches here.
    pub(in crate::elab) fn judge_drive_loops(&mut self, instance: &str) {
        let drives: Vec<(usize, CarrierPart, BTreeSet<CarrierPart>)> = self
            .state.digital.structure
            .drives
            .iter()
            .enumerate()
            .filter(|(_, drive)| drive.after.is_none())
            .map(|(k, drive)| (k, CarrierPart { carrier: drive.carrier, element: drive.element }, self.parts_read(drive.value)))
            .collect();
        let one_driver = self.one_driver_carriers();
        let successors: Vec<Vec<usize>> = drives
            .iter()
.map(|(_, written, _)| (0..drives.len()).filter(|to| drives[*to].2.iter().any(|read| written.feeds(*read))).collect())
            .collect();
        let mut reported = BTreeSet::new();
        for start in 0..drives.len() {
            if reported.contains(&start) {
                continue;
            }
            let ahead = Self::reachable(&successors, start);
            if !ahead.contains(&start) {
                continue;
            }
            let cycle: Vec<usize> = (0..drives.len()).filter(|k| ahead.contains(k) && Self::reachable(&successors, *k).contains(&start)).collect();
            reported.extend(cycle.iter().copied());
            // a loop among drives of resolved carriers only settles by relaxation (§8.2.3)
            if !cycle.iter().any(|k| one_driver.contains_key(&drives[*k].1.carrier)) {
                continue;
            }
            let names: Vec<String> = cycle.iter().map(|k| format!("`{}`", self.part_name(&one_driver, drives[*k].1))).collect();
            self.report_loop(drives[start].1, &one_driver, &names, instance);
        }
    }

    /// The carriers of the body of a one-driver nature, each with the declaration it is (frame 0 first).
    fn one_driver_carriers(&self) -> BTreeMap<u32, DeclId> {
        let mut found = BTreeMap::new();
        for frame in &self.state.structure.frames {
            for (decl, carrier) in &frame.carriers {
                if self.state.types.is_one_driver(*decl) {
                    found.entry(*carrier).or_insert(*decl);
                }
            }
        }
        found
    }

    /// The drives reachable from drive `start` along `successors`, `start` itself only through a cycle.
    fn reachable(successors: &[Vec<usize>], start: usize) -> BTreeSet<usize> {
        let mut seen = BTreeSet::new();
        let mut stack: Vec<usize> = successors[start].clone();
        while let Some(next) = stack.pop() {
            if seen.insert(next) {
                stack.extend(successors[next].iter().copied());
            }
        }
        seen
    }

    /// The parts of carriers the lowered expression `value` reads, through every operand and statement.
    fn parts_read(&self, value: LoweredExpr) -> BTreeSet<CarrierPart> {
        let mut parts = BTreeSet::new();
        let mut stack = vec![value];
        while let Some(at) = stack.pop() {
            let Some(kind) = self.state.builder.expr_kind(at) else { continue };
            match kind {
                Expr::Carrier(carrier) | Expr::Previous(carrier) => {
                    parts.insert(CarrierPart { carrier: *carrier, element: None });
                }
                Expr::CarrierElement { carrier, index } | Expr::ElementPrevious { carrier, index } | Expr::ElementChanged { carrier, index } => {
                    let element = match self.state.builder.expr_kind(*index) {
                        Some(Expr::Int(at)) => u64::try_from(*at).ok(),
                        _ => None,
                    };
                    parts.insert(CarrierPart { carrier: *carrier, element });
                }
                _ => {}
            }
            stack.extend(kind.operands());
            for stmt in kind.statements() {
                if let Some(stmt) = self.state.builder.stmt_kind(*stmt) {
                    stack.extend(stmt.operands());
                }
            }
        }
        parts
    }

    /// The name of `part` as the source writes it: `b[1]`, `g[1][0]`, or the carrier's own name.
    fn part_name(&mut self, carriers: &BTreeMap<u32, DeclId>, part: CarrierPart) -> String {
        let fallback = self.state.digital.structure.carriers[part.carrier as usize].name.clone();
        match (carriers.get(&part.carrier), part.element) {
            (Some(decl), Some(at)) => self.shared_element_name(*decl, ElementReach::Run(at, at + 1)),
            (Some(decl), None) => self.state.design.decl_data(*decl).name.as_ref().map_or(fallback, |n| n.text.clone()),
            (None, _) => fallback,
        }
    }

    /// `E0339` at the first drive of a loop that writes `first`, naming every drive of it.
    fn report_loop(&mut self, first: CarrierPart, carriers: &BTreeMap<u32, DeclId>, names: &[String], instance: &str) {
        let reach = ElementReach::of_element(first.element.is_some(), first.element);
        let site = self.state.digital.structure.drive_sites.iter().find(|site| site.carrier == first.carrier && site.reach == reach).map(|site| (site.file, site.span));
        let (file, span) = match (site, carriers.get(&first.carrier)) {
            (Some(site), _) => site,
            (None, Some(decl)) => (self.state.design.decl_data(*decl).file, self.state.design.decl_data(*decl).span),
            (None, None) => return,
        };
        let message = format!("undelayed definitions of {} in `{instance}` depend on each other: a loop through one-driver carriers (§8.2.3)", names.join(", "));
        self.report(file, Diagnostic::new("E0339", message, span, "a loop of definitions"));
    }
}
