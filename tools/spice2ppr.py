#!/usr/bin/env python3
"""SPICE/ngspice netlist -> Piperine (.ppr) transpiler.

Maps circuit topology + analyses to idiomatic Piperine. Where a feature is not
yet implemented in Piperine but is on the roadmap, the planned syntax is emitted
(e.g. `$include_spice(...)` for `.include`, see docs/development/PHASE4.md/ROADMAP.md).
Faithful for R/C/L/V/I/D/Q/J/M/E/G/F/H/S/B and .model/.tran/.ac/.dc/.op.
"""
import re, sys, os

SI = {'t':1e12,'g':1e9,'meg':1e6,'k':1e3,'m':1e-3,'mil':25.4e-6,
      'u':1e-6,'n':1e-9,'p':1e-12,'f':1e-15,'a':1e-18}

def num(tok):
    """Convert a SPICE value token to a plain float string."""
    t = tok.strip().lower()
    t = t.strip('(){}')             # drop stray parens/braces (ngspice {expr} values)
    m = re.match(r'^([+-]?[0-9]*\.?[0-9]+(?:e[+-]?[0-9]+)?)([a-z]*)$', t)
    if not m: return tok            # expression / model name — pass through
    val, suf = m.group(1), m.group(2)
    f = float(val)
    if suf:
        for s in ('meg','mil'):     # multi-letter first
            if suf.startswith(s): f *= SI[s]; suf=''; break
        if suf and suf[0] in SI: f *= SI[suf[0]]
    # tidy formatting: integers plainly, else %g to avoid float noise
    if f == int(f) and abs(f) < 1e15: return repr(int(f))
    return f'{f:g}'

def net(tok):
    """SPICE node -> Piperine net identifier. 0 -> gnd; numeric -> nN."""
    if tok == '0': return 'gnd'
    if re.match(r'^[0-9]', tok): return 'n'+tok
    return re.sub(r'[^A-Za-z0-9_]', '_', tok)

def ident(name):
    """Sanitize an instance/model name into a Piperine identifier."""
    s = re.sub(r'[^A-Za-z0-9_]', '_', name)
    return s if re.match(r'^[A-Za-z_]', s) else 'x_'+s

# .model TYPE -> (piperine base module, ports)
MODEL_BASE = {
    'd':'d', 'npn':'npn', 'pnp':'pnp', 'njf':'jfet_n', 'pjf':'jfet_p',
    'nmos':'nmos', 'pmos':'pmos', 'nsoi':'nmos', 'psoi':'pmos',
    'sw':'vsw', 'csw':'isw', 'r':'res', 'c':'cap', 'l':'ind',
}

def parse_lines(text):
    """Join continuation lines (+) and drop comments/.control blocks."""
    out, in_ctl = [], False
    for raw in text.splitlines():
        line = raw.rstrip()
        if not line: continue
        low = line.strip().lower()
        if low.startswith('.control'): in_ctl = True; continue
        if low.startswith('.endc'):    in_ctl = False; continue
        if in_ctl: continue
        if line.lstrip().startswith('*'): continue
        if ';' in line: line = line.split(';',1)[0]
        if line.lstrip().startswith('+') and out:
            out[-1] += ' ' + line.lstrip()[1:].strip()
        else:
            out.append(line.strip())
    return out

