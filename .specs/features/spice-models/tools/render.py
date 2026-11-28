#!/usr/bin/env python3
"""Render the per-family model maps of the spice-models feature from the extracted inventory.

Usage: render.py inventory.json <out-dir>
Every table of `models/*.md` is generated; the hand-written decisions live in design.md."""
import json, os, re, sys

inv = json.load(open(sys.argv[1]))
out = sys.argv[2]
os.makedirs(out, exist_ok=True)

KEYWORDS = set("as break const continue crate else enum extern false fn for if impl in let loop match mod pub return self Self struct super trait true use where while module nature signal on out flip after abstract async await become box do dyn final gen macro move mut override priv ref static try type typeof unsafe unsized virtual yield".split())
LIT = re.compile(r"^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$")

# device directory -> (MSL module, namespace, ports, .model keyword / level, one-line note)
DEV = {
 "res": ("Resistor, SemiconductorResistor", "passive", "p, n", "element R; `.model … r`", "one ngspice device, two MSL modules: by value and by geometry"),
 "cap": ("Capacitor, SemiconductorCapacitor", "passive", "p, n", "element C; `.model … c`", "one ngspice device, two MSL modules: by value and by geometry"),
 "ind": ("Inductor, GeometricInductor, MutualInductor", "passive", "p, n (+ pub node `link`)", "elements L and K; `.model … l`", "the directory also holds the mutual inductor `K` (table `MUTpTable`)"),
 "vsrc": ("VoltageSource", "source", "p, n, waveform: Option<f64>", "element V", "the time-domain functions are separate modules with a result"),
 "isrc": ("CurrentSource", "source", "p, n, waveform: Option<f64>", "element I", "the time-domain functions are separate modules with a result"),
 "asrc": ("BehavioralVoltageSource, BehavioralCurrentSource", "source", "p, n, value: f64", "element B", "the expression is a value input of the module"),
 "vcvs": ("VoltageControlledVoltageSource", "controlled", "p, n, cp, cn", "element E", ""),
 "vccs": ("VoltageControlledCurrentSource", "controlled", "p, n, cp, cn", "element G", ""),
 "cccs": ("CurrentControlledCurrentSource", "controlled", "p, n, sp, sn", "element F", "the sensing branch (sp, sn) replaces ngspice's named 0 V source"),
 "ccvs": ("CurrentControlledVoltageSource", "controlled", "p, n, sp, sn", "element H", "the sensing branch (sp, sn) replaces ngspice's named 0 V source"),
 "sw": ("VoltageControlledSwitch", "switch", "p, n, cp, cn", "element S; `.model … sw`", ""),
 "csw": ("CurrentControlledSwitch", "switch", "p, n, sp, sn", "element W; `.model … csw`", "the sensing branch (sp, sn) replaces ngspice's named 0 V source"),
 "tra": ("LosslessTransmissionLine", "line", "p1, n1, p2, n2", "element T", ""),
 "ltra": ("LossyTransmissionLine", "line", "p1, n1, p2, n2", "element O; `.model … ltra`", ""),
 "txl": ("SingleLossyTransmissionLine", "line", "p1, p2 (referred to ground, as in ngspice)", "element Y; `.model … txl`", ""),
 "cpl": ("CoupledTransmissionLines<const N: usize>", "line", "near: [Electrical; N], far: [Electrical; N] (referred to ground, as in ngspice)", "element P; `.model … cpl`", ""),
 "urc": ("UniformRcLine", "line", "p1, p2, reference", "element U; `.model … urc`", "expands into lumps at elaboration, as ngspice does"),
 "dio": ("Diode, GeometricDiode (+ `…SelfHeating` variants)", "diode", "a, k (+ tj)", "element D; `.model … d`, level 1 and level 3", ""),
 "bjt": ("Bjt", "bipolar", "c, b, e, s", "element Q; `.model … npn|pnp`, level 1 (and 0, 2)", "Gummel-Poon"),
 "vbic": ("Vbic (+ VbicSelfHeating)", "bipolar", "c, b, e, s (+ dt)", "element Q; `.model … npn|pnp`, level 4 and 9", ""),
 "hicum2": ("Hicum2 (+ Hicum2SelfHeating)", "bipolar", "c, b, e, s (+ tj)", "element Q; `.model … npn|pnp`, level 8", ""),
 "jfet": ("Jfet", "jfet", "d, g, s", "element J; `.model … njf|pjf`, level 1", ""),
 "jfet2": ("Jfet2", "jfet", "d, g, s", "element J; `.model … njf|pjf`, level 2", "Parker-Skellern"),
 "mes": ("Mesfet", "mesfet", "d, g, s", "element Z; `.model … nmf|pmf`, level 1", "Statz"),
 "mesa": ("Mesa", "mesfet", "d, g, s", "element Z; `.model … nmf|pmf`, levels 2, 3, 4", ""),
 "hfet1": ("Hfet1", "mesfet", "d, g, s", "element Z; `.model … nhfet|phfet`, level 5", ""),
 "hfet2": ("Hfet2", "mesfet", "d, g, s", "element Z; `.model … nhfet|phfet`, level 6", ""),
 "mos1": ("Mos1", "mosfet", "d, g, s, b", "element M; `.model … nmos|pmos`, level 1", "Shichman-Hodges"),
 "mos2": ("Mos2", "mosfet", "d, g, s, b", "element M; level 2", "Grove-Frohman"),
 "mos3": ("Mos3", "mosfet", "d, g, s, b", "element M; level 3", "semi-empirical"),
 "mos6": ("Mos6", "mosfet", "d, g, s, b", "element M; level 6", "Sakurai-Newton"),
 "mos9": ("Mos9", "mosfet", "d, g, s, b", "element M; level 9", "modified level 3"),
 "vdmos": ("Vdmos (+ VdmosSelfHeating)", "mosfet", "d, g, s (+ tj, tcase)", "element M; `.model … vdmos|vdmosn|vdmosp`", "power MOSFET"),
 "bsim1": ("Bsim1", "bsim", "d, g, s, b", "element M; level 4", ""),
 "bsim2": ("Bsim2", "bsim", "d, g, s, b", "element M; level 5", ""),
 "bsim3": ("Bsim3", "bsim", "d, g, s, b", "element M; level 8 and 49, `version` 3.3.0", ""),
 "bsim3v0": ("Bsim3V0", "bsim", "d, g, s, b", "element M; level 8 and 49, `version` 3.0", ""),
 "bsim3v1": ("Bsim3V1", "bsim", "d, g, s, b", "element M; level 8 and 49, `version` 3.1", ""),
 "bsim3v32": ("Bsim3V32", "bsim", "d, g, s, b", "element M; level 8 and 49, `version` 3.2 to 3.2.4", ""),
 "bsim4": ("Bsim4", "bsim", "d, g, s, b", "element M; level 14 and 54, `version` 4.8", ""),
 "bsim4v5": ("Bsim4V5", "bsim", "d, g, s, b", "element M; level 14 and 54, `version` 4.5", ""),
 "bsim4v6": ("Bsim4V6", "bsim", "d, g, s, b", "element M; level 14 and 54, `version` 4.6", ""),
 "bsim4v7": ("Bsim4V7", "bsim", "d, g, s, b", "element M; level 14 and 54, `version` 4.7", ""),
 "bsim3soi_fd": ("Bsim3SoiFd", "soi", "d, g, s, e (+ p, b, t variants)", "element M; `.model … nmos|pmos`, level 55", "fully depleted"),
 "bsim3soi_dd": ("Bsim3SoiDd", "soi", "d, g, s, e (+ p, b, t variants)", "element M; level 56", "dynamically depleted"),
 "bsim3soi_pd": ("Bsim3SoiPd", "soi", "d, g, s, e (+ p, b, t variants)", "element M; level 57", "partially depleted"),
 "bsimsoi": ("Bsim4Soi", "soi", "d, g, s, e (+ p, b, t variants)", "element M; level 10 and 58", "BSIMSOI 4.x"),
 "soi3": ("Soi3", "soi", "d, gf, s, gb, b (+ thermal)", "element M; level 60", "STAG"),
 "hisim2": ("Hisim2", "hisim", "d, g, s, b", "element M; level 68", ""),
 "hisimhv1": ("HisimHv1", "hisim", "d, g, s, b, sub (+ temp)", "element M; level 73, `version` 1.x", ""),
 "hisimhv2": ("HisimHv2", "hisim", "d, g, s, b, sub (+ temp)", "element M; level 73, `version` 2.x", ""),
 "numd": ("—", "excluded", "", "`.model … numd` level 1", "CIDER one-dimensional numerical diode"),
 "numd2": ("—", "excluded", "", "`.model … numd` level 2", "CIDER two-dimensional numerical diode"),
 "nbjt": ("—", "excluded", "", "`.model … nbjt` level 1", "CIDER one-dimensional numerical BJT"),
 "nbjt2": ("—", "excluded", "", "`.model … nbjt` level 2", "CIDER two-dimensional numerical BJT"),
 "numos": ("—", "excluded", "", "`.model … numos`", "CIDER two-dimensional numerical MOSFET"),
 "ndev": ("—", "excluded", "", "`.model … ndev`", "socket to an external device simulator"),
}
FAMILIES = [
 ("passive", "Passive elements", ["res", "cap", "ind"]),
 ("source", "Independent, behavioral and controlled sources", ["vsrc", "isrc", "asrc", "vcvs", "vccs", "cccs", "ccvs"]),
 ("switch", "Switches", ["sw", "csw"]),
 ("line", "Transmission lines", ["tra", "ltra", "txl", "cpl", "urc"]),
 ("diode", "Diodes", ["dio"]),
 ("bipolar", "Bipolar transistors", ["bjt", "vbic", "hicum2"]),
 ("jfet", "Junction field-effect transistors", ["jfet", "jfet2"]),
 ("mesfet", "MESFETs and HFETs", ["mes", "mesa", "hfet1", "hfet2"]),
 ("mosfet", "MOSFETs, levels 1 to 9, and the power MOSFET", ["mos1", "mos2", "mos3", "mos6", "mos9", "vdmos"]),
 ("bsim", "BSIM bulk MOSFETs", ["bsim1", "bsim2", "bsim3", "bsim3v32", "bsim3v1", "bsim3v0", "bsim4", "bsim4v7", "bsim4v6", "bsim4v5"]),
 ("soi", "Silicon-on-insulator MOSFETs", ["bsim3soi_fd", "bsim3soi_dd", "bsim3soi_pd", "bsimsoi", "soi3"]),
 ("hisim", "HiSIM MOSFETs", ["hisim2", "hisimhv1", "hisimhv2"]),
 ("excluded", "Devices not written in MSL by this feature", ["numd", "numd2", "nbjt", "nbjt2", "numos", "ndev"]),
]
DIFF_BASE = {"bsim3v32": "bsim3", "bsim3v1": "bsim3", "bsim3v0": "bsim3", "bsim4v7": "bsim4", "bsim4v6": "bsim4", "bsim4v5": "bsim4"}
SLOT_TEXT = [
 ("DEVload", "DC, operating point, transient"), ("DEVacLoad", "AC"), ("DEVpzLoad", "pole-zero"),
 ("DEVnoise", "noise"), ("DEVtemperature", "temperature update"), ("DEVtrunc", "truncation-error step control"),
 ("DEVconvTest", "own convergence test"), ("DEVsetic", "initial conditions (`ic`)"), ("DEVaccept", "accept hook (breakpoints, history)"),
 ("DEVsenLoad", "sensitivity"), ("DEVdisto", "Volterra distortion (`.disto`)"), ("DEVsoaCheck", "safe-operating-area check"),
]
TYPE_FLAG = {"npn", "pnp", "nmos", "pmos", "njf", "pjf", "nmf", "pmf", "nhfet", "phfet", "d", "r", "c", "l", "sw", "csw", "ltra", "txl", "cpl", "urc", "vdmosn", "vdmosp", "vdmos", "nsoi", "psoi", "numd", "nbjt", "numos", "ndev"}

