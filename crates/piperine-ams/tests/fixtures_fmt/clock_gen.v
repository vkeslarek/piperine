// Simple clock generator
//
// Version 1a, 1 June 04
//
// Olaf Zinke
//
// Downloaded from The Designer's Guide Community (www.designers-guide.org).
// Post any questions on www.designers-guide.org/Forum.
// Taken from "The Designer's Guide to Verilog-AMS" by Kundert & Zinke.
// Chapter 4, Listing 4.

`timescale 1ns / 1ps

module clock_gen(clk);
    parameter cycle = 20;
    // clock period (ns)
    output clk;
    reg clk;

    initial clk = 0;

    always # (cycle / 2) clk = ~clk;
endmodule

