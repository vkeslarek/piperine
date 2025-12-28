use std::collections::HashMap;

pub enum Exp {
    Tera,
    Giga,
    Mega,
    Kilo,
    Mil,
    Mili,
    Micro,
    Nano,
    Pico,
    Femto,
    Atto,
}

impl Exp {
    pub fn to_suffix(&self) -> &'static str {
        match self {
            Exp::Tera => "T",
            Exp::Giga => "G",
            Exp::Mega => "M",
            Exp::Kilo => "K",
            Exp::Mil => "mil",
            Exp::Mili => "m",
            Exp::Micro => "u",
            Exp::Nano => "n",
            Exp::Pico => "p",
            Exp::Femto => "f",
            Exp::Atto => "a",
        }
    }

    pub fn to_multiplier(&self) -> f64 {
        match self {
            Exp::Tera => 1e12,
            Exp::Giga => 1e9,
            Exp::Mega => 1e6,
            Exp::Kilo => 1e3,
            Exp::Mil => 0.001,
            Exp::Mili => 1e-3,
            Exp::Micro => 1e-6,
            Exp::Nano => 1e-9,
            Exp::Pico => 1e-12,
            Exp::Femto => 1e-15,
            Exp::Atto => 1e-18,
        }
    }
}

pub enum ParameterValue {
    Numeric(f64),
    Complex(f64, f64),
    Expression(String),
    Bool(bool),
    String(String),
}

pub struct Quantity {
    pub value: f64,
    pub multiplier: Option<Exp>,
    pub unit: Option<Unit>,
}

// Support for Look-Up Tables (LUTs)
pub struct TableData {
    pub points: Vec<(f64, f64)>,
}

pub enum Unit {
    Ohm,
    Siemens,
    Farad,
    Henry,
    Volt,
    Ampere,
    Watt,
    Hertz,
    Second,
}

pub struct Value(pub f64, pub Option<Exp>, pub Option<Unit>);

pub enum Node {
    Named(String),
    Indexed(usize),
    Ground,
}

pub type Nodes = Vec<Node>;

pub enum SweepType {
    Dec,
    Oct,
    Lin,
}

pub struct DCSweep {
    pub source: String,
    pub start: f64,
    pub stop: f64,
    pub step: f64,
}

pub enum PSSMethod {
    Shooting,
    HarmonicBalance,
}

pub enum Analysis {
    OperatingPoint,
    Transient {
        tstep: f64,
        tstop: f64,
        tstart: Option<f64>,
        tmax: Option<f64>,
        uic: bool,
    },
    AC {
        sweep_type: SweepType,
        points: usize,
        fstart: f64,
        fstop: f64,
    },
    DC {
        source: String,
        start: f64,
        stop: f64,
        step: f64,
        sweep2: Option<DCSweep>,
    },
    Noise {
        output: Node,
        reference: Node,
        src: String,
        sweep: SweepType,
        points: usize,
        fstart: f64,
        fstop: f64,
    },
    PZ {
        node1: Node,
        node2: Node,
        node3: Node,
        node4: Node,
        is_cur: bool,
        is_pol: bool,
    },
    Sens {
        pin_name: String,
        analysis: Box<Analysis>,
    },
    Distortion {
        sweep: SweepType,
        points: usize,
        fstart: f64,
        fstop: f64,
        f2overf1: Option<f64>,
    },
    PSS {
        f_fundamental: f64,
        n_harmonics: usize,
        n_steady_cycles: Option<usize>,
        method: PSSMethod,
    },
}

pub struct Circuit {
    pub title: String,
    pub parameters: HashMap<String, Value>,
    pub globals: Vec<Node>,
    pub models: HashMap<String, Model>,
    pub subcircuits: Vec<Circuit>,
    pub components: HashMap<String, Component>,
}

impl Default for Circuit {
    fn default() -> Self {
        Circuit {
            title: String::from("Untitled Circuit"),
            parameters: HashMap::new(),
            globals: Vec::new(),
            components: HashMap::new(),
            models: HashMap::new(),
            subcircuits: Vec::new(),
        }
    }
}

pub enum Component {
    Resistor {
        n1: Node,
        n2: Node,
        value: Value,
        model: Option<String>,
    },
    Capacitor {
        n1: Node,
        n2: Node,
        value: Value,
        ic: Option<f64>,
    },
    Inductor {
        n1: Node,
        n2: Node,
        value: Value,
        ic: Option<f64>,
    },

