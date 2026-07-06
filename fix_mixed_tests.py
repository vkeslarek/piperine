import re

path = "crates/piperine-solver/tests/mixed_signal.rs"
with open(path, "r") as f:
    text = f.read()

# Replace `&[...], &mut` with `ndarray::ArrayView1::from(&[...]), &mut`
text = re.sub(r'(&\[[^\]]*\]),\s*&mut', r'ndarray::ArrayView1::from(\1), &mut', text)

with open(path, "w") as f:
    f.write(text)

