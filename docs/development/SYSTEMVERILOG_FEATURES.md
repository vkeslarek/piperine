# SystemVerilog (IEEE 1800-2017) Feature Reference

Legend: **[DONE]** already in Piperine · **[ANALOG-ONLY]** not relevant for analog testbench · *(blank)* missing from Piperine

---

## 1. Data Types

### 1.1 Integer Types (2-state)

| Type | Width | Note |
|------|-------|------|
| `bit` | 1b | 0/1 only **[DONE]** |
| `byte` | 8b signed | C-like char |
| `shortint` | 16b signed | |
| `int` | 32b signed | **[DONE]** |
| `longint` | 64b signed | |
| `integer` | 32b 4-state | **[DONE]** |
| `shortreal` | 32b float | C `float` |
| `real` | 64b float | **[DONE]** |
| `realtime` | 64b float time | **[DONE]** |
| `time` | 64b unsigned | **[DONE]** |

### 1.2 Logic (4-state)

| Type | Note |
|------|------|
| `logic` | 4-state: 0/1/X/Z, replaces `reg` **[DONE]** |
| `reg` | Legacy Verilog synonym for `logic` |
| `wire` | Net type, structural |
| `tri`, `triand`, `trior`, `tri0`, `tri1`, `trireg`, `wand`, `wor` | Specific drive/resolution net types **[ANALOG-ONLY]** |
| `supply0`, `supply1` | Power/ground nets **[ANALOG-ONLY]** |

```systemverilog
logic [7:0] data;         // 8-bit 4-state vector  [DONE]
bit   [7:0] addr;         // 8-bit 2-state vector  [DONE]
```

### 1.3 Packed vs Unpacked Vectors

```systemverilog
logic [7:0] packed_byte;           // packed: one 8b value  [DONE]
logic [7:0] mem [0:255];           // unpacked: 256 8b values
bit   [3:0][7:0] packed2d;         // multi-dim packed vector
```

Packed vectors: contiguous in memory, can be sliced/shifted. Unpacked: array of elements.

### 1.4 User-Defined Types

**typedef** — alias for any type:
```systemverilog
typedef logic [7:0] byte_t;
typedef int unsigned uint_t;
typedef enum { IDLE, RUN, DONE } state_t;
```

**enum** — named constants with base type:
```systemverilog
typedef enum logic [1:0] { RED=2'b00, YEL=2'b01, GRN=2'b10 } light_t;
light_t sig = RED;
// Methods: first() last() next(N) prev(N) name() num()
for (light_t s = s.first(); s != s.last(); s = s.next()) ...
$display("%s", sig.name());   // prints "RED"
```

**struct** — named aggregate:
```systemverilog
typedef struct {
    logic [7:0] opcode;
    logic [3:0] flags;
    int         payload;
} pkt_t;

pkt_t p;
p.opcode = 8'hFF;

// packed struct: contiguous bits, can be assigned as integer
typedef struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
} nibble_pair_t;
```

**union** — shared memory:
```systemverilog
typedef union {
    int    i;
    real   r;
} num_u;

// Tagged union: safe discriminated union
typedef union tagged {
    int    Int;
    real   Real;
} tagged_u;
tagged_u tu = tagged Int 42;
```

### 1.5 Special Types

| Type | Description |
|------|-------------|
| `void` | No return value (function/task) |
| `string` | Dynamic-length string **[DONE]** |
| `chandle` | Opaque C pointer (DPI) |
| `event` | Synchronization object |
| `type` parameter | Type parameterization in class/module |

### 1.6 Net Types (SV 2012+)

```systemverilog
nettype real wreal;          // user-defined net type (Verilog-AMS origin)
interconnect [7:0] bus;      // abstract interconnect for mixed-abstraction
```

### 1.7 Type Casting

```systemverilog
int i = int'(3.7);            // static cast: truncates to 3
real r = real'(42);           // widen int to real
state_t s = state_t'(2);      // enum cast from int
$cast(s, some_int_expr);      // dynamic cast: returns 0 on fail (as function)
logic [15:0] v = 16'(byte_val); // width cast
signed'(unsigned_val)         // signedness cast
```

### 1.8 Parameter Types

```systemverilog
module m #(type T = int) (...);   // type parameter
    T val;
endmodule
m #(real) inst (...);             // instantiate with real
```

---

## 2. Operators & Expressions

### 2.1 Arithmetic **[DONE]**
```
+  -  *  /  %  **  (power)
```

### 2.2 Relational **[DONE]**
```
<  <=  >  >=
```

### 2.3 Equality **[DONE]**
```
==  !=              // logical: X if any operand has X/Z
===  !==            // case: exact X/Z match, always 0 or 1
==?  !=?            // wildcard: ? matches any bit
```

### 2.4 Logical **[DONE]**
```
&&  ||  !
```

### 2.5 Bitwise **[DONE]**
```
&  |  ^  ~  ~^  ^~
```

### 2.6 Unary Reduction **[DONE]**
```
&word   ~&word   |word   ~|word   ^word   ~^word
```

### 2.7 Shift **[DONE]**
```
<<  >>   (logical)
<<<  >>> (arithmetic, sign-extends for signed types)
```

