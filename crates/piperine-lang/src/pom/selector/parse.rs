use super::ast::*;

use std::str::FromStr;

impl FromStr for Selector {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut chars = input.trim().chars().peekable();
        if chars.peek().is_none() {
            return Err("Empty selector".into());
        }

        let mut absolute = false;
        let mut is_descendant = false;

        if chars.peek() == Some(&'/') {
            absolute = true;
            chars.next();
            if chars.peek() == Some(&'/') {
                is_descendant = true;
                chars.next();
            }
        }

        let mut steps = Vec::new();

        loop {
            let mut step = Step {
                is_descendant,
                axis: Axis::Inst,
                test: NodeTest::Any,
                predicates: Vec::new(),
            };

            let mut word = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' || c == '*' {
                    word.push(c);
                    chars.next();
                } else {
                    break;
                }
            }

            if chars.peek() == Some(&':') {
                chars.next();
                if chars.peek() == Some(&':') {
                    chars.next();
                    step.axis = match word.as_str() {
                        "inst" => Axis::Inst,
                        "net" => Axis::Net,
                        "port" => Axis::Port,
                        "param" => Axis::Param,
                        "attr" => Axis::Attr,
                        "behavior" => Axis::Behavior,
                        "driver" => Axis::Driver,
                        "load" => Axis::Load,
                        "parent" => Axis::Parent,
                        "ancestor" => Axis::Ancestor,
                        _ => return Err(format!("Unknown axis: {}", word)),
                    };
                    word.clear();
                    while let Some(&c) = chars.peek() {
                        if c.is_alphanumeric() || c == '_' || c == '*' {
                            word.push(c);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                } else {
                    return Err("Expected :: after axis".into());
                }
            }

            if word == "*" {
                step.test = NodeTest::Any;
            } else if !word.is_empty() {
                step.test = NodeTest::Name(word);
            } else {
                return Err("Expected NodeTest".into());
            }

            while let Some(&c) = chars.peek() {
                if c == '[' {
                    chars.next();
                    let mut pred_str = String::new();
                    while let Some(&p) = chars.peek() {
                        if p == ']' {
                            chars.next();
                            break;
                        }
                        pred_str.push(p);
                        chars.next();
                    }
                    if let Ok(i) = pred_str.parse::<usize>() {
                        step.predicates.push(Predicate::Index(i));
                    } else if pred_str == "last()" {
                        step.predicates.push(Predicate::Last);
                    } else if pred_str.starts_with('@') {
                        // Simple parser for [@attr_name == "value"]
                        let mut tokens = pred_str.split_whitespace().collect::<Vec<_>>();
                        if tokens.len() == 1 {
                            // Existence check: [@layout]
                            let attr = tokens[0].trim_start_matches('@').to_string();
                            step.predicates.push(Predicate::Expr(PredExpr::Compare(Compare {
                                lhs: Operand::AttrRef(attr),
                                rhs: None,
                            })));
                        } else if tokens.len() >= 3 {
                            let attr = tokens[0].trim_start_matches('@').to_string();
                            let op = match tokens[1] {
                                "==" => CmpOp::Eq,
                                "!=" => CmpOp::NotEq,
                                "<" => CmpOp::Lt,
                                "<=" => CmpOp::Le,
                                ">" => CmpOp::Gt,
                                ">=" => CmpOp::Ge,
                                "~" => CmpOp::Glob,
                                _ => CmpOp::Eq, // fallback
                            };
                            let rhs_str = tokens[2..].join(" ");
                            let rhs = if rhs_str.starts_with('"') && rhs_str.ends_with('"') {
                                Operand::Literal(Literal::String(rhs_str[1..rhs_str.len()-1].to_string()))
                            } else if let Ok(n) = rhs_str.parse::<f64>() {
                                Operand::Literal(Literal::Number(n))
                            } else if rhs_str == "true" {
                                Operand::Literal(Literal::Bool(true))
                            } else if rhs_str == "false" {
                                Operand::Literal(Literal::Bool(false))
                            } else {
                                Operand::Literal(Literal::Ident(rhs_str.clone()))
                            };
                            step.predicates.push(Predicate::Expr(PredExpr::Compare(Compare {
                                lhs: Operand::AttrRef(attr),
                                rhs: Some((op, rhs)),
                            })));
                        }
                    } else {
                        // skip complex exprs for now
                    }
                } else {
                    break;
                }
            }

            steps.push(step);

            if chars.peek() == Some(&'/') {
                chars.next();
                if chars.peek() == Some(&'/') {
                    is_descendant = true;
                    chars.next();
                } else {
                    is_descendant = false;
                }
            } else {
                break;
            }
        }

        Ok(Selector { absolute, steps })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_absolute() {
        let sel = "/dac".parse::<Selector>().unwrap();
        assert_eq!(sel.absolute, true);
        assert_eq!(sel.steps.len(), 1);
        assert_eq!(sel.steps[0].is_descendant, false);
        assert_eq!(sel.steps[0].axis, Axis::Inst);
        assert_eq!(sel.steps[0].test, NodeTest::Name("dac".to_string()));
    }

    #[test]
    fn test_parse_attr_predicate() {
        let sel = "/dac[@layout.min_width > 1.0]".parse::<Selector>().unwrap();
        assert_eq!(sel.steps[0].predicates.len(), 1);
        if let Predicate::Expr(PredExpr::Compare(cmp)) = &sel.steps[0].predicates[0] {
            assert_eq!(cmp.lhs, Operand::AttrRef("layout.min_width".to_string()));
            let (op, rhs) = cmp.rhs.as_ref().unwrap();
            assert_eq!(*op, CmpOp::Gt);
            assert_eq!(*rhs, Operand::Literal(Literal::Number(1.0)));
        } else {
            panic!("Expected Compare predicate");
        }
    }
    
    #[test]
    fn test_parse_attr_existence() {
        let sel = "//net::clk[@route]".parse::<Selector>().unwrap();
        assert_eq!(sel.steps[0].predicates.len(), 1);
        if let Predicate::Expr(PredExpr::Compare(cmp)) = &sel.steps[0].predicates[0] {
            assert_eq!(cmp.lhs, Operand::AttrRef("route".to_string()));
            assert!(cmp.rhs.is_none());
        } else {
            panic!("Expected Compare predicate");
        }
    }

    #[test]
    fn test_parse_descendant() {
        let sel = "//Resistor".parse::<Selector>().unwrap();
        assert_eq!(sel.absolute, true);
        assert_eq!(sel.steps.len(), 1);
        assert_eq!(sel.steps[0].is_descendant, true);
        assert_eq!(sel.steps[0].axis, Axis::Inst);
        assert_eq!(sel.steps[0].test, NodeTest::Name("Resistor".to_string()));
    }

    #[test]
    fn test_parse_axis_explicit() {
        let sel = "net::*".parse::<Selector>().unwrap();
        assert_eq!(sel.absolute, false);
        assert_eq!(sel.steps.len(), 1);
        assert_eq!(sel.steps[0].is_descendant, false);
        assert_eq!(sel.steps[0].axis, Axis::Net);
        assert_eq!(sel.steps[0].test, NodeTest::Any);
    }
}
