use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::Meter;
use std::sync::Arc;

/// Coupled multiline (`P`) using a CPL model.
///
/// `PXXXX n1+ ... nN+ g1 n1- ... nN- g2 MNAME <LENGTH=val>`
#[derive(Debug)]
pub struct CoupledMultiline {
    name: String,
    pos_nodes: Vec<Node>,
    pos_ref: Node,
    neg_nodes: Vec<Node>,
    neg_ref: Node,
    model: Arc<dyn crate::models::cpl::CplModel + Send + Sync>,
    length: Option<Dynamic<Meter>>,
}

impl Clone for CoupledMultiline {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            pos_nodes: self.pos_nodes.clone(),
            pos_ref: self.pos_ref,
            neg_nodes: self.neg_nodes.clone(),
            neg_ref: self.neg_ref,
            model: Arc::clone(&self.model),
            length: self.length.clone(),
        }
    }
}

impl CoupledMultiline {
    pub const SYMBOL: &str = "P";

    pub fn new(
        name: impl Into<String>,
        pos_nodes: Vec<Node>,
        pos_ref: impl Into<Node>,
        neg_nodes: Vec<Node>,
        neg_ref: impl Into<Node>,
        model: Arc<dyn crate::models::cpl::CplModel + Send + Sync>,
    ) -> Self {
        assert!(
            !pos_nodes.is_empty(),
            "CPL requires at least one coupled line"
        );
        assert_eq!(
            pos_nodes.len(),
            neg_nodes.len(),
            "CPL requires same number of positive and negative conductor nodes"
        );

        Self {
            name: name.into(),
            pos_nodes,
            pos_ref: pos_ref.into(),
            neg_nodes,
            neg_ref: neg_ref.into(),
            model,
            length: None,
        }
    }

    pub fn with_length(&mut self, v: impl Into<Dynamic<Meter>>) -> &mut Self {
        self.length = Some(v.into());
        self
    }
}

impl Component for CoupledMultiline {}

impl SpiceElement for CoupledMultiline {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }

    fn spice_model(&self) -> Option<Arc<dyn crate::spice::SpiceModel>> {
        Some(Arc::clone(&self.model) as Arc<dyn crate::spice::SpiceModel>)
    }
}

impl SpiceComponent for CoupledMultiline {
    fn into_spice(&self) -> String {
        let mut tokens = Vec::new();
        tokens.push(format!("{}{}", Self::SYMBOL, self.name));
        tokens.extend(self.pos_nodes.iter().map(ToString::to_string));
        tokens.push(self.pos_ref.to_string());
        tokens.extend(self.neg_nodes.iter().map(ToString::to_string));
        tokens.push(self.neg_ref.to_string());
        tokens.push(self.model.model_name().to_string());
        if let Some(v) = &self.length {
            tokens.push(format!("LENGTH={v}"));
        }
        tokens.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::cpl::DefaultModel;

    #[test]
    fn serializes_cpl_instance_with_length() {
        let model = Arc::new(DefaultModel::new("PLINE"));
        let mut p = CoupledMultiline::new(
            "1",
            vec![Node::from("5"), Node::from("6")],
            "0",
            vec![Node::from("9"), Node::from("10")],
            "0",
            model,
        );
        p.with_length(24.0);

        assert_eq!(p.into_spice(), "P1 5 6 0 9 10 0 PLINE LENGTH=24");
    }

    #[test]
    #[should_panic(expected = "CPL requires same number of positive and negative conductor nodes")]
    fn cpl_panics_on_mismatched_dimensions() {
        let model = Arc::new(DefaultModel::new("PLINE"));
        let _ = CoupledMultiline::new(
            "bad",
            vec![Node::from("1"), Node::from("2")],
            "0",
            vec![Node::from("3")],
            "0",
            model,
        );
    }
}