### 2.8 Concatenation & Replication **[DONE]**
```systemverilog
{a, b, c}        // concatenation
{4{byte_val}}    // replication (32-bit result)
```

### 2.9 Ternary **[DONE]**
```systemverilog
cond ? expr_true : expr_false
```

### 2.10 Assignment Operators **[DONE]**
```
=  +=  -=  *=  /=  %=  &=  |=  ^=  <<=  >>=  <<<=  >>>=
```

### 2.11 Increment / Decrement
```systemverilog
i++   ++i   i--   --i
```

### 2.12 `inside` Operator — Set Membership
```systemverilog
if (x inside {3, 5, [10:20], [100:$]})  // $ = max value
// also in constraints:
constraint c { x inside {[0:7]}; }
```

### 2.13 Streaming Operators — Pack/Unpack
```systemverilog
{<<4{data}}        // stream right, 4-bit chunks (reverse nibbles)
{>>8{data}}        // stream left, 8-bit chunks
int arr[] = {<<8{32'hDEADBEEF}};  // byte-reverse
```

### 2.14 `with` Clause — Iterator Expression
Used in array methods and `foreach` constraints:
```systemverilog
int q[$] = '{1,2,3,4};
q.sum() with (item * item)  // sum of squares
q.find() with (item > 2)    // returns {3,4}
```

---

## 3. Procedural Statements & Control Flow

### 3.1 `if / else` **[DONE]**
```systemverilog
if (x > 0) ...
else if (x == 0) ...
else ...
```

### 3.2 `case / casez / casex` **[DONE]**
```systemverilog
case (state)
    IDLE:  ...
    RUN:   ...
    default: ...
endcase

casez (ir) 4'b1???: ...    // z/? = don't care
casex (ir) 4'b1xxx: ...    // x/z/? = don't care
```

### 3.3 `unique` / `unique0` / `priority` Case Modifiers
```systemverilog
unique  case (sel) ...   // error if overlap or no match
unique0 case (sel) ...   // error if overlap, ok if no match
priority case (sel) ...  // first match wins; warning if no match
```

### 3.4 `for` / `foreach` / `while` / `do…while` / `repeat` / `forever` **[DONE]**
```systemverilog
for (int i = 0; i < 10; i++) ...
foreach (arr[i, j]) arr[i][j] = 0;  // multi-dim foreach
while (cond) ...
do ... while (cond);
repeat (N) ...
forever begin ... end
```

### 3.5 `break` / `continue` / `return` **[DONE]**

### 3.6 `wait` Statement
```systemverilog
wait (sig == 1);             // level-sensitive wait
wait fork;                   // wait for all spawned processes
```

### 3.7 Event Controls
```systemverilog
@(posedge clk);              // clock edge
@(negedge rst_n);
@(clk);                      // any change
@(a or b or c);              // multi-signal sensitivity
@*;                          // implicit sensitivity (combinational)
#10;                         // delay
##2;                         // cycle delay (clocking block)
```

### 3.8 Timing Controls in Testbench
```systemverilog
#100ns;
#1.5;
@(posedge clk iff data_valid);    // conditional clock edge
```

### 3.9 `disable` Statement
```systemverilog
disable my_task;         // terminate named task/block
disable fork;            // kill all child processes
```

### 3.10 Named Blocks and Hierarchical References
```systemverilog
begin : my_block
    int x;   // local to block
    ...
end : my_block
disable my_block;
```

---

## 4. Tasks & Functions

### 4.1 Functions **[DONE]**
```systemverilog
function automatic real calc(input real x, input real y);
    return x * y;
endfunction
```

### 4.2 Tasks **[DONE]**
```systemverilog
task automatic drive(input logic [7:0] data, output logic ack);
    @(posedge clk);
    bus = data;
    @(posedge ack_sig);
    ack = 1;
endtask
```

### 4.3 `automatic` vs `static`

- `automatic`: each call gets its own stack frame (default in classes, required for recursion)
- `static`: one copy shared across all calls (Verilog legacy default)

### 4.4 Argument Passing

```systemverilog
// Directions: input, output, inout, ref (pass by reference), const ref
function void swap(ref int a, ref int b);
    int tmp = a; a = b; b = tmp;
endfunction

// Default argument values
function int add(int a, int b = 1);
    return a + b;
endfunction
add(5);    // b defaults to 1
```

### 4.5 Void Functions (Callable as Statements)
```systemverilog
function void log(string msg);
    $display("[LOG] %s", msg);
endfunction
log("started");   // called as statement, no return
```

### 4.6 Function/Task Overloading
Not natively supported — use parameterized classes or DPI.

### 4.7 Exported Tasks (Consume Time)
Tasks may consume simulation time (blocking); functions may not.

---

## 5. Classes & OOP

### 5.1 Class Declaration
```systemverilog
class Packet;
    // Properties
    int id;
    rand bit [7:0] data;
    static int count = 0;    // shared across all instances

    // Constructor
    function new(int i = 0);
        id = i;
        count++;
    endfunction

    // Methods
    function void display();
        $display("id=%0d data=%h", id, data);
    endfunction

    // Virtual method
    virtual function string kind();
        return "Packet";
    endfunction
endclass
```

