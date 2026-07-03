// Phase Frequency Detector
//
// Version 1a, 1 June 04
//
// Olaf Zinke
//
// Downloaded from The Designer's Guide Community (www.designers-guide.org).
// Post any questions on www.designers-guide.org/Forum.
// Taken from "The Designer's Guide to Verilog-AMS" by Kundert & Zinke.
// Appendix A, Listing 5.

`timescale 10ps / 1ps

module pfd (qinc, qdec, active, ref, reset);
    output qinc, qdec;
    input reset, active, ref;
    wire fv_rst, fr_rst;
    reg q0, q1;

    assign fr_rst = reset | (q0 & q1);
    assign fv_rst = reset | (q0 & q1);

    always @(posedge active or posedge fv_rst) begin
	    if (fv_rst) q0 <= 0; else q0 <= 1;
    end

    always @(posedge ref or posedge fr_rst) begin
	    if (fr_rst) q1 <= 0; else q1 <= 1;
    end

    assign qinc = q1;
    assign qdec = q0;
endmodule
