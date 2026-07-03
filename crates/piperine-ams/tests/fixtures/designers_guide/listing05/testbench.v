`timescale 10ps / 1ps

module testbench ();
    reg clk, data;
    wire out;

    initial begin
	    clk=0;
	    data=0;
    end

    always #100 clk=~clk;
    always #333 data=~data; 

    dff dff0 (out, data, clk); 
endmodule