### 5.2 Object Handles and `new`
```systemverilog
Packet p;          // handle (null by default)
p = new(42);       // allocate object
if (p == null) ... // null check
p = null;          // release handle
```

### 5.3 Inheritance
```systemverilog
class EthPacket extends Packet;
    bit [47:0] src_mac;

    function new();
        super.new(99);   // call parent constructor
        src_mac = 0;
    endfunction

    virtual function string kind();
        return "EthPacket";
    endfunction
endclass
```

### 5.4 Polymorphism & `virtual`
```systemverilog
Packet p = new EthPacket();    // base handle, derived object
$cast(eth_p, p);               // dynamic downcast
$display(p.kind());            // "EthPacket" — virtual dispatch
```

### 5.5 Abstract Classes
```systemverilog
virtual class BaseDriver;
    pure virtual task drive(input Packet p);  // must override
endclass
```

### 5.6 Access Control
```systemverilog
class Foo;
    local  int private_val;    // accessible only inside class
    protected int prot_val;   // accessible in class + subclasses
    int public_val;            // default: public
endclass
```

### 5.7 `this` and `super`
```systemverilog
function new(int id);
    this.id = id;        // disambiguate property vs argument
    super.new();
endfunction
```

### 5.8 Parameterized Classes
```systemverilog
class Stack #(type T = int, int DEPTH = 16);
    T mem [DEPTH];
    int top = 0;
    function void push(T val); mem[top++] = val; endfunction
    function T pop(); return mem[--top]; endfunction
endclass

Stack #(real, 32) float_stack = new();
```

### 5.9 Static Members
```systemverilog
class Counter;
    static int count = 0;
    function new(); count++; endfunction
    static function int get_count(); return count; endfunction
endclass
Counter::get_count()   // call without object
```

### 5.10 `typedef class` — Forward Reference
```systemverilog
typedef class B;
class A;
    B b_handle;   // legal: forward ref
endclass
class B; ... endclass
```

### 5.11 Copying Objects
```systemverilog
// Shallow copy
Packet p2 = new p1;    // copies all fields, shares sub-objects
// Deep copy: manual copy() method convention (no built-in)
```

---

## 6. Interfaces & Modports

### 6.1 Interface Declaration
```systemverilog
interface bus_if (input logic clk);
    logic [7:0] data;
    logic        valid;
    logic        ready;

    // Clocking block (testbench timing)
    clocking tb_cb @(posedge clk);
        default input #1step output #2ns;
        input  ready;
        output data, valid;
    endclocking

    // Modport: DUT view
    modport dut_mp (input data, valid, output ready, input clk);
    // Modport: testbench view
    modport tb_mp  (clocking tb_cb, input clk);
endinterface
```

### 6.2 Interface Instantiation
```systemverilog
bus_if bif(.clk(clk));    // instantiate
my_dut dut(.bus(bif.dut_mp));
```

### 6.3 Virtual Interface (For Classes)
```systemverilog
virtual bus_if vif;       // handle to an interface — passed to objects
// In class:
class Driver;
    virtual bus_if vif;
    function new(virtual bus_if v); vif = v; endfunction
    task drive(...); @(vif.tb_cb); vif.tb_cb.data <= 8'hAB; endtask
endclass
```

### 6.4 Interface Methods and Assertions
Interfaces can contain tasks, functions, `always` blocks, and concurrent assertions — checked automatically during simulation.

---

## 7. Packages & Namespaces

### 7.1 Package Declaration
```systemverilog
package my_pkg;
    // Contents: typedef, function, task, class, parameter, localparam, import
    typedef enum { A, B, C } state_t;
    parameter int MAX = 256;
    function int clamp(int v, int lo, int hi);
        return (v < lo) ? lo : (v > hi) ? hi : v;
    endfunction
    import other_pkg::*;   // re-export pattern
endpackage
```

### 7.2 Import
```systemverilog
import my_pkg::state_t;    // explicit — only state_t
import my_pkg::*;          // wildcard — all used items
```

### 7.3 Scope Resolution
```systemverilog
my_pkg::state_t s = my_pkg::A;   // no import needed
```

### 7.4 `$unit` Scope
Declarations outside any module/package/program: globally visible. Avoid — prefer packages.

### 7.5 Search Order
Local → explicit import → wildcard import → `$unit`

---

## 8. Clocking Blocks & Synchronization

### 8.1 Clocking Block
```systemverilog
clocking cb @(posedge clk);
    default input #1step output #2ns;   // skew
    input  req, data;     // testbench reads these
    output ack;           // testbench drives these
endclocking
```

Skew: `#1step` samples in Postponed region (avoids races). Output skew delays driving after clock edge.

### 8.2 Cycle Delay `##`
```systemverilog
##1;            // wait 1 clock cycle (uses default clocking)
##3 cb.ack;     // 3 cycles then check ack
```

### 8.3 Default Clocking
```systemverilog
default clocking cb;   // all ## delays use this clocking block
```

