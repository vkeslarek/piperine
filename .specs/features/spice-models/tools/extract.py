#!/usr/bin/env python3
"""Extract the device inventory of an ngspice source tree: per device directory the terminals,
the instance and model IFparm tables, the SPICEdev capability slots, and (heuristically) the
defaults assigned when a parameter is not given. Output: one JSON file."""
import json, os, re, sys

root = sys.argv[1]
out = sys.argv[2]
devroot = os.path.join(root, "src/spicelib/devices")

FLAGS = {"IF_REAL": "real", "IF_INTEGER": "integer", "IF_FLAG": "flag", "IF_STRING": "string",
         "IF_COMPLEX": "complex", "IF_REALVEC": "real vector", "IF_INSTANCE": "instance",
         "IF_NODE": "node", "IF_PARSETREE": "expression", "IF_INTVEC": "integer vector",
         "IF_STRINGVEC": "string vector", "IF_VECTOR": "vector", "IF_FLAGVEC": "flag vector",
         "IF_INSTVEC": "instance vector", "IF_CPLXVEC": "complex vector", "IF_REAL ": "real"}

def strip_comments(text):
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    text = re.sub(r"//[^\n]*", " ", text)
    return text

ENTRY = re.compile(r'\b(I?O?P[A-Z]*|P)\s*\(\s*"([^"]*)"\s*,\s*([A-Za-z0-9_]+)\s*,\s*([A-Za-z_| ]+?)\s*,\s*((?:"(?:[^"\\]|\\.)*"\s*)+|NULL)\s*\)', re.S)

def parse_table(body):
    rows = []
    for m in ENTRY.finditer(body):
        macro, name, ident, typ, desc = m.groups()
        if desc.strip() == "NULL":
            desc = ""
        else:
            desc = "".join(re.findall(r'"((?:[^"\\]|\\.)*)"', desc))
        typ = typ.strip().split("|")[0].strip()
        direction = ("in" if "I" in macro.split("P")[0] or macro in ("P",) else "") 
        head = macro.split("P")[0]
        if macro == "P":
            direction = "none"
        else:
            direction = {"IO": "inout", "I": "in", "O": "out"}.get(head, head)
        tail = macro[len(head) + 1:]
        rows.append({"name": name, "id": ident, "type": FLAGS.get(typ, typ), "dir": direction,
                     "redundant": "R" in tail, "macro": macro, "desc": " ".join(desc.split())})
    return rows

def tables(text):
    res = {}
    for m in re.finditer(r"IFparm\s+(\w+)\s*\[\s*\]\s*=\s*\{", text):
        start = m.end()
        end = text.find("};", start)
        res[m.group(1)] = parse_table(text[start:end])
    return res

def names(text):
    m = re.search(r"char\s*\*\s*(\w+names)\s*\[\s*\]\s*=\s*\{(.*?)\}", text, re.S)
    if not m:
        return []
    return re.findall(r'"([^"]*)"', m.group(2))

SLOTS = ["DEVload", "DEVtemperature", "DEVtrunc", "DEVacLoad", "DEVaccept", "DEVsetic", "DEVpzLoad",
         "DEVconvTest", "DEVsenSetup", "DEVsenLoad", "DEVsenAcLoad", "DEVdisto", "DEVnoise", "DEVsoaCheck",
         "DEVfindBranch"]

def info(text):
    d = {}
    m = re.search(r'\.name\s*=\s*"([^"]*)"', text)
    d["name"] = m.group(1) if m else ""
    m = re.search(r'\.description\s*=\s*"([^"]*)"', text)
    d["description"] = m.group(1) if m else ""
    for s in SLOTS:
        m = re.search(r"\." + s + r"\s*=\s*([A-Za-z0-9_]+)", text)
        d[s] = bool(m and m.group(1) != "NULL")
    return d

