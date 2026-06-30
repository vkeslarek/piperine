`timescale 10ps / 1ps

module testbench ();
    reg clk;

    initial clk=0;

    always #100 clk=~clk;

    inverter inv0 (out, clk); 
endmodule
