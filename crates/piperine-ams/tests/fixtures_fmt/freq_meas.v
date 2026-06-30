// Frequency measuring block
//
// Version 1a, 1 June 04
//
// Olaf Zinke
//
// Downloaded from The Designer's Guide Community (www.designers-guide.org).
// Post any questions on www.designers-guide.org/Forum.
// Taken from "The Designer's Guide to Verilog-AMS" by Kundert & Zinke.
// Chapter 4, Listing 8.

`timescale 1ns / 1ps

module freq_meas(clk);
    input clk;
    real last_time, current_time, freq;

    initial begin
        last_time = 0.0;
        freq = 0.0;
    end
    always @(posedge clk) begin
        current_time = $realtime;
        if (last_time > 0.0) freq = 1.0e9 / (current_time - last_time);
        last_time = current_time;
    end
endmodule