def id_to_field(text, who):
    """case ID: ... who->FIELD = ...  (first assignment after the case label)."""
    res = {}
    for m in re.finditer(r"case\s+([A-Za-z0-9_]+)\s*:(.*?)(?=\bcase\s+[A-Za-z0-9_]+\s*:|\bdefault\s*:)", text, re.S):
        ident, body = m.groups()
        f = re.search(r"\b(?:%s)\s*->\s*((?![A-Za-z0-9_]*Given\b)[A-Za-z0-9_]+)\s*=\s*([^;]+);" % who, body)
        if f:
            res.setdefault(ident, (f.group(1), " ".join(f.group(2).split())))
    return res

def id_to_given(text, who):
    res = {}
    for m in re.finditer(r"case\s+([A-Za-z0-9_]+)\s*:(.*?)(?=\bcase\s+[A-Za-z0-9_]+\s*:|\bdefault\s*:)", text, re.S):
        ident, body = m.groups()
        f = re.search(r"\b(?:%s)\s*->\s*([A-Za-z0-9_]*Given)\s*=" % who, body)
        if f:
            res.setdefault(ident, f.group(1))
    return res

def defaults(text, who):
    """if (!who->FIELDGiven) { who->FIELD = EXPR; ... }  or single statement form."""
    res = {}
    for m in re.finditer(r"if\s*\(\s*!\s*\(?\s*(?:%s)\s*->\s*([A-Za-z0-9_]+)Given\s*\)?\s*\)\s*\{?\s*(?:%s)\s*->\s*((?![A-Za-z0-9_]*Given\b)[A-Za-z0-9_]+)\s*=\s*([^;]+);" % (who, who), text, re.S):
        res.setdefault(m.group(2), " ".join(m.group(3).split()))
    return res

