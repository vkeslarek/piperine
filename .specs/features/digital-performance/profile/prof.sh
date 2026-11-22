#!/bin/sh
# usage: prof.sh <n> <raw out> <program> [args]: runs the program under gdb (symbols load first);
# a loop sends SIGUSR1 to it every 0.1 s, gdb stops on it (not passed on), prints every stack, continues.
n=$1; out=$2; shift 2
cmds=$(mktemp)
cat > "$cmds" <<G
set pagination off
set confirm off
set startup-with-shell off
handle SIGUSR1 stop print nopass
run
set \$i = 0
while \$i < $n
  echo === sample\n
  thread apply all bt 60
  set \$i = \$i + 1
  continue
end
kill
quit
G
gdb -batch -nx -x "$cmds" --args "$@" > "$out" 2>&1 &
g=$!
pid=""
while [ -z "$pid" ] && kill -0 $g 2>/dev/null; do sleep 0.5; pid=$(pgrep -P $g | head -1); done
sleep 1
while kill -0 $g 2>/dev/null; do kill -USR1 "$pid" 2>/dev/null || break; sleep 0.1; done
rm -f "$cmds"