### 8.4 Global Clocking
```systemverilog
global clocking gc @(posedge clk); endclocking
// Used by $global_clock in SVA
```

---

## 9. Assertions (SVA — Concurrent & Immediate)

### 9.1 Immediate Assertions **[DONE]**
```systemverilog
assert (x > 0) else $error("x not positive");
assert #0 (x > 0);    // deferred: evaluated in Observed region
```

### 9.2 Sequences
```systemverilog
sequence s_req_ack;
    req ##[1:4] ack;    // req then ack within 1–4 cycles
endsequence

// Repetition operators
a [*3]          // a true exactly 3 times consecutively
a [*1:5]        // 1 to 5 consecutive
a [->2]         // a true at least 2 non-consecutive times (goto)
a [=3]          // a true at least 3 non-consecutive times (non-consecutive)
first_match(s)  // match on first occurrence
s1 throughout s2  // s1 must hold during all of s2
s1 within s2      // s1 starts/ends within s2
s1 intersect s2   // same length, both true
s1 and s2         // both true, may differ in length
s1 or  s2         // either true
```

### 9.3 Properties
```systemverilog
property p_req_ack;
    @(posedge clk) disable iff (rst)
    req |-> ##[1:4] ack;
endproperty
```

**Implication operators:**
- `|->` overlapping: antecedent ends, consequent starts same cycle
- `|=>` non-overlapping: consequent starts next cycle

**Linear temporal:**
```systemverilog
always p           // holds on every cycle
s_always [n:m] p   // holds on cycles n to m
eventually p       // eventually holds
s_eventually p
until p q          // p holds until q
strong(s)          // s must eventually match
weak(s)            // match not required but checked if it starts
```

### 9.4 Assertion Directives
```systemverilog
assert property (p_req_ack);   // check property, error on fail
assume property (p);           // formal tool: input constraint
cover  property (p);           // collect functional coverage
restrict property (p);         // formal only: simulation ignores
```

### 9.5 SVA System Functions
```systemverilog
$rose(sig)         // sig was 0, now 1
$fell(sig)         // sig was 1, now 0
$stable(sig)       // sig unchanged from last clock
$changed(sig)      // sig changed from last clock
$past(sig)         // sampled value N cycles ago: $past(sig, N)
$past(sig, 2, enable)
$sampled(expr)     // sampled value at current Observed region
$isunknown(sig)    // any X/Z bit?
$onehot(sig)       // exactly one bit set?
$onehot0(sig)      // zero or one bit set?
$countones(sig)    // number of 1 bits
```

### 9.6 `expect` Statement
```systemverilog
// In procedural code: block until property holds or fails
expect (@(posedge clk) ##[1:10] done) else $error("timeout");
```

---

## 10. Randomization & Constraints

### 10.1 Random Variables
```systemverilog
class Pkt;
    rand  bit [7:0] data;    // random, any value each call
    randc bit [3:0] id;      // random-cyclic: cycles through all values
    int             fixed;   // not randomized
endclass
```

### 10.2 Constraint Blocks
```systemverilog
constraint c_range {
    data inside {[0:100]};
}
constraint c_cond {
    if (mode == 0) data < 50;
    else           data >= 50;
}
constraint c_impl {
    (flag == 1) -> data > 10;    // implication
}
constraint c_dist {
    data dist { 0 := 1, [1:9] :/ 5, 10 := 1 };
    //           ^weight    ^spread weight equally
}
constraint c_foreach {
    foreach (arr[i]) arr[i] inside {[0:255]};
}
constraint c_solve {
    solve mode before data;    // solve mode first
}
```

### 10.3 `randomize()` Method
```systemverilog
Pkt p = new();
if (!p.randomize()) $fatal(1, "randomization failed");
// With inline constraint:
p.randomize() with { data > 50; };
// Disable a constraint:
p.randomize() with { c_range.constraint_mode(0); };
```

### 10.4 `std::randomize` — Standalone
```systemverilog
int x;
std::randomize(x) with { x inside {[1:100]}; };
```

### 10.5 Hooks
```systemverilog
function void pre_randomize();   // called before randomize()
function void post_randomize();  // called after — good for derived fields
```

### 10.6 Constraint Inheritance & Override
```systemverilog
class ExtPkt extends Pkt;
    constraint c_range { data inside {[50:100]}; }   // override
endclass
```

### 10.7 `constraint_mode` / `rand_mode`
```systemverilog
p.c_range.constraint_mode(0);   // disable constraint
p.data.rand_mode(0);            // make data non-random
```

---

## 11. Coverage

### 11.1 Covergroup Declaration
```systemverilog
covergroup cg_pkt @(posedge clk);
    cp_data: coverpoint data {
        bins low  = {[0:63]};
        bins high = {[64:127]};
        bins other = default;
    }
    cp_cmd: coverpoint cmd {
        bins read  = {CMD_READ};
        bins write = {CMD_WRITE};
        wildcard bins any_rd = {4'b?1??};       // wildcard bin
        ignore_bins idle  = {CMD_IDLE};         // not counted
        illegal_bins rsvd = {CMD_RSVD};         // error if hit
    }
    // Transition coverage
    cp_trans: coverpoint state {
        bins rst_to_run = (IDLE => RUN);
        bins run_loop   = (RUN  => RUN[*3]);    // 3 consecutive
    }
    // Cross coverage
    cx: cross cp_data, cp_cmd;
endcovergroup
```