inventory = {}
for dev in sorted(os.listdir(devroot)):
    d = os.path.join(devroot, dev)
    if not os.path.isdir(d):
        continue
    files = {}
    for dirpath, _, fns in os.walk(d):
        for fn in fns:
            if fn.endswith((".c", ".h", ".cpp", ".hpp")):
                with open(os.path.join(dirpath, fn), errors="replace") as fh:
                    files[fn] = strip_comments(fh.read())
    alltext = "\n".join(files.values())
    tabs = {}
    for fn, text in files.items():
        tabs.update(tables(text))
    extra = {k: v for k, v in tabs.items()}
    inst = [(k, v) for k, v in sorted(tabs.items()) if re.search(r"pTable$", k) and not re.search(r"mPTable$", k)]
    mod = [(k, v) for k, v in sorted(tabs.items()) if re.search(r"mPTable$", k)]
    table_names = sorted(tabs)
    # the device's own tables are the ones its SPICEdev names
    initm = "\n".join(t for fn, t in files.items() if "SPICEdev" in t and ".DEVpublic" in t)
    mi = re.search(r"\.instanceParms\s*=\s*(\w+)", initm); mm = re.search(r"\.modelParms\s*=\s*(\w+)", initm)
    others = {k: v for k, v in tabs.items() if not (mi and k == mi.group(1)) and not (mm and k == mm.group(1))}
    inst = tabs.get(mi.group(1), []) if mi else (inst[0][1] if inst else [])
    mod = tabs.get(mm.group(1), []) if mm and mm.group(1) != "NULL" else []
    init = "\n".join(t for fn, t in files.items() if "SPICEdev" in t and ".DEVpublic" in t)
    meta = info(init) if init else {}
    par_text = "\n".join(t for fn, t in files.items() if re.search(r"(par|mpar|param|mparam)\.c$", fn))
    imap = id_to_field(par_text, "here|inst|instance|fast")
    mmap = id_to_field(par_text, "model|mod|mods|modfast")
    igiv = id_to_given(par_text, "here|inst|instance|fast")
    mgiv = id_to_given(par_text, "model|mod|mods|modfast")
    nonpar = "\n".join(t for fn, t in files.items() if not re.search(r"(par|mpar|param|mparam|ask|mask)\.c$", fn))
    def given_reads(flag):
        return len(re.findall(r"->\s*" + re.escape(flag) + r"\b", nonpar)) if flag else 0
    idef = defaults(alltext, "here|inst|instance|fast")
    mdef = defaults(alltext, "model|mod|mods")
    for row in inst:
        f = imap.get(row["id"])
        row["field"] = f[0] if f else ""
        row["store"] = f[1] if f else ""
        row["default"] = idef.get(row["field"], "") if f else ""
        row["given"] = igiv.get(row["id"], ""); row["given_reads"] = given_reads(row["given"])
    for tab in others.values():
        for row in tab:
            f = imap.get(row["id"]) or mmap.get(row["id"])
            row["field"] = f[0] if f else ""
            row["store"] = f[1] if f else ""
            row["default"] = (idef.get(row["field"], "") or mdef.get(row["field"], "")) if f else ""
    for row in mod:
        f = mmap.get(row["id"])
        row["field"] = f[0] if f else ""
        row["store"] = f[1] if f else ""
        row["default"] = mdef.get(row["field"], "") if f else ""
        row["given"] = mgiv.get(row["id"], ""); row["given_reads"] = given_reads(row["given"])
    cfiles = {fn: t for fn, t in files.items() if fn.endswith((".c", ".cpp"))}
    ctext = "\n".join(cfiles.values())
    limiters = sorted(set(re.findall(r"\b(DEV(?:pnjlim|fetlim|limvds|limitlog|soipnjlim|cmeyer|qmeyer|pred))\b", ctext)))
    noise_names = []
    for fn, t in cfiles.items():
        if re.search(r"noi", fn):
            for m in re.finditer(r"char\s*\*\s*\w+\s*\[[^\]]*\]\s*=\s*\{(.*?)\}", t, re.S):
                noise_names += [n for n in re.findall(r'"([^"]*)"', m.group(1))]
            noise_names += re.findall(r'(?:onoise|inoise)[_a-z]*%s(\.?[A-Za-z0-9_.]+)"', t)
    seen = []
    for n in noise_names:
        if n not in seen:
            seen.append(n)
    internal = []
    for m in re.finditer(r"CKTmk(Volt|Cur)\s*\([^;]*?\"([^\"]*)\"\s*\)", ctext, re.S):
        e = ("node " if m.group(1) == "Volt" else "branch ") + m.group(2)
        if e not in internal:
            internal.append(e)
    st = re.search(r"#\s*define\s+\w*(?:numStates|NUMSTATES|NumStates|numstates)\w*\s+\(?\s*(\d+)", "\n".join(files.values()))
    states = int(st.group(1)) if st else None
    loc = sum(t.count("\n") for fn, t in files.items() if fn.endswith((".c", ".cpp")))
    inventory[dev] = {"meta": meta, "terminals": names(alltext), "instance": inst, "model": mod,
                      "files": sorted(files), "loc": loc, "other_tables": others,
                      "limiters": limiters, "noise": seen, "internal": internal, "states": states}
with open(out, "w") as fh:
    json.dump(inventory, fh, indent=1)
for dev, v in inventory.items():
    i_in = [r for r in v["instance"] if r["dir"] in ("in", "inout") and not r["redundant"]]
    m_in = [r for r in v["model"] if r["dir"] in ("in", "inout") and not r["redundant"]]
    i_out = [r for r in v["instance"] if r["dir"] == "out" and not r["redundant"]]
    md = sum(1 for r in m_in if r["default"]); idf = sum(1 for r in i_in if r["default"])
    if v["other_tables"]: print("   other tables:", {k: len(t) for k, t in v["other_tables"].items()})
    print(f"{dev:12} {v['meta'].get('name',''):12} term={len(v['terminals'])} inst_in={len(i_in)}({idf}) inst_out={len(i_out)} model_in={len(m_in)}({md}) loc={v['loc']}")
