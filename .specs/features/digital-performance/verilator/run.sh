#!/bin/sh
# Builds and times the Verilator twins of the digital benchmark (diagnosis.md §4): the best of three
# runs per case. Usage: run.sh <work dir>. Needs `verilator` and a C++ compiler on PATH; installs nothing.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
work=${1:?a work directory}
python3 "$here/gen.py" "$work"
printf '%-13s %10s %9s %12s %10s\n' case cycles "run s" cycles/s ns/cycle
for d in "$work"/*/; do
  name=$(basename "$d")
  if [ ! -x "$d/obj_dir/sim" ] || [ "$d/top.v" -nt "$d/obj_dir/sim" ] || [ "$d/tb.cpp" -nt "$d/obj_dir/sim" ]; then
    (cd "$d" && verilator --cc --exe --build -O3 --x-assign fast --x-initial fast --no-assert -Wno-fatal \
        -CFLAGS -O2 --top-module top top.v tb.cpp -o sim >build.log 2>&1)
  fi
  cycles=$(cat "$d/cycles")
  best=""
  for _ in 1 2 3; do
    set -- $("$d/obj_dir/sim" "$cycles")
    best=$(awk -v a="$2" -v b="${best:-1e30}" 'BEGIN { print (a < b) ? a : b }')
  done
  awk -v n="$name" -v c="$cycles" -v s="$best" 'BEGIN { printf "%-13s %10d %9.3f %12.0f %10.1f\n", n, c, s, c / s, s * 1e9 / c }'
done