### 11.2 Coverage Options
```systemverilog
covergroup cg with function sample(bit [7:0] d);
    option.per_instance = 1;     // separate stats per instance
    option.at_least     = 3;     // need N hits per bin
    option.auto_bin_max = 256;   // max auto bins
    option.goal         = 95;    // % coverage goal
    option.comment      = "...";
endcovergroup
```

### 11.3 Explicit Sample / Query
```systemverilog
cg cg_inst = new();
cg_inst.sample();                // manual trigger
real cov = cg_inst.get_coverage();
real overall = $get_coverage();  // total across all covergroups
```

---

## 12. Arrays — Advanced

### 12.1 Array Types

| Type | Syntax | Index | Size |
|------|--------|-------|------|
| Fixed unpacked | `int a [8]` | 0..7 | compile-time **[DONE]** |
| Dynamic | `int a []` | 0..N-1 | runtime **[DONE]** |
| Associative | `int a [string]` | any type | sparse |
| Queue | `int a [$]` | 0..$ | auto-resize **[DONE]** |
| Multi-dim | `int a [4][8]` | nested | nested |

### 12.2 Dynamic Array Methods
```systemverilog
int d[];
d = new[10];              // allocate 10 elements
d = new[20](d);           // resize, copy existing
d.delete();               // free
int sz = d.size();
```

### 12.3 Associative Array
```systemverilog
int aa [string];
aa["hello"] = 1;
if (aa.exists("hello")) ...
aa.delete("hello");       // delete one entry
aa.delete();              // delete all
int num = aa.num();       // entry count
// Iteration:
string key;
if (aa.first(key)) do $display("%s=%0d", key, aa[key]); while(aa.next(key));
aa.last(key);             // last key
aa.prev(key);             // previous key
```

### 12.4 Queue Methods **[DONE: push_back, pop_front, size, min, max, sum]**
```systemverilog
int q[$];
q.push_back(x);    q.push_front(x);
q.pop_back();      q.pop_front();
q.insert(idx, x);
q.delete(idx);     q.delete();    // delete one or all
q.size();
```

### 12.5 Array Manipulation Methods (all array types)

**Ordering:**
```systemverilog
arr.sort();                      // ascending
arr.sort() with (item.field);    // sort by field
arr.rsort();                     // descending
arr.reverse();
arr.shuffle();                   // random order
```

**Reduction:**
```systemverilog
arr.sum()                        // [DONE]
arr.sum() with (item * item)     // sum of squares
arr.product()
arr.and()    arr.or()    arr.xor()
```

**Locator (return array of matches):**
```systemverilog
q2 = arr.find()       with (item > 5);        // matching elements
qi = arr.find_index() with (item > 5);        // matching indices
q2 = arr.find_first() with (item > 5);
qi = arr.find_first_index() with (item > 5);
q2 = arr.find_last()  with (item > 5);
qi = arr.find_last_index()  with (item > 5);
q2 = arr.min();   q2 = arr.max();
q2 = arr.unique();               // unique elements
qi = arr.unique_index();
```

---

## 13. Strings — Full Method Set

```systemverilog
string s = "hello world";

// Length
int n = s.len();                   // 11

// Substrings
string sub = s.substr(6, 10);      // "world"
s.putc(0, "H");                    // in-place char replace
byte c = s.getc(0);               // 'h'

// Case
string up = s.toupper();           // "HELLO WORLD"
string lo = s.tolower();           // "hello world"

// Comparison (C-style: 0 = equal, neg = less, pos = greater)
int cmp = s.compare("hello");
int icmp = s.icompare("HELLO");    // case-insensitive

// Conversion: string → number
int    i = s.atoi();
real   r = s.atoreal();
int    h = s.atohex();
int    o = s.atooct();
int    b = s.atobin();

// Conversion: number → string (in-place)
s.itoa(42);
s.hextoa(255);     // "ff"
s.octtoa(8);       // "10"
s.bintoa(5);       // "101"
s.realtoa(3.14);

// $sformat — formatted string creation
string out;
$sformat(out, "val=%0d hex=%h", 255, 255);  // "val=255 hex=ff"
$sformatf("val=%0d", 42);                   // returns string directly
```

---

## 14. Processes & Concurrency

### 14.1 `fork / join` Variants
```systemverilog
fork
    task_a();    // runs concurrently
    task_b();
    begin        // sequential sub-block as one thread
        #10 task_c();
    end
join           // wait for ALL threads to finish

fork
    task_a();
    task_b();
join_any       // continue when ANY one thread finishes

fork
    task_a();   // starts but parent doesn't wait
    task_b();
join_none      // parent continues immediately
```

### 14.2 `wait fork` / `disable fork`
```systemverilog
fork task_a(); task_b(); join_none
// ... do other stuff ...
wait fork;        // now wait for all outstanding children

fork task_long(); join_none
#100;
disable fork;     // kill all child processes
```