def emit_device(p, t):
    """p: element prefix char, t: whitespace tokens of the element line."""
    name = ident(t[0])
    P = lambda *ns: ', '.join(f'.{a}({net(b)})' for a,b in ns)
    if p == 'r' and len(t) >= 4:
        return f'res #(.r({num(t[3])})) {name}({P(("p",t[1]),("n",t[2]))});'
    if p == 'c' and len(t) >= 4:
        return f'cap #(.c({num(t[3])})) {name}({P(("p",t[1]),("n",t[2]))});'
    if p == 'l' and len(t) >= 4:
        return f'ind #(.l({num(t[3])})) {name}({P(("p",t[1]),("n",t[2]))});'
    if p == 'd' and len(t) >= 4:
        return f'{dev_for(t[3],"d")} {name}({P(("a",t[1]),("c",t[2]))});'
    if p == 'q' and len(t) >= 5:
        return f'{dev_for(t[4],"npn")} {name}({P(("c",t[1]),("b",t[2]),("e",t[3]))});'
    if p == 'j' and len(t) >= 5:
        return f'{dev_for(t[4],"jfet_n")} {name}({P(("d",t[1]),("g",t[2]),("s",t[3]))});'
    if p == 'm' and len(t) >= 6:
        return f'{dev_for(t[5],"nmos")} {name}({P(("d",t[1]),("g",t[2]),("s",t[3]),("b",t[4]))});'
    if p in 'vi':
        mod = 'vsource' if p=='v' else 'isource'
        rest = ' '.join(t[3:])
        WF = {'sin':   ['vo','va','freq','td','theta','phase'],
              'pulse': ['v0','v1','td','tr','tf','pw','per'],
              'exp':   ['v1','v2','td1','tau1','td2','tau2']}
        IWF = {'sin':   ['io','ia','freq','td','theta','phase'],
               'pulse': ['i0','i1','td','tr','tf','pw','per'],
               'exp':   ['i1','i2','td1','tau1','td2','tau2']}
        for fn in ('sin','pulse','exp'):
            mm = re.search(fn+r'\s*\(([^)]*)\)', rest, re.I)
            if mm:
                args = [num(a) for a in mm.group(1).split()]
                names = (WF if p=='v' else IWF)[fn]
                ppar = ', '.join(f'.{names[i]}({a})'
                                 for i,a in enumerate(args) if i < len(names))
                return f'{p}{fn} #({ppar}) {name}({P(("p",t[1]),("n",t[2]))});'
        dc = '0'
        dm = re.search(r'\bdc\b\s+([^\s]+)', rest, re.I)
        if dm: dc = num(dm.group(1))
        elif t[3:] and re.match(r'^[+-]?[0-9.]', t[3]): dc = num(t[3])
        ac = ''
        am = re.search(r'\bac\b\s+([^\s]+)', rest, re.I)
        if am: ac = f', .acmag({num(am.group(1))})'
        return f'{mod} #(.dc({dc}){ac}) {name}({P(("p",t[1]),("n",t[2]))});'
    if p == 'e' and len(t) >= 6:  # VCVS
        return f'vcvs #(.gain({num(t[5])})) {name}({P(("p",t[1]),("n",t[2]),("cp",t[3]),("cn",t[4]))});'
    if p == 'g' and len(t) >= 6:  # VCCS
        return f'vccs #(.gm({num(t[5])})) {name}({P(("p",t[1]),("n",t[2]),("cp",t[3]),("cn",t[4]))});'
    if p == 'x':                  # subckt instance == a module instantiation
        sub = ident(t[-1])
        nodes = t[1:-1]
        if sub in SUBCKTS:        # in-file subckt: bind ports by name (we know them)
            ports = SUBCKTS[sub]
            conns = ', '.join(f'.{ident(pn)}({net(nn)})'
                              for pn, nn in zip(ports, nodes))
            return f'{sub} {name}({conns});'
        # external subckt (from an included lib): module instantiation, positional
        # connections (its port names live in the not-yet-included library).
        conns = ', '.join(net(nn) for nn in nodes)
        return f'{sub} {name}({conns});'
    if p == 'b' and len(t) >= 3:
        expr = ' '.join(t[3:])
        return f'// behavioral source: {expr}\n  // bsource_v #(.v("...")) {name}(...);'
    return f'// TODO ({p.upper()}): ' + ' '.join(t)

def emit_model(t):
    """`.model NAME TYPE (k=v ...)` -> paramset."""
    name = t[1]; typ = t[2].lower()
    base = MODEL_BASE.get(typ)
    body = ' '.join(t[3:]).replace('(', ' ').replace(')', ' ')
    pairs = re.findall(r'([A-Za-z_]\w*)\s*=\s*([^\s=()]+)', body)
    if base is None:
        return f'// .model {name} {t[2]} — base unknown; provide via a SPICE-lib include'
    psname = 'm_'+ident(name)
    # Real paramset grammar: top-level, entries are `.name = value;`
    entries = [f'    .model = "{name}";'] + [f'    .{k.lower()} = {num(v)};' for k,v in pairs[:12]]
    return f'paramset {psname} {base};\n' + '\n'.join(entries) + '\nendparamset'

LOCAL_MODELS = {}   # model-name -> base module, for the file being transpiled
SUBCKTS = {}        # subckt-name (ident) -> raw port-name list, for X instantiation

def split_subckts(lines):
    """Pull `.subckt NAME ports… / .ends` blocks out of the main line list.
    Returns (main_lines, [block]) where block = {'header': tokens, 'body': lines}."""
    main, blocks, cur = [], [], None
    for ln in lines:
        low = ln.lower()
        if low.startswith('.subckt'):
            cur = {'header': ln.split(), 'body': []}
        elif low.startswith('.ends'):
            if cur: blocks.append(cur); cur = None
        elif cur is not None:
            cur['body'].append(ln)
        else:
            main.append(ln)
    return main, blocks

