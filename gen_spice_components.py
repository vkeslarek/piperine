import os

COMPONENTS = [
    {
        "struct": "SpiceResistor",
        "name": "res",
        "prefix": "R",
        "ports": ["p", "n"],
        "params": [
            ("r", "real", None, "{}"),
            ("model", "string", '""', "{model}"),
            ("ac", "real", "0.0", "AC={ac}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}"),
            ("l", "real", "0.0", "L={l}"),
            ("w", "real", "0.0", "W={w}"),
            ("m", "real", "1.0", "M={m}"),
            ("tc1", "real", "0.0", "TC1={tc1}"),
            ("tc2", "real", "0.0", "TC2={tc2}"),
            ("scale", "real", "1.0", "SCALE={scale}"),
            ("noisy", "integer", "1", "NOISY={noisy}"),
            ("bv_max", "real", "0.0", "BV_MAX={bv_max}")
        ]
    },
    {
        "struct": "SpiceCapacitor",
        "name": "cap",
        "prefix": "C",
        "ports": ["p", "n"],
        "params": [
            ("c", "real", None, "{}"),
            ("model", "string", '""', "{model}"),
            ("ic", "real", "0.0", "IC={ic}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}"),
            ("w", "real", "0.0", "W={w}"),
            ("l", "real", "0.0", "L={l}"),
            ("m", "real", "1.0", "M={m}"),
            ("tc1", "real", "0.0", "TC1={tc1}"),
            ("tc2", "real", "0.0", "TC2={tc2}"),
            ("scale", "real", "1.0", "SCALE={scale}"),
            ("bv_max", "real", "0.0", "BV_MAX={bv_max}")
        ]
    },
    {
        "struct": "SpiceInductor",
        "name": "ind",
        "prefix": "L",
        "ports": ["p", "n"],
        "params": [
            ("l", "real", None, "{}"),
            ("model", "string", '""', "{model}"),
            ("ic", "real", "0.0", "IC={ic}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}"),
            ("m", "real", "1.0", "M={m}"),
            ("tc1", "real", "0.0", "TC1={tc1}"),
            ("tc2", "real", "0.0", "TC2={tc2}"),
            ("scale", "real", "1.0", "SCALE={scale}"),
            ("nt", "real", "0.0", "NT={nt}")
        ]
    },
    {
        "struct": "SpiceMutual",
        "name": "mutual",
        "prefix": "K",
        "ports": [],
        "params": [
            ("inductor1", "string", None, "{inductor1}"),
            ("inductor2", "string", None, "{inductor2}"),
            ("k", "real", "1.0", "{k}")
        ]
    },
    {
        "struct": "SpiceVoltageSource",
        "name": "vsource",
        "prefix": "V",
        "ports": ["p", "n"],
        "params": [
            ("dc", "real", "0.0", "DC {dc}"),
            ("acmag", "real", "0.0", "AC {acmag}"),
            ("acphase", "real", "0.0", "{acphase}")
        ]
    },
    {
        "struct": "SpiceCurrentSource",
        "name": "isource",
        "prefix": "I",
        "ports": ["p", "n"],
        "params": [
            ("dc", "real", "0.0", "DC {dc}"),
            ("acmag", "real", "0.0", "AC {acmag}"),
            ("acphase", "real", "0.0", "{acphase}")
        ]
    },
    {
        "struct": "SpiceVpulse",
        "name": "vpulse",
        "prefix": "V",
        "ports": ["p", "n"],
        "params": [
            ("v0", "real", "0.0", ""),
            ("v1", "real", "1.0", ""),
            ("td", "real", "0.0", ""),
            ("tr", "real", "1e-9", ""),
            ("tf", "real", "1e-9", ""),
            ("pw", "real", "10e-9", ""),
            ("per", "real", "20e-9", "")
        ],
        "custom_format": "PULSE({v0} {v1} {td} {tr} {tf} {pw} {per})"
    },
    {
        "struct": "SpiceIpulse",
        "name": "ipulse",
        "prefix": "I",
        "ports": ["p", "n"],
        "params": [
            ("i0", "real", "0.0", ""),
            ("i1", "real", "1.0", ""),
            ("td", "real", "0.0", ""),
            ("tr", "real", "1e-9", ""),
            ("tf", "real", "1e-9", ""),
            ("pw", "real", "10e-9", ""),
            ("per", "real", "20e-9", "")
        ],
        "custom_format": "PULSE({i0} {i1} {td} {tr} {tf} {pw} {per})"
    },
    {
        "struct": "SpiceVsin",
        "name": "vsin",
        "prefix": "V",
        "ports": ["p", "n"],
        "params": [
            ("vo", "real", "0.0", ""),
            ("va", "real", "1.0", ""),
            ("freq", "real", "1e6", ""),
            ("td", "real", "0.0", ""),
            ("theta", "real", "0.0", ""),
            ("phase", "real", "0.0", "")
        ],
        "custom_format": "SIN({vo} {va} {freq} {td} {theta} {phase})"
    },
    {
        "struct": "SpiceIsin",
        "name": "isin",
        "prefix": "I",
        "ports": ["p", "n"],
        "params": [
            ("io", "real", "0.0", ""),
            ("ia", "real", "1.0", ""),
            ("freq", "real", "1e6", ""),
            ("td", "real", "0.0", ""),
            ("theta", "real", "0.0", ""),
            ("phase", "real", "0.0", "")
        ],
        "custom_format": "SIN({io} {ia} {freq} {td} {theta} {phase})"
    },
    {
        "struct": "SpiceVexp",
        "name": "vexp",
        "prefix": "V",
        "ports": ["p", "n"],
        "params": [
            ("v1", "real", "0.0", ""),
            ("v2", "real", "1.0", ""),
            ("td1", "real", "0.0", ""),
            ("tau1", "real", "1e-9", ""),
            ("td2", "real", "50e-9", ""),
            ("tau2", "real", "1e-9", "")
        ],
        "custom_format": "EXP({v1} {v2} {td1} {tau1} {td2} {tau2})"
    },
    {
        "struct": "SpiceIexp",
        "name": "iexp",
        "prefix": "I",
        "ports": ["p", "n"],
        "params": [
            ("i1", "real", "0.0", ""),
            ("i2", "real", "1e-3", ""),
            ("td1", "real", "0.0", ""),
            ("tau1", "real", "1e-9", ""),
            ("td2", "real", "50e-9", ""),
            ("tau2", "real", "1e-9", "")
        ],
        "custom_format": "EXP({i1} {i2} {td1} {tau1} {td2} {tau2})"
    },
    {
        "struct": "SpiceVpwl",
        "name": "vpwl",
        "prefix": "V",
        "ports": ["p", "n"],
        "params": [
            ("points", "string", None, "")
        ],
        "custom_format": "PWL({points})"
    },
    {
        "struct": "SpiceIpwl",
        "name": "ipwl",
        "prefix": "I",
        "ports": ["p", "n"],
        "params": [
            ("points", "string", None, "")
        ],
        "custom_format": "PWL({points})"
    },
    {
        "struct": "SpiceVsffm",
        "name": "vsffm",
        "prefix": "V",
        "ports": ["p", "n"],
        "params": [
            ("vo", "real", "0.0", ""),
            ("va", "real", "1.0", ""),
            ("fc", "real", "1e6", ""),
            ("mdi", "real", "1.0", ""),
            ("fs", "real", "1e4", ""),
            ("phasec", "real", "0.0", ""),
            ("phases", "real", "0.0", "")
        ],
        "custom_format": "SFFM({vo} {va} {fc} {mdi} {fs} {phasec} {phases})"
    },
    {
        "struct": "SpiceIsffm",
        "name": "isffm",
        "prefix": "I",
        "ports": ["p", "n"],
        "params": [
            ("io", "real", "0.0", ""),
            ("ia", "real", "1.0", ""),
            ("fc", "real", "1e6", ""),
            ("mdi", "real", "1.0", ""),
            ("fs", "real", "1e4", ""),
            ("phasec", "real", "0.0", ""),
            ("phases", "real", "0.0", "")
        ],
        "custom_format": "SFFM({io} {ia} {fc} {mdi} {fs} {phasec} {phases})"
    },
    {
        "struct": "SpiceVam",
        "name": "vam",
        "prefix": "V",
        "ports": ["p", "n"],
        "params": [
            ("sa", "real", "1.0", ""),
            ("fc", "real", "1e6", ""),
            ("fm", "real", "1e4", ""),
            ("td", "real", "0.0", ""),
            ("phases", "real", "0.0", "")
        ],
        "custom_format": "AM({sa} {fc} {fm} {td} {phases})"
    },
    {
        "struct": "SpiceIam",
        "name": "iam",
        "prefix": "I",
        "ports": ["p", "n"],
        "params": [
            ("sa", "real", "1.0", ""),
            ("fc", "real", "1e6", ""),
            ("fm", "real", "1e4", ""),
            ("td", "real", "0.0", ""),
            ("phases", "real", "0.0", "")
        ],
        "custom_format": "AM({sa} {fc} {fm} {td} {phases})"
    },
    {
        "struct": "SpiceVnoise",
        "name": "vnoise",
        "prefix": "V",
        "ports": ["p", "n"],
        "params": [
            ("na", "real", "0.0", ""),
            ("nt", "real", "1e-9", ""),
            ("nalpha", "real", "0.0", ""),
            ("namp", "real", "0.0", "")
        ],
        "custom_format": "TRNOISE({na} {nt} {nalpha} {namp})"
    },
    {
        "struct": "SpiceInoise",
        "name": "inoise",
        "prefix": "I",
        "ports": ["p", "n"],
        "params": [
            ("na", "real", "0.0", ""),
            ("nt", "real", "1e-9", ""),
            ("nalpha", "real", "0.0", ""),
            ("namp", "real", "0.0", "")
        ],
        "custom_format": "TRNOISE({na} {nt} {nalpha} {namp})"
    },
    {
        "struct": "SpiceVrandom",
        "name": "vrandom",
        "prefix": "V",
        "ports": ["p", "n"],
        "params": [
            ("rtype", "integer", "1", ""),
            ("ts", "real", "1e-9", ""),
            ("td", "real", "0.0", ""),
            ("param1", "real", "0.5", ""),
            ("param2", "real", "0.0", "")
        ],
        "custom_format": "TRRANDOM({rtype} {ts} {td} {param1} {param2})"
    },
    {
        "struct": "SpiceIrandom",
        "name": "irandom",
        "prefix": "I",
        "ports": ["p", "n"],
        "params": [
            ("rtype", "integer", "1", ""),
            ("ts", "real", "1e-9", ""),
            ("td", "real", "0.0", ""),
            ("param1", "real", "0.5", ""),
            ("param2", "real", "0.0", "")
        ],
        "custom_format": "TRRANDOM({rtype} {ts} {td} {param1} {param2})"
    },
    {
        "struct": "SpiceVcvs",
        "name": "vcvs",
        "prefix": "E",
        "ports": ["p", "n", "cp", "cn"],
        "params": [
            ("gain", "real", "1.0", "{gain}")
        ]
    },
    {
        "struct": "SpiceVccs",
        "name": "vccs",
        "prefix": "G",
        "ports": ["p", "n", "cp", "cn"],
        "params": [
            ("gm", "real", "1e-3", "{gm}"),
            ("m", "real", "1.0", "M={m}")
        ]
    },
    {
        "struct": "SpiceCcvs",
        "name": "ccvs",
        "prefix": "H",
        "ports": ["p", "n"],
        "params": [
            ("vsrc", "string", None, "{vsrc}"),
            ("transres", "real", "1.0", "{transres}")
        ]
    },
    {
        "struct": "SpiceCccs",
        "name": "cccs",
        "prefix": "F",
        "ports": ["p", "n"],
        "params": [
            ("vsrc", "string", None, "{vsrc}"),
            ("gain", "real", "1.0", "{gain}"),
            ("m", "real", "1.0", "M={m}")
        ]
    },
    {
        "struct": "SpiceBSourceV",
        "name": "bsource_v",
        "prefix": "B",
        "ports": ["p", "n"],
        "params": [
            ("V", "string", None, "V={V}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}"),
            ("tc1", "real", "0.0", "TC1={tc1}"),
            ("tc2", "real", "0.0", "TC2={tc2}"),
            ("reciproctc", "integer", "0", "RECIPROCTC={reciproctc}")
        ]
    },
    {
        "struct": "SpiceBSourceI",
        "name": "bsource_i",
        "prefix": "B",
        "ports": ["p", "n"],
        "params": [
            ("I", "string", None, "I={I}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}"),
            ("tc1", "real", "0.0", "TC1={tc1}"),
            ("tc2", "real", "0.0", "TC2={tc2}"),
            ("reciproctc", "integer", "0", "RECIPROCTC={reciproctc}")
        ]
    },
    {
        "struct": "SpiceVsw",
        "name": "vsw",
        "prefix": "S",
        "ports": ["p", "n", "cp", "cn"],
        "params": [
            ("model", "string", None, "{model}"),
            ("on", "integer", "0", "ON={on}"),
            ("off", "integer", "0", "OFF={off}")
        ]
    },
    {
        "struct": "SpiceIsw",
        "name": "isw",
        "prefix": "W",
        "ports": ["p", "n"],
        "params": [
            ("vsrc", "string", None, "{vsrc}"),
            ("model", "string", None, "{model}"),
            ("on", "integer", "0", "ON={on}"),
            ("off", "integer", "0", "OFF={off}")
        ]
    },
    {
        "struct": "SpiceDiode",
        "name": "d",
        "prefix": "D",
        "ports": ["a", "c"],
        "params": [
            ("model", "string", None, "{model}"),
            ("area", "real", "1.0", "AREA={area}"),
            ("pj", "real", "0.0", "PJ={pj}"),
            ("w", "real", "0.0", "W={w}"),
            ("l", "real", "0.0", "L={l}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("ic", "real", "0.0", "IC={ic}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}")
        ]
    },
    {
        "struct": "SpiceNpn",
        "name": "npn",
        "prefix": "Q",
        "ports": ["c", "b", "e"],
        "params": [
            ("model", "string", None, "{model}"),
            ("area", "real", "1.0", "AREA={area}"),
            ("areab", "real", "1.0", "AREAB={areab}"),
            ("areac", "real", "1.0", "AREAC={areac}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("icvbe", "real", "0.0", "ICVBE={icvbe}"),
            ("icvce", "real", "0.0", "ICVCE={icvce}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}")
        ]
    },
    {
        "struct": "SpicePnp",
        "name": "pnp",
        "prefix": "Q",
        "ports": ["c", "b", "e"],
        "params": [
            ("model", "string", None, "{model}"),
            ("area", "real", "1.0", "AREA={area}"),
            ("areab", "real", "1.0", "AREAB={areab}"),
            ("areac", "real", "1.0", "AREAC={areac}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("icvbe", "real", "0.0", "ICVBE={icvbe}"),
            ("icvce", "real", "0.0", "ICVCE={icvce}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}")
        ]
    },
    {
        "struct": "SpiceNpn4",
        "name": "npn4",
        "prefix": "Q",
        "ports": ["c", "b", "e", "sub"],
        "params": [
            ("model", "string", None, "{model}"),
            ("area", "real", "1.0", "AREA={area}"),
            ("areab", "real", "1.0", "AREAB={areab}"),
            ("areac", "real", "1.0", "AREAC={areac}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("icvbe", "real", "0.0", "ICVBE={icvbe}"),
            ("icvce", "real", "0.0", "ICVCE={icvce}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}")
        ]
    },
    {
        "struct": "SpicePnp4",
        "name": "pnp4",
        "prefix": "Q",
        "ports": ["c", "b", "e", "sub"],
        "params": [
            ("model", "string", None, "{model}"),
            ("area", "real", "1.0", "AREA={area}"),
            ("areab", "real", "1.0", "AREAB={areab}"),
            ("areac", "real", "1.0", "AREAC={areac}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("icvbe", "real", "0.0", "ICVBE={icvbe}"),
            ("icvce", "real", "0.0", "ICVCE={icvce}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}")
        ]
    },
    {
        "struct": "SpiceNmos",
        "name": "nmos",
        "prefix": "M",
        "ports": ["d", "g", "s", "b"],
        "params": [
            ("model", "string", None, "{model}"),
            ("w", "real", "1e-6", "W={w}"),
            ("l", "real", "100e-9", "L={l}"),
            ("ad", "real", "0.0", "AD={ad}"),
            ("as_", "real", "0.0", "AS={as_}"),
            ("pd", "real", "0.0", "PD={pd}"),
            ("ps", "real", "0.0", "PS={ps}"),
            ("nrd", "real", "0.0", "NRD={nrd}"),
            ("nrs", "real", "0.0", "NRS={nrs}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("icvds", "real", "0.0", "ICVDS={icvds}"),
            ("icvgs", "real", "0.0", "ICVGS={icvgs}"),
            ("icvbs", "real", "0.0", "ICVBS={icvbs}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}"),
            ("nf", "real", "1.0", "NF={nf}"),
            ("sa", "real", "0.0", "SA={sa}"),
            ("sb", "real", "0.0", "SB={sb}")
        ]
    },
    {
        "struct": "SpicePmos",
        "name": "pmos",
        "prefix": "M",
        "ports": ["d", "g", "s", "b"],
        "params": [
            ("model", "string", None, "{model}"),
            ("w", "real", "1e-6", "W={w}"),
            ("l", "real", "100e-9", "L={l}"),
            ("ad", "real", "0.0", "AD={ad}"),
            ("as_", "real", "0.0", "AS={as_}"),
            ("pd", "real", "0.0", "PD={pd}"),
            ("ps", "real", "0.0", "PS={ps}"),
            ("nrd", "real", "0.0", "NRD={nrd}"),
            ("nrs", "real", "0.0", "NRS={nrs}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("icvds", "real", "0.0", "ICVDS={icvds}"),
            ("icvgs", "real", "0.0", "ICVGS={icvgs}"),
            ("icvbs", "real", "0.0", "ICVBS={icvbs}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}"),
            ("nf", "real", "1.0", "NF={nf}"),
            ("sa", "real", "0.0", "SA={sa}"),
            ("sb", "real", "0.0", "SB={sb}")
        ]
    },
    {
        "struct": "SpiceJfetN",
        "name": "jfet_n",
        "prefix": "J",
        "ports": ["d", "g", "s"],
        "params": [
            ("model", "string", None, "{model}"),
            ("area", "real", "1.0", "AREA={area}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("ic", "real", "0.0", "IC={ic}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}")
        ]
    },
    {
        "struct": "SpiceJfetP",
        "name": "jfet_p",
        "prefix": "J",
        "ports": ["d", "g", "s"],
        "params": [
            ("model", "string", None, "{model}"),
            ("area", "real", "1.0", "AREA={area}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("ic", "real", "0.0", "IC={ic}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}")
        ]
    },
    {
        "struct": "SpiceMesfetN",
        "name": "mesfet_n",
        "prefix": "Z",
        "ports": ["d", "g", "s"],
        "params": [
            ("model", "string", None, "{model}"),
            ("area", "real", "1.0", "AREA={area}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("icvds", "real", "0.0", "ICVDS={icvds}"),
            ("icvgs", "real", "0.0", "ICVGS={icvgs}")
        ]
    },
    {
        "struct": "SpiceMesfetP",
        "name": "mesfet_p",
        "prefix": "Z",
        "ports": ["d", "g", "s"],
        "params": [
            ("model", "string", None, "{model}"),
            ("area", "real", "1.0", "AREA={area}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("icvds", "real", "0.0", "ICVDS={icvds}"),
            ("icvgs", "real", "0.0", "ICVGS={icvgs}")
        ]
    },
    {
        "struct": "SpiceVdmos",
        "name": "vdmos",
        "prefix": "M",
        "ports": ["d", "g", "s"],
        "params": [
            ("model", "string", None, "{model}"),
            ("w", "real", "1e-3", "W={w}"),
            ("l", "real", "1e-6", "L={l}"),
            ("m", "real", "1.0", "M={m}"),
            ("off", "integer", "0", "OFF={off}"),
            ("icvds", "real", "0.0", "ICVDS={icvds}"),
            ("icvgs", "real", "0.0", "ICVGS={icvgs}"),
            ("temp", "real", "27.0", "TEMP={temp}"),
            ("dtemp", "real", "0.0", "DTEMP={dtemp}")
        ]
    },
    {
        "struct": "SpiceTline",
        "name": "tline",
        "prefix": "T",
        "ports": ["ap", "an", "bp", "bn"],
        "params": [
            ("z0", "real", "50.0", "Z0={z0}"),
            ("td", "real", "1e-9", "TD={td}"),
            ("f", "real", "0.0", "F={f}"),
            ("nl", "real", "0.25", "NL={nl}"),
            ("v1", "real", "0.0", "V1={v1}"),
            ("v2", "real", "0.0", "V2={v2}"),
            ("i1", "real", "0.0", "I1={i1}"),
            ("i2", "real", "0.0", "I2={i2}")
        ]
    },
    {
        "struct": "SpiceLtra",
        "name": "ltra",
        "prefix": "O",
        "ports": ["ap", "an", "bp", "bn"],
        "params": [
            ("model", "string", None, "{model}"),
            ("v1", "real", "0.0", "V1={v1}"),
            ("v2", "real", "0.0", "V2={v2}"),
            ("i1", "real", "0.0", "I1={i1}"),
            ("i2", "real", "0.0", "I2={i2}")
        ]
    },
    {
        "struct": "SpiceUrc",
        "name": "urc",
        "prefix": "U",
        "ports": ["a", "b", "ref_"],
        "params": [
            ("model", "string", None, "{model}"),
            ("length", "real", "1e-3", "L={length}"),
            ("n", "integer", "0", "N={n}")
        ]
    },
    {
        "struct": "SpiceCpl",
        "name": "cpl",
        "prefix": "P",
        "ports": [],
        "params": [
            ("ports", "string", None, "{ports}"),
            ("model", "string", None, "{model}"),
            ("length", "real", "1.0", "length={length}"),
            ("dimension", "integer", "0", "dimension={dimension}")
        ]
    },
    {
        "struct": "SpiceTxl",
        "name": "txl",
        "prefix": "Y",
        "ports": ["y1p", "y1n"],
        "params": [
            ("model", "string", None, "{model}"),
            ("length", "real", "1.0", "length={length}")
        ]
    },
    {
        "struct": "SpicePort",
        "name": "port",
        "prefix": "P",
        "ports": ["p", "n"],
        "params": [
            ("num", "integer", "1", "PORT={num}"),
            ("z0", "real", "50.0", "Z0={z0}")
        ]
    },
    {
        "struct": "SpiceSubckt",
        "name": "subckt",
        "prefix": "X",
        "ports": [],
        "params": [
            ("ports", "string", None, "{ports}"),
            ("subckt_name", "string", None, "{subckt_name}"),
            ("params", "string", '""', "{params}")
        ]
    }
]

import textwrap

def sanitize_keyword(p):
    if p.endswith('_'):
        return p[:-1]
    return p

def gen_hardware_rs():
    out = []
    out.append("""\
use piperine_circuit::{
    HardwareDefinition, HardwareInstance,
    NetResolver, PortDefinition, ParameterDefinition,
    ParameterMap, ConnectionMap, ElaborationError,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn spice_name(prefix: char, name: &str) -> String {
    if name.chars().next().map(|c| c.to_ascii_uppercase()) == Some(prefix.to_ascii_uppercase()) {
        name.to_string()
    } else {
        format!("{prefix}{name}")
    }
}

fn require_net<'a>(
    connections: &'a ConnectionMap,
    port: &str,
    instance: &str,
) -> Result<&'a str, ElaborationError> {
    connections.get(port).map(|s| s.as_str()).ok_or_else(|| {
        ElaborationError::ConnectionError {
            instance: instance.to_string(),
            detail: format!("missing port {}", port),
        }
    })
}

fn require_parameter(
    parameters: &ParameterMap,
    param: &str,
    instance: &str,
) -> Result<f64, ElaborationError> {
    parameters.get(param).and_then(|v| v.as_f64()).ok_or_else(|| {
        ElaborationError::MissingParameter {
            instance: instance.to_string(),
            parameter: param.to_string(),
        }
    })
}

fn require_string_parameter(
    parameters: &ParameterMap,
    param: &str,
    instance: &str,
) -> Result<String, ElaborationError> {
    parameters.get(param).and_then(|v| v.as_str()).map(|s| s.to_string()).ok_or_else(|| {
        ElaborationError::MissingParameter {
            instance: instance.to_string(),
            parameter: param.to_string(),
        }
    })
}

fn get_parameter_or(parameters: &ParameterMap, param: &str, default: f64) -> f64 {
    parameters.get(param).and_then(|v| v.as_f64()).unwrap_or(default)
}

fn get_string_parameter_or(parameters: &ParameterMap, param: &str, default: &str) -> String {
    parameters.get(param).and_then(|v| v.as_str()).unwrap_or(default).to_string()
}
""")

    for c in COMPONENTS:
        struct_name = c["struct"]
        name = c["name"]
        prefix = c["prefix"]
        ports = c["ports"]
        params = c["params"]

        # HardwareDefinition
        out.append(f"// ── {struct_name} ─────────────────────────────────────────────────────────────\n")
        out.append(f"#[derive(Debug)]\n")
        out.append(f"pub struct {struct_name};\n")
        out.append(f"impl {struct_name} {{ pub fn new() -> Self {{ Self }} }}\n")
        out.append(f"impl HardwareDefinition for {struct_name} {{\n")
        out.append(f'    fn name(&self) -> &str {{ "{name}" }}\n')
        out.append(f'    fn ports(&self) -> &[PortDefinition] {{ &[] }}\n')
        out.append(f'    fn parameters(&self) -> &[ParameterDefinition] {{ &[] }}\n')
        out.append(f'    fn instantiate(\n')
        out.append(f'        &self,\n')
        out.append(f'        instance_name: &str,\n')
        out.append(f'        parameters: &ParameterMap,\n')
        if len(ports) == 0:
            out.append(f'        _connections: &ConnectionMap,\n')
        else:
            out.append(f'        connections: &ConnectionMap,\n')
        out.append(f'        _resolver: &dyn NetResolver,\n')
        out.append(f'    ) -> Result<Box<dyn HardwareInstance>, ElaborationError> {{\n')
        
        for p in ports:
            out.append(f'        let {p} = require_net(connections, "{sanitize_keyword(p)}", instance_name)?.to_string();\n')
        
        for p_name, p_type, default, _ in params:
            if default is None:
                if p_type == "string":
                    out.append(f'        let {p_name} = require_string_parameter(parameters, "{sanitize_keyword(p_name)}", instance_name)?;\n')
                else:
                    out.append(f'        let {p_name} = require_parameter(parameters, "{sanitize_keyword(p_name)}", instance_name)?;\n')
            else:
                if p_type == "string":
                    default_str = default.strip('"')
                    out.append(f'        let {p_name} = get_string_parameter_or(parameters, "{sanitize_keyword(p_name)}", "{default_str}");\n')
                elif p_type == "integer":
                    out.append(f'        let {p_name} = get_parameter_or(parameters, "{sanitize_keyword(p_name)}", {default}.0) as i64;\n')
                else:
                    out.append(f'        let {p_name} = get_parameter_or(parameters, "{sanitize_keyword(p_name)}", {default});\n')
        
        struct_fields = ", ".join(p for p in ports)
        if len(ports) > 0 and len(params) > 0:
            struct_fields += ", "
        struct_fields += ", ".join(p[0] for p in params)
        
        out.append(f'        Ok(Box::new({struct_name}Instance {{ name: instance_name.to_string(), {struct_fields} }}))\n')
        out.append(f'    }}\n')
        out.append(f'}}\n')

        # HardwareInstance
        out.append(f"#[derive(Debug)]\n")
        
        struct_def = "struct " + struct_name + "Instance { name: String, "
        fields = []
        for p in ports:
            fields.append(f"{p}: String")
        for p_name, p_type, _, _ in params:
            if p_type == "string":
                fields.append(f"{p_name}: String")
            elif p_type == "integer":
                fields.append(f"{p_name}: i64")
            else:
                fields.append(f"{p_name}: f64")
        struct_def += ", ".join(fields) + " }\n"
        out.append(struct_def)

        out.append(f"impl HardwareInstance for {struct_name}Instance {{\n")
        out.append(f'    fn instance_name(&self) -> &str {{ &self.name }}\n')
        out.append(f'    fn spice_lines(&self) -> Vec<String> {{\n')
        
        # Build formatting string
        if "custom_format" in c:
            fmt = c["custom_format"]
            # The custom format replaces the param definitions, but ports still precede it
            port_str = " ".join("{}" for _ in ports)
            args = ", ".join(f"self.{p}" for p in ports)
            
            # format the format string!
            # e.g. "PULSE({v0} {v1})" -> "PULSE({} {})", self.v0, self.v1
            param_args = []
            for p_name, _, _, _ in params:
                fmt = fmt.replace(f"{{{p_name}}}", "{}")
                param_args.append(f"self.{p_name}")
            
            spice_str = f"{{}} {port_str} {fmt}".strip()
            all_args = f"spice_name('{prefix}', &self.name)"
            if args: all_args += f", {args}"
            if param_args: all_args += f", {', '.join(param_args)}"
            
            out.append(f'        vec![format!("{spice_str}", {all_args})]\n')
        else:
            # Conditional formatting for default/empty parameters
            out.append('        let mut s = format!("{}')
            for _ in ports:
                out.append(' {}')
            out.append('", spice_name(\'' + prefix + '\', &self.name)')
            for p in ports:
                out.append(', self.' + p)
            out.append(');\n')
            
            for p_name, p_type, default, p_fmt in params:
                if not p_fmt:
                    continue # handled by custom_format or invisible
                if default is None:
                    # Required param
                    sub_fmt = p_fmt.replace(f"{{{p_name}}}", "{}")
                    out.append(f'        s.push_str(&format!(" {sub_fmt}", self.{p_name}));\n')
                else:
                    # Optional param
                    if p_type == "string":
                        default_val = '""'
                        cond = f'!self.{p_name}.is_empty()'
                    elif p_type == "integer":
                        cond = f'self.{p_name} != {default}'
                    else:
                        cond = f'self.{p_name} != {default}'
                    
                    sub_fmt = p_fmt.replace(f"{{{p_name}}}", "{}")
                    out.append(f'        if {cond} {{ s.push_str(&format!(" {sub_fmt}", self.{p_name})); }}\n')
            out.append('        vec![s]\n')

        out.append(f'    }}\n')
        out.append(f'}}\n\n')

    with open("crates/piperine-ngspice/src/hardware.rs", "w") as f:
        f.write("".join(out))

def gen_ngspice_ppr():
    out = []
    out.append("`ifndef NGSPICE_PPR\n`define NGSPICE_PPR\n\n")
    
    for c in COMPONENTS:
        name = c["name"]
        ports = c["ports"]
        params = c["params"]
        
        out.append(f"extern module {name}(\n")
        
        if ports:
            port_decls = ", ".join(f"inout {sanitize_keyword(p)}" for p in ports)
            out.append(f"    {port_decls};\n")
        else:
            out.append("    ;\n")
            
        for i, (p_name, p_type, default, _) in enumerate(params):
            if default is None:
                out.append(f"    parameter {p_type} {sanitize_keyword(p_name)}")
            else:
                out.append(f"    parameter {p_type} {sanitize_keyword(p_name)} = {default}")
            if i < len(params) - 1:
                out.append(",\n")
            else:
                out.append("\n")
                
        out.append(");\n\n")

    out.append("`endif // NGSPICE_PPR\n")
    
    with open("crates/piperine-ngspice/ppr/ngspice.ppr", "w") as f:
        f.write("".join(out))

if __name__ == "__main__":
    gen_hardware_rs()
    gen_ngspice_ppr()