def msl_name(name):
    n = name.lower().replace("-", "_").replace(".", "_").replace("/", "_per_")
    if n in KEYWORDS:
        n += "_"
    return n

def esc(text):
    return text.replace("|", "\\|").replace("\n", " ")

def default_cell(row):
    d = row.get("default", "")
    if not d:
        return "—"
    d = d.replace("ckt->CKTnomTemp", "<nominal temperature of the run>").replace("ckt->CKTtemp", "<temperature of the run>")
    d = re.sub(r"\b(?:model|here|mods|inst)\s*->\s*", "", d)
    return "`" + esc(d) + "`"

def kind_cell(row):
    """How the extractor classifies the parameter's given-ness."""
    if row["type"] == "flag":
        return "flag"
    d = row.get("default", "").replace(" ", "")
    reads = row.get("given_reads", 0)
    if d and LIT.match(d) and reads <= 1:
        return "default"
    if reads > 1:
        return "given-ness read"
    if d:
        return "computed default"
    return "unresolved"

def split_rows(rows):
    ins, alias, outs, skip, flags = [], [], [], [], []
    for r in rows:
        n = r["name"]
        if n.startswith("sens_"):
            skip.append(r)
        elif r["dir"] in ("in", "inout"):
            if r["redundant"]:
                alias.append(r)
            elif r["type"] == "flag" and n in TYPE_FLAG:
                flags.append(r)
            else:
                ins.append(r)
        elif r["dir"] == "out":
            (alias if r["redundant"] else outs).append(r)
        else:
            skip.append(r)
    return ins, alias, outs, skip, flags

