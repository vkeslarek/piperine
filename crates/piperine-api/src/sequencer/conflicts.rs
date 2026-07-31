//! Ordering and conflicts among several host devices at one instant (T26,
//! DVT-28/44): the authored POM instantiation order already governs
//! independent actions — it is a property of `Vec<Box<dyn Element>>`
//! iteration order, which the circuit builder fills by walking the POM in
//! source order, so nothing new is needed there (`ideal.md` §12.12). What
//! *is* new here is the other half of that decision: two devices **driving
//! the same net** is a loud conflict regardless of order, never
//! last-writer-wins. Declaring a drive is the only way to detect this —
//! raw MNA stamps never distinguish "this device is intentionally forcing
//! this net" from an ordinary resistive contribution, so [`DriveRegistry`]
//! is a host-side pre-flight the drive helpers ([`super::drive`],
//! [`super::install::InstallSite`]) are meant to be paired with.

use std::collections::HashMap;

/// One instance's claim to be driving `net_label`.
#[derive(Debug, Clone)]
struct Claim {
    instance: String,
    net_label: String,
}

/// Collects drive claims from however many sequencers/devices a host
/// builds, then checks them for conflicts — order-independent by
/// construction (claims are grouped by net, not compared pairwise in
/// registration order).
#[derive(Default)]
pub struct DriveRegistry {
    claims: Vec<Claim>,
}

/// Two (or more) instances declared they drive the same net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveConflict {
    pub net_label: String,
    /// The conflicting instances, in the order their claims group in
    /// (sorted, so registration order never changes the reported set).
    pub instances: Vec<String>,
}

impl std::fmt::Display for DriveConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "net '{}' is driven by more than one device: {} — a loud conflict, never last-writer-wins",
            self.net_label,
            self.instances.join(", ")
        )
    }
}

impl DriveRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare that `instance` drives `net_label`.
    pub fn claim(&mut self, instance: impl Into<String>, net_label: impl Into<String>) {
        self.claims.push(Claim { instance: instance.into(), net_label: net_label.into() });
    }

    /// Every net claimed by more than one distinct instance, as a
    /// [`DriveConflict`] each — sorted by net label, and each conflict's
    /// instance list sorted, so the result is identical regardless of the
    /// order `claim` was called in.
    pub fn conflicts(&self) -> Vec<DriveConflict> {
        let mut by_net: HashMap<&str, Vec<&str>> = HashMap::new();
        for c in &self.claims {
            let set = by_net.entry(c.net_label.as_str()).or_default();
            if !set.contains(&c.instance.as_str()) {
                set.push(&c.instance);
            }
        }
        let mut out: Vec<DriveConflict> = by_net
            .into_iter()
            .filter(|(_, instances)| instances.len() > 1)
            .map(|(net, mut instances)| {
                instances.sort();
                DriveConflict { net_label: net.to_string(), instances: instances.into_iter().map(String::from).collect() }
            })
            .collect();
        out.sort_by(|a, b| a.net_label.cmp(&b.net_label));
        out
    }

    /// `Ok(())` when no net has more than one claimant; the first conflict
    /// (by net label) as a loud `Err` otherwise.
    pub fn check(&self) -> Result<(), DriveConflict> {
        match self.conflicts().into_iter().next() {
            Some(c) => Err(c),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_claims_or_disjoint_claims_have_no_conflict() {
        let mut r = DriveRegistry::new();
        r.claim("seq_a", "vout");
        r.claim("seq_b", "vin");
        assert!(r.check().is_ok());
    }

    #[test]
    fn two_instances_claiming_the_same_net_conflict_regardless_of_registration_order() {
        let mut forward = DriveRegistry::new();
        forward.claim("seq_a", "vout");
        forward.claim("seq_b", "vout");

        let mut reverse = DriveRegistry::new();
        reverse.claim("seq_b", "vout");
        reverse.claim("seq_a", "vout");

        let e1 = forward.check().expect_err("two claimants on vout must conflict");
        let e2 = reverse.check().expect_err("the reverse registration order must conflict identically");
        assert_eq!(e1, e2, "the reported conflict must not depend on registration order");
        assert_eq!(e1.net_label, "vout");
        assert_eq!(e1.instances, vec!["seq_a".to_string(), "seq_b".to_string()]);
    }

    #[test]
    fn the_same_instance_claiming_a_net_twice_is_not_a_conflict() {
        let mut r = DriveRegistry::new();
        r.claim("seq_a", "vout");
        r.claim("seq_a", "vout");
        assert!(r.check().is_ok(), "one instance re-declaring its own claim is not a conflict with itself");
    }
}
