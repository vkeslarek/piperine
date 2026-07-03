// Frequency Divider
//
// Version 1a, 1 June 04
//
// Olaf Zinke
//
// Downloaded from The Designer's Guide Community (www.designers-guide.org).
// Post any questions on www.designers-guide.org/Forum.
// Taken from "The Designer's Guide to Verilog-AMS" by Kundert & Zinke.
// Appendix A, Listing 7.

`timescale 10ps / 1ps

module fd (out, clk, reset);
    input clk, reset;
    output out;
    wire out;
    reg q;
    integer i;

    always @(negedge reset) begin
	i = 0;
	q = 0;
    end

    always @(posedge clk) begin
	if (~reset) begin
	    i = i + 1;
	    if (i == 63) begin
		q = ~q;
		i = 0; 
	    end
	end
    end

    assign out = q & ~reset;
endmodule
