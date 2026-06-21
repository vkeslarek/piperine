#!/bin/bash
# ngspice manuals distribution build script for Linux, release or debug version, 64 bit
# compile_linux.sh <d>

SECONDS=0

autoreconf
./configure
make dist
if [ $exitcode -ne 0 ]; then  echo "make dist failed"; exit 1 ; fi

ELAPSED="Elapsed compile time: $(($SECONDS / 3600))hrs $((($SECONDS / 60) % 60))min $(($SECONDS % 60))sec"
echo
echo $ELAPSED
echo "success"
exit 0
