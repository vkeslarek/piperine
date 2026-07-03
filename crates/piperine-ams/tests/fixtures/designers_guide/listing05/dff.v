// D Flip Flop
//
// Version 1a, 1 June 04
//
// Olaf Zinke
//
// Downloaded from The Designer's Guide Community (www.designers-guide.org).
// Post any questions on www.designers-guide.org/Forum.
// Taken from "The Designer's Guide to Verilog-AMS" by Kundert & Zinke.
// Chapter 4, Listing 5.

module dff (q, d, clk);
    output q;
    input d, clk;
    reg q;

    always @(clk)
	if (clk)
	    q = d;
endmodule
