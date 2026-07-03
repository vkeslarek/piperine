`timescale 10ps / 1ps

module testbench ();
    reg clk;
    wire outclk;

    initial clk=0; 

    always #100 clk=~clk;


    counter ct0 (outclk, clk);
endmodule
