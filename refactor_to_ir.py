import re

with open("crates/piperine-ams/src/to_ir.rs", "r") as f:
    code = f.read()

# I will write the python script incrementally or just use `sed`/regex in python.
# Actually, the file has 1323 lines. There are about 50 functions. 
# Many use `lower_expr(..., ctx)`. They need to become `lower_expr(..., ctx)?`.
