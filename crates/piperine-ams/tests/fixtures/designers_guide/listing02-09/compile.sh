# cp.vams    compile script
#
# Version 1a, 1 June 04
#
# Olaf Zinke
#
# Downloaded from The Designer's Guide (www.designers-guide.org).
# Post any questions on www.designers-guide.org/Forum.
# Taken from "The Designer's Guide to Verilog-AMS" by Kundert & Zinke.
# Appendix A, Listing 9.


ncvlog -ams cp.vams
ncvlog -ams vco.vams
ncvlog pfd.v
ncvlog fd.v 
ncvlog -ams plltop.vams