### 14.3 `process` Class — Fine-Grain Control
```systemverilog
process p;
fork
    begin
        p = process::self();    // capture own handle
        #50;
    end
join_none
p.kill();          // terminate
p.suspend();       // pause
p.resume();        // restart
p.await();         // wait for completion (like join)
// p.status() returns: FINISHED, RUNNING, WAITING, SUSPENDED, KILLED
```

### 14.4 `automatic` in Fork
```systemverilog
// Each iteration must have its own copy of i:
for (int i = 0; i < N; i++) begin
    automatic int j = i;   // snapshot for this thread
    fork
        $display("thread %0d", j);
    join_none
end
wait fork;
```

---

## 15. Events, Mailboxes & Semaphores

### 15.1 Events
```systemverilog
event done;
-> done;           // trigger (blocking: all waiters see it in this timestep)
->> done;          // nonblocking trigger (Reactive region)
@(done);           // wait for trigger
@(done.triggered); // level-sensitive: true if already triggered this timestep
wait (done.triggered);
```

### 15.2 Mailbox (FIFO IPC)
```systemverilog
mailbox #(int) mb = new(8);   // typed, capacity 8 (0=unbounded)
// In producer:
mb.put(42);           // blocking if full
mb.try_put(42);       // returns 0 if full
mb.num();             // items in mailbox
// In consumer:
int val;
mb.get(val);          // blocking if empty
mb.try_get(val);      // returns 0 if empty
mb.peek(val);         // copy without remove, blocking
mb.try_peek(val);     // non-blocking peek
```

### 15.3 Semaphore (Mutex / Token Pool)
```systemverilog
semaphore sem = new(1);   // 1 key = mutex
sem.get(1);               // acquire (blocking)
sem.try_get(1);           // non-blocking, returns 0 if unavailable
sem.put(1);               // release
sem.get(N);               // acquire N keys at once
```

---

## 16. System Tasks & Functions (Complete List)

### 16.1 Display / Write / Format **[DONE: $display, $write, $strobe]**
```systemverilog
$display("fmt", args...);   // newline appended
$write("fmt", args...);     // no newline
$strobe("fmt", args...);    // end of timestep
$monitor("fmt", args...);   // prints when any arg changes
$monitoron; $monitoroff;
$displayb / $displayh / $displayo  // force radix
```

### 16.2 File I/O
```systemverilog
int fd = $fopen("file.txt", "w");
$fdisplay(fd, "fmt", args...);
$fwrite(fd, "fmt", args...);
$fstrobe(fd, "fmt", args...);
$fmonitor(fd, "fmt", args...);
$fclose(fd);
string line;
int code = $fgets(line, fd);     // read line
code = $fscanf(fd, "%d %f", i, r);
code = $sscanf(str, "%d", i);    // scan from string
$rewind(fd);
$fseek(fd, offset, origin);
int pos = $ftell(fd);
$fflush(fd);
int eof = $feof(fd);
```

### 16.3 Simulation Control **[DONE: $finish, $fatal]**
```systemverilog
$finish;          $finish(0|1|2);   // 0=quiet, 1=stats, 2=verbose
$stop;                               // halt (interactive)
$exit;                               // clean exit from program block
```

### 16.4 Timescale
```systemverilog
$time;            // [DONE] integer time in time units
$realtime;        // [DONE] real time
$stime;           // 32-bit $time
$scale(val);      // scale a time value
$timeformat(-9, 2, "ns", 10);   // set display format
```

### 16.5 Math **[DONE: $sqrt $pow $exp $ln $log10 $sin $cos $floor $ceil $abs]**
```systemverilog
$atan(x)   $atan2(y,x)   $asin(x)   $acos(x)
$tan(x)    $cosh(x)      $sinh(x)   $tanh(x)
$acosh(x)  $asinh(x)     $atanh(x)
$hypot(x,y)              // sqrt(x²+y²)
$min(a,b)  $max(a,b)
$clog2(n)                // ceiling(log2(n)), for param sizing
```

### 16.6 Random **[DONE: $random, $urandom]**
```systemverilog
$random(seed);            // 32b signed, seeded
$urandom(seed);           // 32b unsigned
$urandom_range(hi, lo);   // uniform in [lo, hi]
$dist_uniform(seed, lo, hi);
$dist_normal(seed, mean, stddev);
$dist_exponential(seed, mean);
$dist_poisson(seed, mean);
$dist_chi_square(seed, dof);
$dist_t(seed, dof);
$dist_erlang(seed, k, mean);
```

### 16.7 Bit & Type Queries
```systemverilog
$bits(expr)               // bit width of expression
$size(arr)                // array size (first dim)
$size(arr, dim)           // size of dimension dim
$dimensions(arr)          // number of dimensions
$high(arr, dim)           // highest index of dim
$low(arr, dim)            // lowest index of dim
$left(arr, dim)           // left bound
$right(arr, dim)          // right bound
$increment(arr, dim)      // +1 or -1 (direction)
$unpacked_dimensions(arr) // unpacked dimension count
$isunknown(expr)          // 1 if any X/Z bit
$countbits(expr, ctl...)  // count bits matching control values
$countones(expr)          // count '1' bits
$onehot(expr)             // exactly one '1'
$onehot0(expr)            // zero or one '1'
```

