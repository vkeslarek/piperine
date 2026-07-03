`timescale 10ps / 1ps

module testbench ();
    reg en, d1, d2;
    wire [1:0] out;

    initial begin
	    en=0;
	    d1=0;
	    d2=1;
    end

    always #100 en=~en;

    always begin
	    #222 d1=~d1; 
	    #444 d2=~d2;
    end

    latch2 l0 (out, d1, d2, en);
endmodule
