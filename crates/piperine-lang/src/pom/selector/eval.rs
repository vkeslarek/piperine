use super::ast::{Axis, NodeTest, Predicate, Selector, Step};
use crate::pom::design::Design;
use crate::pom::node::Node;
use crate::pom::selection::NodeSelection;
use crate::pom::traits::Named;

pub struct Evaluator<'a> {
    design: &'a Design,
}

impl<'a> Evaluator<'a> {
    pub fn new(design: &'a Design) -> Self {
        Self { design }
    }

    pub fn evaluate(
        &self,
        selector: &Selector,
        mut current: NodeSelection<'a>,
    ) -> Result<NodeSelection<'a>, String> {
        if selector.absolute {
            if let Some(top) = self.design.top() {
                current = NodeSelection::from_vec(vec![Node::Module(top)]);
            } else {
                return Ok(NodeSelection::new());
            }
        }

        for step in &selector.steps {
            current = self.eval_step(step, current)?;
        }
        Ok(current)
    }

    fn eval_step(
        &self,
        step: &Step,
        current: NodeSelection<'a>,
    ) -> Result<NodeSelection<'a>, String> {
        let mut next_candidates = Vec::new();

        for node in current.iter() {
            let mut base_nodes = vec![*node];
            
            if step.is_descendant {
                let mut descendants = Vec::new();
                self.collect_descendants(*node, &mut descendants);
                base_nodes.append(&mut descendants);
            }

            for base_node in base_nodes {
                let mut children = self.walk_axis(base_node, &step.axis)?;
                next_candidates.append(&mut children);
            }
        }

        next_candidates.retain(|node| match &step.test {
            NodeTest::Any => true,
            NodeTest::Name(n) => {
                if step.axis == Axis::Inst {
                    if n.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        if let Node::Instance(inst) = node {
                            inst.module_name() == n
                        } else {
                            false
                        }
                    } else {
                        node.name() == n
                    }
                } else {
                    node.name() == n
                }
            }
        });

        for pred in &step.predicates {
            match pred {
                Predicate::Index(i) => {
                    if *i < next_candidates.len() {
                        let kept = next_candidates[*i];
                        next_candidates.clear();
                        next_candidates.push(kept);
                    } else {
                        next_candidates.clear();
                    }
                }
                Predicate::Last => {
                    if let Some(last) = next_candidates.pop() {
                        next_candidates.clear();
                        next_candidates.push(last);
                    }
                }
                Predicate::Expr(_) => {
                    return Err("Expression predicates not yet implemented".into());
                }
            }
        }

        let mut deduped = Vec::new();
        for c in next_candidates {
            if !deduped.contains(&c) {
                deduped.push(c);
            }
        }

        Ok(NodeSelection::from_vec(deduped))
    }

    fn collect_descendants(&self, node: Node<'a>, out: &mut Vec<Node<'a>>) {
        if let Node::Module(m) = node {
            for inst in m.instances() {
                out.push(Node::Instance(inst));
                if let Some(child_mod) = self.design.module(inst.module_name()) {
                    self.collect_descendants(Node::Module(child_mod), out);
                }
            }
        } else if let Node::Instance(inst) = node {
            if let Some(child_mod) = self.design.module(inst.module_name()) {
                for child_inst in child_mod.instances() {
                    out.push(Node::Instance(child_inst));
                    if let Some(grandchild_mod) = self.design.module(child_inst.module_name()) {
                        self.collect_descendants(Node::Module(grandchild_mod), out);
                    }
                }
            }
        }
    }

    fn walk_axis(&self, node: Node<'a>, axis: &Axis) -> Result<Vec<Node<'a>>, String> {
        let mut res = Vec::new();
        match axis {
            Axis::Inst => {
                if let Node::Module(m) = node {
                    for i in m.instances() {
                        res.push(Node::Instance(i));
                    }
                } else if let Node::Instance(inst) = node {
                    if let Some(child_mod) = self.design.module(inst.module_name()) {
                        for i in child_mod.instances() {
                            res.push(Node::Instance(i));
                        }
                    }
                }
            }
            Axis::Net => {
                if let Node::Module(m) = node {
                    for w in m.wires() {
                        res.push(Node::Wire(w));
                    }
                } else if let Node::Instance(inst) = node {
                    if let Some(child_mod) = self.design.module(inst.module_name()) {
                        for w in child_mod.wires() {
                            res.push(Node::Wire(w));
                        }
                    }
                }
            }
            Axis::Port => {
                if let Node::Module(m) = node {
                    for p in m.ports() {
                        res.push(Node::Port(p));
                    }
                } else if let Node::Instance(inst) = node {
                    if let Some(child_mod) = self.design.module(inst.module_name()) {
                        for p in child_mod.ports() {
                            res.push(Node::Port(p));
                        }
                    }
                }
            }
            Axis::Param => {
                if let Node::Module(m) = node {
                    for p in m.params() {
                        res.push(Node::Param(p));
                    }
                } else if let Node::Instance(inst) = node {
                    if let Some(child_mod) = self.design.module(inst.module_name()) {
                        for p in child_mod.params() {
                            res.push(Node::Param(p));
                        }
                    }
                }
            }
            Axis::Behavior => {
                if let Node::Module(m) = node {
                    for b in m.behaviors() {
                        res.push(Node::Behavior(b));
                    }
                } else if let Node::Instance(inst) = node {
                    if let Some(child_mod) = self.design.module(inst.module_name()) {
                        for b in child_mod.behaviors() {
                            res.push(Node::Behavior(b));
                        }
                    }
                }
            }
            _ => return Err(format!("Axis {:?} not yet implemented", axis)),
        }
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pom::module::{Instance, Module};

    #[test]
    fn test_eval_simple() {
        let mut design = Design::new();
        
        let child_mod = Module::new(
            "Child".into(),
            vec![], vec![], vec![], vec![], vec![], vec![]
        );
        let inst1 = Instance {
            label: Some("c1".into()),
            module: "Child".into(),
            ports: vec![],
            params: vec![],
        };
        let inst2 = Instance {
            label: Some("c2".into()),
            module: "Child".into(),
            ports: vec![],
            params: vec![],
        };
        let top_mod = Module::new(
            "Top".into(),
            vec![], vec![], vec![], vec![inst1, inst2], vec![], vec![]
        );

        design.insert_module("Child".into(), child_mod);
        design.insert_module("Top".into(), top_mod);
        design.set_top("Top");

        let sel = "/Child".parse::<Selector>().unwrap();
        let current = NodeSelection::new();
        let evaluator = Evaluator::new(&design);
        let res = evaluator.evaluate(&sel, current).unwrap();

        assert_eq!(res.len(), 2);
        assert_eq!(res.get(0).unwrap().name(), "c1");
        assert_eq!(res.get(1).unwrap().name(), "c2");

        let sel2 = "/c2".parse::<Selector>().unwrap();
        let res2 = evaluator.evaluate(&sel2, NodeSelection::new()).unwrap();
        assert_eq!(res2.len(), 1);
        assert_eq!(res2.get(0).unwrap().name(), "c2");
    }
}
