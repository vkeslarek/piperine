// Two input latch
//
// Version 1a, 1 June 04
//
// Olaf Zinke
//
// Downloaded from The Designer's Guide Community (www.designers-guide.org).
// Post any questions on www.designers-guide.org/Forum.
// Taken from "The Designer's Guide to Verilog-AMS" by Kundert & Zinke.
// Chapter 4, Listing 6.

module latch2(out, d1, d2, en);
    output [ 1 : 0 ] out;
    input d1, d2, en;
    reg [ 1 : 0 ] out;

    always @(d1 or d2) wait(!en) begin
        out [ 0 ] = d1;
        out [ 1 ] = d2;
    end
endmodule