### 16.8 Type Conversion
```systemverilog
$itor(i)         // int → real
$rtoi(r)         // real → int (truncate)
$bitstoreal(b)   // 64-bit pattern → real (reinterpret)
$realtobits(r)   // real → 64-bit pattern
$bitstoshortreal(b)
$shortrealtobits(r)
$signed(expr)    // force signed interpretation
$unsigned(expr)  // force unsigned
```

### 16.9 Cast
```systemverilog
$cast(dest, src);    // dynamic cast: returns 1 on success (as function)
                     // runtime error as task on fail
```

### 16.10 String Formatting
```systemverilog
$sformat(s, "fmt", args...);    // write to string variable
$sformatf("fmt", args...);      // return formatted string
```

### 16.11 Severity **[DONE: $fatal $error $warning $info]**
```systemverilog
$fatal(finish_num, "msg");   // 0=don't call $finish
$error("msg", args...);
$warning("msg", args...);
$info("msg", args...);
```

### 16.12 VCD / Dump
```systemverilog
$dumpfile("waves.vcd");
$dumpvars(depth, module_inst);
$dumpon; $dumpoff;
$dumpall; $dumpflush;
$dumplimit(size);
```

### 16.13 PLI / Memory Image
```systemverilog
$readmemb("file.txt", mem);      // load binary
$readmemh("file.hex", mem);      // load hex
$writememb("file.txt", mem);
$writememh("file.hex", mem);
$readmemh("f", mem, start, stop);
```

### 16.14 Simulation Misc
```systemverilog
$system("cmd");          // execute shell command
$test$plusargs("opt");   // check +opt on command line
$value$plusargs("N=%d", val);  // parse +N=42
```

### 16.15 Assertion Control
```systemverilog
$asserton;   $assertoff;          // global
$asserton(level, scope);          // scoped
$assertkill;  $assertpasson;  $assertpassoff;
$assertfailon; $assertfailoff;
$asserthit;   $assertnonvacuouson;
$get_coverage();                  // total functional coverage %
```

---

## 17. Compiler Directives

```systemverilog
`define  NAME  value       // text macro
`NAME                      // use macro
`undef   NAME              // undefine
`ifdef   NAME ... `endif
`ifndef  NAME ... `endif
`elsif   NAME
`else
`include "file.sv"
`timescale 1ns/1ps
`default_nettype none      // disable implicit nets (best practice)
`line N "file" lvl
`resetall                  // reset all directives
`begin_keywords "1800-2017"
`end_keywords
`undefineall
// Predefined macros:
`__FILE__    `__LINE__
```

---

## 18. Generate Constructs

### 18.1 `for` Generate
```systemverilog
genvar i;
generate
    for (i = 0; i < N; i++) begin : gen_block
        assign out[i] = in[N-1-i];   // name: gen_block[0], gen_block[1], ...
    end
endgenerate
```

### 18.2 `if` Generate
```systemverilog
generate
    if (WIDTH == 8) begin : gen_8
        my_8bit_inst inst(...);
    end else begin : gen_other
        my_Nbit_inst #(WIDTH) inst(...);
    end
endgenerate
```

### 18.3 `case` Generate
```systemverilog
generate
    case (IMPL_TYPE)
        "BRAM":  bram_impl inst(...);
        "DIST":  dist_impl inst(...);
        default: basic_impl inst(...);
    endcase
endgenerate
```

---

## 19. Attributes

Carry metadata through the toolchain:
```systemverilog
(* synthesis, keep = 1 *)  wire sig;
(* full_case, parallel_case *)  case (sel) ...
(* dont_touch = "true" *)  reg q;
```

---

## 20. DPI — Direct Programming Interface

### 20.1 Import (Call C from SV)
```systemverilog
import "DPI-C" function real sin(real x);             // direct name
import "DPI-C" function void my_sv = c_my_func(int n); // rename
import "DPI-C" pure function real mymath(real x);      // no side effects
import "DPI-C" context task my_task(inout int v);      // accesses SV data
```

### 20.2 Export (Call SV from C)
```systemverilog
export "DPI-C" function my_sv_func;
export "DPI-C" task    my_sv_task;
```

### 20.3 Type Mapping

| SV Type | C Type |
|---------|--------|
| `byte` | `char` |
| `int` | `int` |
| `longint` | `long long` |
| `shortreal` | `float` |
| `real` | `double` |
| `chandle` | `void*` |
| `string` | `const char*` (input) / `char**` (output) |
| `bit [N:0]` | `svBit` / `svBitVecVal*` |
| `logic [N:0]` | `svLogic` / `svLogicVecVal*` |

### 20.4 Open Arrays (Variable-Dimension)
```systemverilog
import "DPI-C" function void process(input int arr[]);
// C sees: svOpenArrayHandle
```

---

## 21. Additional Hardware/Structural Constructs

### 21.1 `program` Block
```systemverilog
program automatic test;
    initial begin
        // Runs in Reactive region — avoids testbench/DUT races
    end