def param_table(rows):
    lines = ["| MSL name | ngspice name | Type | Default in ngspice | Given-ness | Meaning |", "| --- | --- | --- | --- | --- | --- |"]
    for r in rows:
        lines.append(f"| `{msl_name(r['name'])}` | `{r['name']}` | {r['type']} | {default_cell(r)} | {kind_cell(r)} | {esc(r['desc'])} |")
    return lines

def device_section(dev, level="##"):
    v = inv[dev]
    module, ns, ports, card, note = DEV[dev]
    meta = v["meta"]
    L = []
    L.append(f"{level} `{dev}`: {meta.get('description') or meta.get('name')}")
    L.append("")
    L.append("| | |")
    L.append("| --- | --- |")
    L.append(f"| ngspice device | `{meta.get('name')}`, `src/spicelib/devices/{dev}/` ({v['loc']} lines of C) |")
    L.append(f"| Netlist form | {card} |")
    L.append(f"| MSL module | `{module}`" + (f" in `spice::{ns}`" if ns != "excluded" else "") + " |")
    if ports:
        L.append(f"| MSL ports | `{ports}` |")
    L.append(f"| ngspice terminals | {', '.join('`'+t+'`' for t in v['terminals']) or '—'} |")
    if note:
        L.append(f"| Note | {note} |")
    slots = [text for slot, text in SLOT_TEXT if meta.get(slot)]
    L.append(f"| ngspice entry points | {'; '.join(slots) or '—'} |")
    L.append(f"| Newton limiting | {', '.join('`'+x+'`' for x in v['limiters']) or 'none'} |")
    L.append(f"| Internal unknowns | {', '.join(v['internal']) or 'none'} |")
    noise = [n.strip('_.') for n in v['noise'] if n.strip('_.')]
    L.append(f"| Noise sources | {', '.join('`'+n+'`' for n in noise) or ('none' if not meta.get('DEVnoise') else 'see the noise file')} |")
    L.append(f"| State vector length | {v['states'] if v['states'] is not None else '—'} |")
    L.append("")
    base = DIFF_BASE.get(dev)
    for title, key in (("Instance parameters", "instance"), ("Model parameters", "model")):
        ins, alias, outs, skip, flags = split_rows(v[key])
        if not (ins or alias or outs or flags):
            continue
        if base:
            bins = split_rows(inv[base][key])[0]
            bnames = {r["name"] for r in bins}
            names = {r["name"] for r in ins}
            only = [r for r in ins if r["name"] not in bnames]
            missing = sorted(bnames - names)
            bdef = {r["name"]: r.get("default", "") for r in bins}
            changed = [r for r in ins if r["name"] in bnames and r.get("default", "") != bdef[r["name"]]]
            L.append(f"**{title}: {len(ins)} settable.** Listed as a difference against `{base}` ({len(bins)} settable), whose table holds the shared rows.")
            L.append("")
            if only:
                L.append(f"Parameters `{dev}` has and `{base}` does not ({len(only)}):")
                L.append("")
                L += param_table(only)
                L.append("")
            if missing:
                L.append(f"Parameters of `{base}` that `{dev}` does not have ({len(missing)}): " + ", ".join(f"`{m}`" for m in missing) + ".")
                L.append("")
            if changed:
                L.append(f"Shared parameters whose extracted default differs ({len(changed)}):")
                L.append("")
                L += param_table(changed)
                L.append("")
        else:
            L.append(f"**{title}: {len(ins)} settable.**")
            L.append("")
            if ins:
                L += param_table(ins)
                L.append("")
        if flags:
            L.append("Type keywords of the model card (they become the polarity param, or select the module): " + ", ".join(f"`{r['name']}`" for r in flags) + ".")
            L.append("")
        if alias:
            L.append("Aliases ngspice accepts (not MSL names; listed in the Book's translation table): " + ", ".join(f"`{r['name']}`" for r in alias) + ".")
            L.append("")
        if outs and not base:
            L.append(f"Read-only quantities ({len(outs)}; each becomes an operating-point variable of the same name unless it is a node number): " + ", ".join(f"`{r['name']}`" for r in outs) + ".")
            L.append("")
    for tname, rows in v["other_tables"].items():
        if not rows:
            continue
        ins, alias, outs, skip, flags = split_rows(rows)
        L.append(f"**Table `{tname}`: {len(ins)} settable.**")
        L.append("")
        L += param_table(ins)
        L.append("")
    return L

