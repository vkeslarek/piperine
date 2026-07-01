import os

def fix_tests():
    path = "crates/piperine-lang/src/elab/typecheck.rs"
    with open(path, "r") as f:
        content = f.read()

    bad_logic = """                if let Some(discipline) = design.disciplines.get(d_name) {
                    for item in &discipline.items {
                        if let crate::parse::ast::DisciplineItem::Resolve(_) = item {
                            resolves = true;
                            break;
                        }
                    }
                }"""
    
    good_logic = """                if d_name == "Ground" {
                    resolves = true;
                } else if let Some(discipline) = design.disciplines.get(d_name) {
                    for item in &discipline.items {
                        match item {
                            crate::parse::ast::DisciplineItem::Resolve(_) => resolves = true,
                            crate::parse::ast::DisciplineItem::Nature { kind: crate::parse::ast::NatureKind::Flow, .. } => resolves = true,
                            _ => {}
                        }
                    }
                }"""
    content = content.replace(bad_logic, good_logic)

    with open(path, "w") as f:
        f.write(content)

if __name__ == "__main__":
    fix_tests()