endprogram
```

### 21.2 `bind` — Attach Assertions/Coverage to DUT
```systemverilog
bind my_dut my_checker chk_inst (.clk(clk), .data(data));
// Inserts instance into my_dut's scope without modifying DUT
```

### 21.3 `checker` Block (IEEE 1800-2012+)
```systemverilog
checker req_ack_checker(logic req, ack, clk);
    sequence s; @(posedge clk) req ##[1:5] ack; endsequence
    property p; @(posedge clk) req |-> ##[1:5] ack; endproperty
    assert property (p);
    cover  sequence (s);
endchecker
```

### 21.4 `config` Block
```systemverilog
config my_cfg;
    design work.top;
    cell top use work.top;
    instance top.u1 use work.fast_adder;
endconfig
```
Selects specific implementations at elaboration time. Largely simulator-specific.

### 21.5 `specify` / `specparam` [ANALOG-ONLY]
Path delays for timing analysis — synthesis/STA context.

---

## 22. Scheduling & Simulation Regions

Not language features per se, but affect testbench correctness:

| Region | What runs |
|--------|-----------|
| Active | `always`, `initial`, NBA scheduling |
| NBA | Non-blocking assignment updates (`<=`) |
| Observed | Concurrent assertion evaluation (sampled values) |
| Reactive | `program` blocks, clocking block drives |
| Postponed | `$monitor`, `$strobe`, `$past` sampling |

Rule: testbench drives in Reactive, DUT reads in Active → no races. Clocking blocks + `program` blocks enforce this automatically.

---

## Summary: Piperine Gap vs SystemVerilog

### Wave 1 — implemented 2026-06

Landed and covered by `tests/e2e_onda1_test.rs`:

- `++` / `--` (prefix and postfix) and compound assignment `+= -= *= /= %=`
- Loop control: `break`, `continue`, `return` (interpreter `Flow` signal)
- `repeat (n)` and `forever` loops
- Brace blocks `{ … }` interchangeable with `begin`/`end`
- Math system functions: `$sqrt $pow $exp $ln $log10 $sin $cos $tan $asin $acos
  $atan $atan2 $sinh $cosh $tanh $hypot $floor $ceil` and `$clog2`

Still missing from the lists below (the `[DONE]` markers elsewhere in this file
predate verification and are aspirational): user-defined `function`/`task`
execution, arrays/queues, `inside`, `$urandom`/`$dist_*`, associative arrays,
`package`.

### High Value for Analog Testbench (implement soon)

| Feature | Why Needed |
|---------|-----------|
| `++` / `--` | Natural loop variable, sweep counters |
| `inside` operator | Clean range checks in testbench logic |
| `typedef` / `enum` / `struct` | Named states, sweep configs, result records |
| `string` methods (full set) | Filename generation, CSV output, parsing |
| `$sformatf` | Dynamic filename generation for MC sweeps |
| `$urandom_range` | Uniform random for Monte Carlo |
| `$dist_normal` / `$dist_uniform` | Statistical distributions |
| `package` | Shared constants, device parameter sets across files |
| `longint` / `byte` / `shortint` | Complete integer set |
| `automatic` functions | Recursion, scoped temporaries |
| Named blocks + `disable` | Early exit from nested loops |
| `wait (expr)` | Level-sensitive synchronization |
| `$clog2` | Sizing calculations |
| `$bits` / `$size` | Array/type introspection |
| Associative array `[string]` | Named result sets, parameter dictionaries |
| Array locator methods | Find extrema across MC runs |
| Array sort/shuffle | Result ordering |
| `void` functions | Callable-as-statement helpers |
| `process` class | Timeout control in long simulations |
| `event` + `->` | Sequencing between `initial` blocks |

### Medium Priority (verification features)

| Feature | Note |
|---------|------|
| `class` + inheritance | Base driver/monitor pattern; useful for multi-DUT testbenches |
| `rand` / `constraint` | Full constrained random (Monte Carlo already handled via `$dist_*`) |
| `covergroup` | Functional coverage — useful but not blocking |
| `mailbox` / `semaphore` | Needed if running parallel analyses |
| `fork/join` variants | Parallel sweeps across ngspice workers |
| `virtual interface` | If Piperine ever supports multi-DUT scenarios |

### Low Priority / Not Applicable

| Feature | Note |
|---------|------|
| Concurrent SVA | Analog testbench uses `always @(step)` SOA monitors instead |
| Clocking blocks | ngspice has no concept of digital clock edges |
| `program` block | Verilog-AMS `initial` serves this role |
| `bind` / `config` | Structural elaboration handled by Piperine's own flow |
| `generate` | Piperine's parametric `initial` loop is the analog equivalent |
| DPI | Use `extern task` / Piperine plugin mechanism instead |
| Net types (tri/wand/wor) | [ANALOG-ONLY] ngspice handles node resolution |
| `specify` / `specparam` | [ANALOG-ONLY] timing analysis only |
| UVM | Framework on top of SV — Piperine provides its own runtime |
