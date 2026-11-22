#!/bin/sh
# fold.sh <raw>: one line per sample (the stack of the thread running the test), innermost first
awk '
function flush() { if (best != "") print best; best = ""; cur = "" }
function endthread() { if (cur ~ /digital_throughput/ && best == "") best = cur; cur = "" }
/^=== sample/ { endthread(); flush(); next }
/^Thread / { endthread(); next }
/^#[0-9]+ / { l = $0; sub(/^#[0-9]+ +(0x[0-9a-f]+ in )?/, "", l); sub(/ \(.*$/, "", l); while (l ~ /<[^<>]*>/) gsub(/<[^<>]*>/, "", l); cur = cur (cur == "" ? "" : ";") l }
END { endthread(); flush() }' "$1"
