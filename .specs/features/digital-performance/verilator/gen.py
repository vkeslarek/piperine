#!/usr/bin/env python3
"""The Verilog twins of `crates/piperine-api/tests/digital_bench.rs`, for the Verilator reference of
`digital-performance/diagnosis.md`. Writes `<out>/<case>/top.v` and `<out>/<case>/tb.cpp` for every case;
`run.sh` builds and times them. Each twin is the same logic as its MSL case, two-state (Verilog has no
unknown byte), with every register observable through an output so nothing is optimised away."""
import pathlib
import sys

OUT = pathlib.Path(sys.argv[1])

SRC = """module src(input clk, output reg [31:0] q);
  initial q = 0;
  always @(posedge clk) q <= q + 1;
endmodule
"""
MIX = """module mix(input [31:0] a, output [31:0] y);
  assign y = (a ^ (a >> 3)) + 7;
endmodule
"""

def counter():
    return """module top(input clk, output reg [63:0] q);
  initial q = 0;
  always @(posedge clk) q <= q + 1;
endmodule
"""


def fsm():
    return """module top(input clk, output charge, output dump, output reg [31:0] laps);
  localparam IDLE = 0, CHARGE = 1, HOLD = 2, DUMP = 3;
  reg [1:0] phase; reg [7:0] ticks; reg [1:0] next; reg [7:0] t;
  initial begin phase = IDLE; ticks = 0; laps = 0; end
  assign charge = phase == CHARGE;
  assign dump = phase == DUMP;
  always @(posedge clk) begin
    t = ticks + 1;
    case (phase)
      IDLE: next = CHARGE;
      CHARGE: next = (t == 20) ? HOLD : CHARGE;
      HOLD: next = (t == 10) ? DUMP : HOLD;
      default: next = (t == 20) ? CHARGE : DUMP;
    endcase
    phase <= next;
    ticks <= (next != phase) ? 0 : t;
    if (phase == DUMP && next == CHARGE) laps <= laps + 1;
  end
endmodule
"""


def pipeline(n):
    return SRC + """module stage(input clk, input [31:0] d, output reg [31:0] q);
  wire [31:0] x = (d ^ (d >> 3)) + 7;
  initial q = 0;
  always @(posedge clk) q <= x;
endmodule
module top(input clk, output [31:0] last, output [31:0] first);
  wire [31:0] s [0:%d];
  src u0(clk, s[0]);
  genvar i;
  generate for (i = 1; i <= %d; i = i + 1) begin : g
    stage st(clk, s[i-1], s[i]);
  end endgenerate
  assign last = s[%d];
  assign first = s[0];
endmodule
""" % (n, n, n)


def flops(n):
    return SRC + """module tff(input clk, output reg q);
  initial q = 0;
  always @(posedge clk) q <= ~q;
endmodule
module top(input clk, output [%d:0] qs, output [31:0] first);
  src u0(clk, first);
  genvar i;
  generate for (i = 0; i < %d; i = i + 1) begin : g
    tff t(clk, qs[i]);
  end endgenerate
endmodule
""" % (n - 1, n)


def chain(d):
    return SRC + MIX + """module top(input clk, output [31:0] last, output [31:0] first);
  wire [31:0] c [0:%d];
  src u0(clk, c[0]);
  genvar i;
  generate for (i = 1; i <= %d; i = i + 1) begin : g
    mix m(c[i-1], c[i]);
  end endgenerate
  assign last = c[%d];
  assign first = c[0];
endmodule
""" % (d, d, d)


def dpwm():
    # examples/buck/src/dpwm.msl, line for line; `Never` becomes an assertion-free flag
    return SRC + """module top(input clk, output reg hs, output reg ls, output sample, output reg cut, output [31:0] first, output reg bad);
  localparam N_PERIOD = 400, DEAD = 4, SAMPLE_COUNT = 0;
  wire run = 1; wire [11:0] duty = 1000; wire oc = 0; wire sink_oc = 0;
  reg [8:0] count, width, hs_off; reg [2:0] frame; reg hs_cut, ls_cut;
  initial begin count = 0; width = 0; hs_off = 0; frame = 0; hs_cut = 0; ls_cut = 0; hs = 0; ls = 0; cut = 0; bad = 0; end
  assign sample = count == SAMPLE_COUNT;
  src u0(clk, first);
  function [2:0] rev3(input [2:0] k); rev3 = {k[0], k[1], k[2]}; endfunction
  function [8:0] dithered(input [11:0] d, input [2:0] f);
    dithered = (rev3(f) < d[2:0]) ? d[11:3] + 1 : d[11:3];
  endfunction
  reg wrap, hs_trip, ls_trip, hs_next, ls_next; reg [8:0] next, w, off_at;
  always @(posedge clk) begin
    wrap = count == N_PERIOD - 1;
    next = wrap ? 0 : count + 1;
    w = wrap ? dithered(duty, frame + 1) : width;
    hs_trip = !wrap && (hs_cut || (hs && oc));
    ls_trip = !wrap && (ls_cut || (ls && sink_oc));
    hs_next = run && !hs_trip && next < w;
    off_at = wrap ? w : (hs && !hs_next) ? next : hs_off;
    ls_next = run && !hs_next && !ls_trip && next >= off_at + DEAD && next < N_PERIOD - DEAD;
    count <= next; width <= w; hs_off <= off_at; hs_cut <= hs_trip; ls_cut <= ls_trip;
    hs <= hs_next; ls <= ls_next;
    if (wrap) begin frame <= frame + 1; cut <= hs_cut; end
    bad <= bad | (hs & ls);
  end
endmodule
"""


TB = """#include "Vtop.h"
#include "verilated.h"
#include <chrono>
#include <cstdio>
#include <cstdlib>
int main(int argc, char** argv) {
  const long cycles = atol(argv[1]);
  VerilatedContext ctx;
  Vtop top{&ctx};
  top.clk = 0;
  top.eval();
  auto t0 = std::chrono::steady_clock::now();
  for (long c = 0; c < cycles; ++c) {
    top.clk = 1; top.eval();
    top.clk = 0; top.eval();
  }
  double s = std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();
  printf("%ld %.6f %llu\\n", cycles, s, (unsigned long long)top.first_or_q());
  top.final();
  return 0;
}
"""

# case: (twin, the output the testbench prints, cycles: about a second each)
CASES = {
    "counter64": (counter, "q", 20_000_000),
    "fsm": (fsm, "laps", 20_000_000),
    "pipeline10": (lambda: pipeline(10), "first", 10_000_000),
    "pipeline100": (lambda: pipeline(100), "first", 5_000_000),
    "pipeline1000": (lambda: pipeline(1000), "first", 500_000),
    "flops100": (lambda: flops(100), "first", 10_000_000),
    "flops1000": (lambda: flops(1000), "first", 1_000_000),
    "flops10000": (lambda: flops(10000), "first", 200_000),
    "chain10": (lambda: chain(10), "first", 10_000_000),
    "chain100": (lambda: chain(100), "first", 5_000_000),
    "chain1000": (lambda: chain(1000), "first", 500_000),
    "dpwm": (dpwm, "first", 20_000_000),
}


def write(path, text):
    """Writes `text` only when it differs, so `run.sh` rebuilds only a changed twin."""
    if not path.exists() or path.read_text() != text:
        path.write_text(text)

for name, (make, probe, cycles) in CASES.items():
    d = OUT / name
    d.mkdir(parents=True, exist_ok=True)
    verilog = make()
    write(d / "top.v", verilog)
    write(d / "tb.cpp", TB.replace("first_or_q()", probe))
    write(d / "cycles", str(cycles))