    // Transistors
    BJT {
        collector: Node,
        base: Node,
        emitter: Node,
        substrate: Option<Node>,
        model: String,
        area: Option<f64>,
    },
    MOSFET {
        drain: Node,
        gate: Node,
        source: Node,
        bulk: Node,
        model: String,
        l: f64,
        w: f64,
    },
    JFET {
        drain: Node,
        gate: Node,
        source: Node,
        model: String,
    },
    MESFET {
        drain: Node,
        gate: Node,
        source: Node,
        model: String,
    },

    // Controlled Sources
    VoltageControlledVoltageSource {
        pos: Node,
        neg: Node,
        ctrl_pos: Node,
        ctrl_neg: Node,
        gain: Value,
    },
    VoltageControlledCurrentSource {
        pos: Node,
        neg: Node,
        ctrl_pos: Node,
        ctrl_neg: Node,
        transconductance: Value,
    },
    CurrentControlledCurrentSource {
        pos: Node,
        neg: Node,
        ctrl_source: String,
        gain: Value,
    },
    CurrentControlledVoltageSource {
        pos: Node,
        neg: Node,
        ctrl_source: String,
        transresistance: Value,
    },

    // Sources
    VoltageSource {
        n1: Node,
        n2: Node,
        functions: Vec<SourceFunction>,
    },
    CurrentSource {
        n1: Node,
        n2: Node,
        function: SourceFunction,
    },

    // Transmission Lines
    LosslessLine {
        n1: Node,
        n2: Node,
        n3: Node,
        n4: Node,
        z0: f64,
        td: f64,
    },
    LossyLine {
        n1: Node,
        n2: Node,
        n3: Node,
        n4: Node,
        model: String,
    },

    // XSPICE & Behavioral
    XSpiceCodeModel {
        name: String,
        ports: Vec<Node>,
        model: String,
    },
    VerilogA {
        nodes: Vec<Node>,
        module: String,
        parameters: HashMap<String, ParameterValue>,
    },

    BehavioralSource {
        pos: Node,
        neg: Node,
        expression: String,
        is_voltage: bool,
    },

    SubCircuit {
        name: String,
        nodes: Vec<Node>,
        params: HashMap<String, ParameterValue>,
    },

    // Transmission Line Types
    CPL {
        nodes: Vec<Node>,
        n_ref: Node,
        model: String,
        length: f64,
    },

    TXL {
        n1: Node,
        n2: Node,
        n3: Node,
        n4: Node,
        model: String,
        length: f64,
    },

    UniformlyDistributedRC {
        n1: Node,
        n2: Node,
        n_common: Node,
        model: String,
        length: f64,
        lumps: Option<usize>,
    },

    // Switches
    VoltageControlledSwitch {
        pos: Node,
        neg: Node,
        ctrl_pos: Node,
        ctrl_neg: Node,
        model: String,
    },

    CurrentControlledSwitch {
        pos: Node,
        neg: Node,
        ctrl_source: String,
        model: String,
    },
}

pub enum RandomDistribution {
    Uniform = 1,
    Gaussian = 2,
    Exponential = 3,
    Poisson = 4,
}

pub enum SourceFunction {
    DC(f64),
    AC {
        offset: f64,
        amplitude: Value,
        frequency: f64,
    },
    Pulse {
        v1: f64,
        v2: f64,
        delay: f64,
        rise: f64,
        fall: f64,
        width: f64,
        period: f64,
    },
    Sine {
        vo: f64,
        va: f64,
        freq: f64,
        delay: f64,
        theta: f64,
    },
    PieceWiseLinear(Vec<(f64, f64)>), // Time, Value pairs
    SingleFrequencyFM {
        vo: f64,
        va: f64,
        fc: f64,
        mdi: f64,
        fs: f64,
    },
    AM {
        sa: f64,
        oc: f64,
        fc: f64,
        td: f64,
    },

    Exponential {
        v1: f64,
        v2: f64,
        td1: f64,
        tau1: f64,
        td2: f64,
        tau2: f64,
    },

    TransientNoise {
        white_amp: f64,
        step: f64,
        points: usize,
        pink_amp: f64,
    },

    Random {
        dist_type: RandomDistribution,
        step: f64,
        delay: Option<f64>,
        p1: Option<f64>,
        p2: Option<f64>,
    },

    External {
        buffer_index: usize,
    },

    RFPort {
        port_num: usize,
        z0: f64,
        power: Option<f64>,
        freq: Option<f64>,
    },
}

pub enum Model {
    R,
    C,
    L,
    SW,
    CSW,
    URC,
    LTRA,
    D,
    NPN,
    PNP,
    NJF,
    PJF,
    NMOS,
    PMOS,
    NMF,
    PMF,
    VDMOS,
}

