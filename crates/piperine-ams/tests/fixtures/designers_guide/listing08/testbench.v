`timescale 10ps / 1ps

module testbench ();
    reg clk;
    integer i;
    
    initial begin
	    clk=0; 
	    i=1;
    end

    always begin
	    #i
	    clk=~clk;
	    i=i+1;
    end

    freq_meas fm1 (clk);
endmodule
