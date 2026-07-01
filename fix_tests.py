import os

def fix_tests():
    path = "crates/piperine-lang/src/elab/typecheck.rs"
    with open(path, "r") as f:
        content = f.read()

    bad_init = """        let mut prog = crate::pom::Design {
            modules: HashMap::new(),
            disciplines: HashMap::new(),
            bundles: HashMap::new(),
            capabilities: HashMap::new(),
            impls: vec![],
            consts: HashMap::new(),
        };"""
    good_init = "        let mut prog = crate::pom::Design::new();"
    content = content.replace(bad_init, good_init)

    # Fix HashMap missing types issue
    content = content.replace("let mut parent = HashMap::new();", "let mut parent: HashMap<String, String> = HashMap::new();")

    with open(path, "w") as f:
        f.write(content)

if __name__ == "__main__":
    fix_tests()