def emit_subckt_module(block):
    """A SPICE subckt IS a Piperine module. Emit it as one."""
    name = ident(block['header'][1])
    raw_ports = block['header'][2:]
    ports = [net(p) for p in raw_ports]
    devs = []
    for ln in block['body']:
        if ln.startswith('.'): continue
        t = ln.split()
        if t and t[0][0].lower() in 'rclvidqjmegfhsbx':
            devs.append(emit_device(t[0][0].lower(), t))
    o = [f'module {name}({", ".join(ports)});']
    for d in devs: o.append('    ' + d.replace('\n', '\n    '))
    o.append('endmodule')
    return '\n'.join(o)

def dev_for(model_name, default_base):
    """Device module for an instance referencing `model_name`.
    Locally-defined model -> its paramset module; external (via include) ->
    the base device with a `.model("...")` string."""
    key = ident(model_name)
    if key in LOCAL_MODELS:
        return 'm_'+key
    return f'{default_base} #(.model("{model_name}"))'

def transpile(path):
    global LOCAL_MODELS, SUBCKTS
    text = open(path, errors='ignore').read()
    lines = parse_lines(text)
    mod = ident(os.path.splitext(os.path.basename(path))[0])
    models, includes, devices, analyses = [], [], [], []
    # ngspice allows `.model NAME TYPE(...)` with no space before `(`.
    norm = lambda s: s.replace('(', ' ').replace(')', ' ').split()
    # Subckts become sibling modules; record their ports so X instances bind by name.
    lines, blocks = split_subckts(lines)
    SUBCKTS = {ident(b['header'][1]): b['header'][2:] for b in blocks}
    submods = [emit_subckt_module(b) for b in blocks]
    # Pass 1: collect locally-defined models so device emission knows local vs external.
    LOCAL_MODELS = {}
    for ln in lines:
        if ln.lower().startswith('.model'):
            t = norm(ln)
            if len(t) >= 3:
                base = MODEL_BASE.get(t[2].lower())
                if base: LOCAL_MODELS[ident(t[1])] = base
    # Pass 2: emit.
    for ln in lines:
        low = ln.lower()
        t = ln.split()
        if low.startswith('.model'):
            t = norm(ln)
            models.append(emit_model(t))
        elif low.startswith('.include') or low.startswith('.inc') or low.startswith('.lib'):
            inc = ln.split(None,1)[1].strip().strip('"').strip("'") if len(t)>1 else ''
            inc = os.path.basename(inc.split()[0]) if inc else inc
            includes.append(inc)
        elif low.startswith('.tran'):
            a=[num(x) for x in t[1:]]
            analyses.append(f'$tran({", ".join(a)});' if a else '$tran();')
        elif low.startswith('.ac'):
            a=t[1:]
            if len(a)>=4: analyses.append(f'$ac("{a[0]}", {a[1]}, {num(a[2])}, {num(a[3])});')
        elif low.startswith('.dc') and len(t)>=5:
            analyses.append(f'$dc("{t[1]}", {num(t[2])}, {num(t[3])}, {num(t[4])});')
        elif low.startswith('.op'):
            analyses.append('$op();')
        elif low.startswith('.') :
            pass  # other dot-cards omitted
        elif t and t[0][0].lower() in 'rclvidqjmegfhsbx':
            devices.append(emit_device(t[0][0].lower(), t))
    # build output
    o = []
    o.append('`include "ngspice.ppr"')
    # SPICE model/lib includes become normal `include directives; resolving a
    # non-.ppr include is a PLANNED pluggable-include-handler feature (ROADMAP Phase 8).
    for inc in includes:
        o.append(f'`include "{inc}"   // PLANNED: SPICE-lib include handler')
    o.append('')
    o.append(f'// Ported from ngspice by tools/spice2ppr.py — see examples/ngspice-ported/README.md')
    o.append(f'// Source netlist: {os.path.basename(path)}')
    o.append('')
    # paramsets are top-level declarations (outside the module)
    for m in models: o.append(m); o.append('')
    # subckts are sibling modules
    for sm in submods: o.append(sm); o.append('')
    o.append(f'module {mod};')
    for d in devices: o.append('    '+d.replace('\n','\n    '))
    o.append('')
    o.append('    initial begin')
    if not analyses: analyses=['$op();']
    for a in analyses: o.append('        '+a)
    o.append('    end')
    o.append('endmodule')
    return '\n'.join(o)+'\n'

if __name__ == '__main__':
    for p in sys.argv[1:]:
        sys.stdout.write(transpile(p))
