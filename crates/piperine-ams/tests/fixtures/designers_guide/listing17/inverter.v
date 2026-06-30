// Simple inverter
//
// Version 1a, 1 June 04
//
// Olaf Zinke
//
// Downloaded from The Designer's Guide Community (www.designers-guide.org).
// Post any questions on www.designers-guide.org/Forum.
// Taken from "The Designer's Guide to Verilog-AMS" by Kundert & Zinke.
// Chapter 4, Listing 1.

module inverter (q, a);
    output q;
    input a;
    wire a, q;	// digital net type (declaration optional)

    assign q=~a;
endmodule