totals = {}
for slug, title, devs in FAMILIES:
    L = [f"# Model map: {title}", "",
         "Generated by `tools/render.py` from the ngspice-47 sources (`tools/extract.py`). Do not edit by hand: change the tool and render again. The decisions on names, modules and ports are in `../design.md`.", "",
         "**How to read a row.** *MSL name* is the param of the MSL module: the ngspice name in lower case, with `_` appended where the name is an MSL keyword. *Default in ngspice* is the expression ngspice assigns when the parameter is not given, as the extractor found it (`—`: not assigned in the `if (!given)` idiom; the family's mapping task resolves it by hand). *Given-ness*: `default` means a constant default and a given flag nothing else reads, so the MSL param is `f64 = <default>`; `given-ness read` means ngspice changes behaviour on whether the parameter was written, so the MSL param is an `Option`; `computed default` means the default depends on other parameters, so the MSL param is an `Option` resolved in a body `let`; `unresolved` is decided by the mapping task. Units are not in the ngspice tables: they are added per family by the mapping task from the model's reference manual (spec SM-12).", ""]
    for dev in devs:
        L += device_section(dev)
        v = inv[dev]
        ii = len(split_rows(v["instance"])[0]); mm = len(split_rows(v["model"])[0]) + sum(len(split_rows(t)[0]) for t in v["other_tables"].values())
        totals[dev] = (ii, mm, len(split_rows(v["instance"])[2]))
    open(os.path.join(out, slug + ".md"), "w").write("\n".join(L).rstrip() + "\n")

# summary table for spec.md
rows = ["| ngspice device | Directory | Netlist form | MSL module | Instance params | Model params | C lines |", "| --- | --- | --- | --- | --- | --- | --- |"]
ti = tm = 0
for slug, title, devs in FAMILIES:
    for dev in devs:
        v = inv[dev]; i, m, o = totals[dev]
        if slug != "excluded":
            ti += i; tm += m
        rows.append(f"| {v['meta'].get('name')} | `{dev}` | {DEV[dev][3]} | `{DEV[dev][0]}` | {i} | {m} | {v['loc']} |")
open(os.path.join(out, "_summary.md"), "w").write("\n".join(rows) + f"\n\nIn scope: instance {ti}, model {tm}, total {ti+tm}\n")
print("in scope", ti, tm, ti + tm)
