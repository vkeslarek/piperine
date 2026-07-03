database -open waves -into waves.shm -default
probe -create testbench -shm -waveform
run 1us
exit