pub struct GlobalOptions {
    pub temp: f64,
    pub tnom: f64,
    pub gmin: f64,
    pub reltol: f64,
    pub abstol: f64,
    // ... hundreds more exist in NgSpice
}

/*
- Global variable TEMPER
- Global variable and set TNOM, TEMP
- Sweep through temperatures in analyses

- Control blocks
- CSParam -> Add vectors to the output
- Flow commands (if endeif else)
- Output fourier analysis
- Function definitions
- Initial Conditions
- Includes and libraries
- Measurements and probes
- NodeSEt ??
- Options parsing
- Defining parameters
- Plot/Print
- Saving and loading state
- Brace expressions

- Title is the first line of a netlist but can be set with .TITLE
- If the next line has a + it is a continuation of the previous line
- Comments start with * or ; or $\s or //
- If there is a continuation on the next line, the comment continues only the current line
- Subcircuits are defined with .SUBCKT and .ENDS and defines a new "component" type. The netlist of the subcircuit is declared.
- Globals define nodes that can be accessed from anywhere in the circuit
- String support is limited. SHould we consider in out designs?
- Subcircuit can have parameters with default values declared like <ident>=<value>
- Brace expressions are always evaluated statically
- Subcircuits and model names are GLOBAL and must have unique names
-

Expressions are one of:
    <atom> where <atom> is either a spice number or an identifier
    <unary-operator> <atom>
    <function-name> ( <expr> [ , <expr> ...] )
    <atom> <binary-operator> <expr>
    ( <expr> )

Unary operators (precedence 1):
Precedence1:    - !

Binary operators:
Precedence2:    ** ^
Precedence3:    * / % \
Precedence4:    + -
Precedence5:    == != <> <= >= < >
Precedence6:    &&
Precedence7:    ||

Ternary Operator:
Precedence8:    C?x:y

0 is evaluated as TRUE and non-zero as FALSE
Available functions:
sqrt sin cos tan sinh cosh tanh asin acos atan asinh
acosh atanh arctan exp ln log abs nint int floor ceil
pow(2) pwr(2) min(2) max(2) sgn ternary_fcn(3) gauss(3)
agauss(3) unif(2) aunif(2) limit(2) var vec

Sufixes available:
g meg k m u n p f

Function def:
.func <ident>(<param_list>) [=] { <expr> }

Conditional execution:
.if <boolean_expr>
.elseif <boolean_expr>
.else
.endif
OBS: SUBCKT INC LIB and PARAM are not supported inside IF blocks.

Params AND Functions are evaluated at compile time while non linear sources are the only place where runtime eval happens
- The parameter m on a instance is replaced by multiple parallel instances of the same component. The devices that support it:
    Capacitor
    Diode
    CCCS
    VCCS
    Current Source
    JFET
    Inductor
    MOSFET
    BJT
    Resistor
    Subcircuit
    MESFET

Resistor params:
    RXXXXXXX n+ n- <value> <mname> <l=length> <w=width>
        + <temp=val> <dtemp=val> <m=val> <ac=val> <scale=val>
        + <noisy = 0|1>
Resistor can be behavioral too:
    RXXXXXXX n+ n- R = ’expression’ <tc1=value> <tc2=value> <noisy=0>
    RXXXXXXX n+ n- ’expression’ <tc1=value> <tc2=value> <noisy=0>
        Obs:
        Simulating small valued resistors: If you need to simulate very small resistors (0.001 Ohm or less), you should use CCVS (transresistance). It is less
        efficient but improves overall numerical accuracy. Consider a small resistance as a large conductance.

Capacitor params:
    CXXXXXXX n+ n- <value> <mname> <l=length> <w=width> <m=val>
        + <scale=val> <temp=val> <dtemp=val> <ic=init_condition>
Behavioral capacitor:
    CXXXXXXX n+ n- C = ’expression’ <tc1=value> <tc2=value>
    CXXXXXXX n+ n- ’expression’ <tc1=value> <tc2=value>
    CXXXXXXX n+ n- Q = ’expression’ <tc1=value> <tc2=value>

Inductor params:
    LYYYYYYY n+ n- <value> <mname> <nt=val> <m=val>
        + <scale=val> <temp=val> <dtemp=val> <tc1=val>
        + <tc2=val> <ic=init_condition>
Behavioral inductor:
    LXXXXXXX n+ n- L = ’expression’ <tc1=value> <tc2=value>
    LXXXXXXX n+ n- ’expression’ <tc1=value> <tc2=value>

Switches:
    SXXXXXXX N+ N- NC+ NC- MODEL <ON><OFF>
    WYYYYYYY N+ N- VNAM MODEL <ON><OFF>

Coupled Inductors:
    KXXXXXXX LYYYYYYY LZZZZZZZ value

Sources
    VXXXXXXX N+ N- <<DC> DC/TRAN VALUE> <AC <ACMAG <ACPHASE>>>
        + <DISTOF1 <F1MAG <F1PHASE>>> <DISTOF2 <F2MAG <F2PHASE>>>
    IYYYYYYY N+ N- <<DC> DC/TRAN VALUE> <AC <ACMAG <ACPHASE>>>
        + <DISTOF1 <F1MAG <F1PHASE>>> <DISTOF2 <F2MAG <F2PHASE>>>

    Functions for sources:
    - PULSE(V1 V2 <TD> <TR> <TF> <PW> <PER> <NP>)
        V1 Initial value
        V2 Pulsed value
        TD Delay time
        TR Rise time
        TF Fall time time
        PW Pulse width
        PER Period
        NP Number of pulses (optional, default infinite)
    - SIN(VO VA <FREQ> <TD> <THETA> <PHASE>)
        VO Offset
        VA Amplitude
        FREQ Frequency
        TD Delay time
        THETA Damping factor
        PHASE Phase in degrees
     - EXP(V1 V2 <TD1> <TAU1> <TD2> <TAU2>)
        V1 Initial value
        V2 Final value
        TD1 Rise delay time
        TAU1 Rise time constant
        TD2 Fall delay time
        TAU2 Fall time constant
     - PWL(T1 V1 <T2 V2 ... TN VN>)
        T1 Time point 1
        V1 Value at time point 1
        ...
        TN Time point N
        VN Value at time point N

      - SFFM(VO VA <FM> <MDI> <FC> <TD> <PHASEM> <PHASEC>)
        VO Offset
        VA Amplitude
        FM Modulating frequency
        MDI Modulation index
        FC Carrier frequency
        TD Signal delay
        PHASEM Phase of modulating signal in degrees
        PHASEC Phase of carrier signal in degrees

      - AM(VO VMO <VMA> <FM> <FC> <TD> <PHASEM> <PHASEC>)
        VO Offset
        VMO Modulating signal offset
        VMA Modulating signal amplitude
        FM Modulating frequency
        FC Carrier frequency
        TD Signal delay
        PHASEM Phase of modulating signal in degrees
        PHASEC Phase of carrier signal in degrees

      - TRNOISE(NA NT NALPHA NAMP RTSAM RTSCAPT RTSEMT)
        NA RMS noise amplitude (Gaussian)
        NT Time step
        NALPHA Exponent of 1/f noise
        NAMP RMS amplitude of 1/f noise
        RTSAM Resistance for sampling noise
        RTSCAPT Resistance for capture noise
        RTSEMT Resistance for emitter noise

      - TRRANDOM(TYPE TS <TD <PARAM1 <PARAM2>>>)
        TYPE Type of distribution (1=Uniform, 2=Gaussian, 3=Exponential, 4=Poisson)
        TS Time step
        TD Delay time
        PARAM1 Parameter 1 (Uniform: Range, Gaussian: Std Dev, Exponential: Mean, Poisson: Lambda)
        PARAM2 Parameter 2 (Uniform: Offset, Gaussian: Mean, Exponential: Offset and Poisson: Offset)

      - EXTERNAL

     RF Ports:
       - RFPORT(PORTNUM Z0 <PWR> <FREQ>)
        PORTNUM Port number
        Z0 Characteristic impedance
        PWR Power (optional)
        FREQ Frequency (optional)

Behavioral Sources:
    VCCS:
        GXXXXXXX N+ N- NC+ NC- VALUE <m=val>
    VCVS:
        EXXXXXXX N+ N- NC+ NC- VALUE <m=val>
    CCCS:
        FXXXXXXX N+ N- VNAM VALUE <m=val>
    CCVS:
        HXXXXXXX N+ N- VNAM VALUE <m=val>

Non Linear Dependent Sources (using expressions):
    BXXXXXXX n+ n- <i=expr> <v=expr> <tc1=value> <tc2=value>
        + <temp=value> <dtemp=value>

    There are special variables that can be used in these sources:
        time - current simulation time
        temper - current temperature
        herz - current frequency in Hz

Non Lineas Voltage Source:
    VOL:
        EXXXXXXX n+ n- vol=’expr’
        EXXXXXXX n+ n- value={expr}
    TABLE:
        Exxx n1 n2 TABLE {expression} = (x0, y0) (x1, y1) (x2, y2)
    POLY:
        EXXXXXXX n+ n- POLY(ND) NC1+ NC1- (NC2+ NC2-...) P0 (P1...)
    LAPLACE: ?
    FREQ: ?
    AND/OR/NAND/NOR:
        EAND out1 out0 and(2) in1 0 in2 0 (0.5, 0) (2.8, 3.3)

Non Linear Current Source:
    CUR:
        GXXXXXXX n+ n- cur=’expr’ <m=val>
    VALUE:
        GXXXXXXX n+ n- value={expr} <m=val>
    TABLE:
        Gxxx n1 n2 TABLE {expression} = (x0, y0) <m=val>
    POLY:
        GXXXXXXX n+ n- POLY(ND) NC1+ NC1- (NC2+ NC2-...) P0 (P1...)
    LAPLACE: ?
    FREQ: ?

F and H sources are exclusive POLY:
    FNONLIN 100 101 POLY(2) VDD Vxx 0 0.0 13.6 0.2 0.005

Lossless Transmission Line:
    TXXXXXXX N1 N2 N3 N4 Z0=VALUE <TD=VALUE>
        + <F=FREQ <NL=NRMLEN>> <IC=V1, I1, V2, I2>

Lossy Transmission Lines:
    OXXXXXXX n1 n2 n3 n4 mname

Uniform Distributed RC Lines:
    UXXXXXXX n1 n2 n3 mname l=len <n=lumps>

Single Lossy Transmission Line (TXL):
    YXXXXXXX N1 0 N2 0 mname <LEN=LENGTH>

Coupled Multiconductor Line (CPL):
    PXXXXXXX NI1 NI2...NIX GND1 NO1 NO2...NOX GND2 mname <LEN=LENGTH>

Diodes
    DXXXXXXX n+ n- mname <area=val> <m=val> <pj=val> <off>
        + <ic=vd> <temp=val> <dtemp=val>
        + <lm=val> <wm=val> <lp=val> <wp=val>
OBS: Missing models for transmission LINES!

BJT:
    QXXXXXXX nc nb ne <ns> <tj> mname <area=val> <areac=val>
        + <areab=val> <m=val> <off> <ic=vbe,vce> <temp=val>
        + <dtemp=val>

JFET:
    JXXXXXXX nd ng ns mname <area> <off> <ic=vds,vgs> <temp=t>

MESFET:
    ZXXXXXXX ND NG NS MNAME <AREA> <OFF> <IC=VDS, VGS>

MOSFET:
    MXXXXXXX nd ng ns nb mname <m=val> <l=val> <w=val>
        + <ad=val> <as=val> <pd=val> <ps=val> <nrd=val>
        + <nrs=val> <off> <ic=vds, vgs, vbs> <temp=t>

Digital Devices:
    U<name> <basic type> [(<parameter value>*)]
        +<digital power node> <digital ground node> <node>*
        +<timing model name> <I/O model name>
        +[MNTYMXDLY=<delay select value>]
        +[IO_LEVEL=<interface subcircuit select value>]

            Standard gates:
                BUF buffer
                INV inverter
                AND AND gate
                NAND NAND gate
                OR OR gate
                NOR NOR gate
                XOR exclusive OR gate
                NXOR exclusive NOR gate
                BUFA buffer array
                INVA inverter array
                ANDA AND gate array
                NANDA NAND gate array
                ORA OR gate array
                NORA NOR gate array
                XORA exclusive OR gate array
                NXORA exclusive NOR gate array
                AO AND-OR compound gate
                OA OR-AND compound gate
                AOI AND-NOR compound gate
                OAI OR-NAND compound gate

             Tristate gates:
                BUF3 buffer
                INV3 inverter
                AND3 AND gate
                NAND3 NAND gate
                OR3 OR gate
                NOR3 NOR gate
                XOR3 exclusive OR gate
                NXOR3 exclusive NOR gate
                BUF3A buffer array
                INV3A inverter array
                AND3A AND gate array
                NAND3A NAND gate array
                OR3A OR gate array
                NOR3A NOR gate array
                XOR3A exclusive OR gate array
                NXOR3A exclusive NOR gate array
            Flip-flops and latches:
                DFF D-type flip-flop, positive-edge triggered
                JKFF J-K flip-flop, negative-edge triggered
                DLTCH D-type latch
                SRFF S-R flip-flop

            Delay lines:
                DLYLINE Delay line

            Behavioral primitives:
                LOGICEXP Combinational logic expressions
                PINDLY Output buffers and tristate buffers with estimated delays

TOTAL OPTIONS

*/