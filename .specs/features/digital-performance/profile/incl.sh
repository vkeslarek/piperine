#!/bin/sh
# incl.sh <fold> [filter]: inclusive count per function (once per sample), of samples matching filter
f=$1; pat=${2:-transient_loop}
grep "$pat" "$f" | awk -F';' '{delete seen; for(i=1;i<=NF;i++){ if(!($i in seen)){seen[$i]=1; c[$i]++}} n++} END{for(k in c) printf "%5.1f%% %s\n", 100*c[k]/n, k; print n " samples" > "/dev/stderr"}' | sort -rn
