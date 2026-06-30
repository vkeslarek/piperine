// Counter
//
// Version 1a, 1 June 04
//
// Olaf Zinke
//
// Downloaded from The Designer's Guide Community (www.designers-guide.org).
// Post any questions on www.designers-guide.org/Forum.
// Taken from "The Designer's Guide to Verilog-AMS" by Kundert & Zinke.
// Chapter 4, Listing 7.

module counter (out, clk);
    parameter maxcount = 9;     // the number of input pulses per output pulse
    input clk;
    output out;
    reg out;
    integer count;

    initial begin
	out = 0;
	count = 0;
    end

    always @(posedge clk) begin
	count = count + 1;
	if (count == maxcount) begin
	    out = 1;
	    count = 0;
	end else if (count == 1)
	    out = 0;
    end
endmodule
