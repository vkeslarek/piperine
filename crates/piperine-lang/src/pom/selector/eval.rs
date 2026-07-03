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
    ) -> Result<NodeSelection<'a>, crate::pom::error::SelectorError> {
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
    ) -> Result<NodeSelection<'a>, crate::pom::error::SelectorError> {
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
                Predicate::Expr(super::ast::PredExpr::Compare(cmp)) => {
                    next_candidates.retain(|node| {
                        match &cmp.lhs {
                            super::ast::Operand::AttrRef(attr) => {
                                // Split schema.field if present
                                let parts: Vec<&str> = attr.split('.').collect();
                                let (schema, field) = if parts.len() > 1 {
                                    (parts[0], Some(parts[1]))
                                } else {
                                    (parts[0], None)
                                };
                                
                                // Find attribute matching schema
                                let attr_node = match node {
                                    Node::Module(m) => m.attributes().iter().find(|a| a.schema() == schema),
                                    Node::Instance(i) => i.attributes().iter().find(|a| a.schema() == schema),
                                    Node::Port(p) => p.attributes().iter().find(|a| a.schema() == schema),
                                    Node::Param(p) => p.attributes().iter().find(|a| a.schema() == schema),
                                    Node::Wire(w) => w.attributes().iter().find(|a| a.schema() == schema),
                                    Node::Attribute(a) if a.schema() == schema => Some(*a),
                                    _ => None,
                                };
                                
                                if let Some(a) = attr_node {
                                    if let Some(rhs) = &cmp.rhs {
                                        let val = if let Some(f) = field {
                                            a.field(f)
                                        } else {
                                            // If no field specified but we have a rhs, assume we're comparing a default value or it's invalid
                                            // For now, let's just fail the match if there's no field but a rhs is expected
                                            // Unless the attribute itself is a scalar? The SPEC says attributes are scalar properties returning a `Value`.
                                            // But our `Attribute` struct has `data: HashMap<String, Value>`.
                                            // We'll assume if field is none, we check if rhs is matching anything? Let's just return false for now.
                                            None
                                        };
                                        if let Some(val) = val {
                                            match (&rhs.0, val, &rhs.1) {
                                                (super::ast::CmpOp::Eq, crate::pom::Value::Real(r), super::ast::Operand::Literal(super::ast::Literal::Number(n))) => r == n,
                                                (super::ast::CmpOp::Eq, crate::pom::Value::Str(s), super::ast::Operand::Literal(super::ast::Literal::String(n))) => s == n,
                                                (super::ast::CmpOp::Eq, crate::pom::Value::Str(s), super::ast::Operand::Literal(super::ast::Literal::Ident(n))) => s == n,
                                                (super::ast::CmpOp::Gt, crate::pom::Value::Real(r), super::ast::Operand::Literal(super::ast::Literal::Number(n))) => r > n,
                                                (super::ast::CmpOp::Lt, crate::pom::Value::Real(r), super::ast::Operand::Literal(super::ast::Literal::Number(n))) => r < n,
                                                (super::ast::CmpOp::Ge, crate::pom::Value::Real(r), super::ast::Operand::Literal(super::ast::Literal::Number(n))) => r >= n,
                                                (super::ast::CmpOp::Le, crate::pom::Value::Real(r), super::ast::Operand::Literal(super::ast::Literal::Number(n))) => r <= n,
                                                (super::ast::CmpOp::NotEq, crate::pom::Value::Real(r), super::ast::Operand::Literal(super::ast::Literal::Number(n))) => r != n,
                                                (super::ast::CmpOp::NotEq, crate::pom::Value::Str(s), super::ast::Operand::Literal(super::ast::Literal::String(n))) => s != n,
                                                _ => false, // fallback false
                                            }
                                        } else {
                                            false // field not found
                                        }
                                    } else {
                                        true // existence check passed
                                    }
                                } else {
                                    false
                                }
                            }
                            _ => true, // other lhs not supported yet
                        }
                    });
                }
                Predicate::Expr(_) => {
                    // other complex exprs not implemented
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
        } else if let Node::Instance(inst) = node
            && let Some(child_mod) = self.design.module(inst.module_name()) {
                for child_inst in child_mod.instances() {
                    out.push(Node::Instance(child_inst));
                    if let Some(grandchild_mod) = self.design.module(child_inst.module_name()) {
                        self.collect_descendants(Node::Module(grandchild_mod), out);
                    }
                }
            }
    }

    fn walk_axis(&self, node: Node<'a>, axis: &Axis) -> Result<Vec<Node<'a>>, crate::pom::error::SelectorError> {
        let mut res = Vec::new();
        match axis {
            Axis::Inst => {
                if let Node::Module(m) = node {
                    for i in m.instances() {
                        res.push(Node::Instance(i));
                    }
                } else if let Node::Instance(inst) = node
                    && let Some(child_mod) = self.design.module(inst.module_name()) {
                        for i in child_mod.instances() {
                            res.push(Node::Instance(i));
                        }
                    }
            }
            Axis::Net => {
                if let Node::Module(m) = node {
                    for w in m.wires() {
                        res.push(Node::Wire(w));
                    }
                } else if let Node::Instance(inst) = node
                    && let Some(child_mod) = self.design.module(inst.module_name()) {
                        for w in child_mod.wires() {
                            res.push(Node::Wire(w));
                        }
                    }
            }
            Axis::Port => {
                if let Node::Module(m) = node {
                    for p in m.ports() {
                        res.push(Node::Port(p));
                    }
                } else if let Node::Instance(inst) = node
                    && let Some(child_mod) = self.design.module(inst.module_name()) {
                        for p in child_mod.ports() {
                            res.push(Node::Port(p));
                        }
                    }
            }
            Axis::Param => {
                if let Node::Module(m) = node {
                    for p in m.params() {
                        res.push(Node::Param(p));
                    }
                } else if let Node::Instance(inst) = node
                    && let Some(child_mod) = self.design.module(inst.module_name()) {
                        for p in child_mod.params() {
                            res.push(Node::Param(p));
                        }
                    }
            }
            Axis::Behavior => {
                if let Node::Module(m) = node {
                    for b in m.behaviors() {
                        res.push(Node::Behavior(b));
                    }
                } else if let Node::Instance(inst) = node
                    && let Some(child_mod) = self.design.module(inst.module_name()) {
                        for b in child_mod.behaviors() {
                            res.push(Node::Behavior(b));
                        }
                    }
            }

            Axis::Attr => {
                if let Node::Module(m) = node {
                    for a in m.attributes() { res.push(Node::Attribute(a)); }
                } else if let Node::Instance(i) = node {
                    for a in i.attributes() { res.push(Node::Attribute(a)); }
                } else if let Node::Port(p) = node {
                    for a in p.attributes() { res.push(Node::Attribute(a)); }
                } else if let Node::Param(p) = node {
                    for a in p.attributes() { res.push(Node::Attribute(a)); }
                } else if let Node::Wire(w) = node {
                    for a in w.attributes() { res.push(Node::Attribute(a)); }
                }
            }
            Axis::Driver | Axis::Load | Axis::Parent | Axis::Ancestor => {
                // To be implemented: structural connectivity and parent axes
                return Err(crate::pom::error::SelectorError::AxisNotImplemented(axis.clone()));
            }

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
        let inst1 = crate::pom::Instance { attributes: vec![], label: Some("c1".into()),
            module: "Child".into(),
            ports: vec![],
            params: vec![],
        };
        let inst2 = crate::pom::Instance { attributes: vec![], label: Some("c2".into()),
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

    #[test]
    fn test_eval_attr_predicates() {
        let mut design = Design::new();
        let mut top_mod = Module::new(
            "Top".into(),
            vec![], vec![], vec![], vec![], vec![], vec![]
        );

        let attr1 = crate::pom::module::Attribute {
            schema: "layout".into(),
            data: {
                let mut map = std::collections::HashMap::new();
                map.insert("min_width".into(), crate::pom::Value::Real(2.0));
                map
            },
        };

        let attr2 = crate::pom::module::Attribute {
            schema: "layout".into(),
            data: {
                let mut map = std::collections::HashMap::new();
                map.insert("min_width".into(), crate::pom::Value::Real(0.5));
                map
            },
        };

        let w1 = crate::pom::module::Wire { name: "clk".into(), attributes: vec![attr1],
            ty: crate::pom::net_type::NetType::Discipline("Electrical".into()),
        };

        let w2 = crate::pom::module::Wire { name: "rst".into(), attributes: vec![attr2],
            ty: crate::pom::net_type::NetType::Discipline("Electrical".into()),
        };

        let w3 = crate::pom::module::Wire { name: "data".into(), attributes: vec![],
            ty: crate::pom::net_type::NetType::Discipline("Electrical".into()),
        };

        top_mod.wires.push(w1);
        top_mod.wires.push(w2);
        top_mod.wires.push(w3);
        design.insert_module("Top".into(), top_mod);
        design.set_top("Top");

        let evaluator = Evaluator::new(&design);

        let sel_all = "//net::*".parse::<Selector>().unwrap();
        let res_all = evaluator.evaluate(&sel_all, NodeSelection::new()).unwrap();
        assert_eq!(res_all.len(), 3);

        let sel_has_layout = "//net::*[@layout]".parse::<Selector>().unwrap();
        let res_has_layout = evaluator.evaluate(&sel_has_layout, NodeSelection::new()).unwrap();
        assert_eq!(res_has_layout.len(), 2);

        let sel_gt_1 = "//net::*[@layout.min_width > 1.0]".parse::<Selector>().unwrap();
        let res_gt_1 = evaluator.evaluate(&sel_gt_1, NodeSelection::new()).unwrap();
        assert_eq!(res_gt_1.len(), 1);
        assert_eq!(res_gt_1.get(0).unwrap().name(), "clk");
    }
}
