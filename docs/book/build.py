import html, re, pathlib

HERE = pathlib.Path(__file__).parent
content = (HERE / "content.html").read_text()

KW = set("""module nature signal on out flip after in extern analog digital for fn let mut pub use mod
crate self super Self struct enum impl trait const if else match while loop return where as true false
break continue type static unsafe dyn async ref move""".split())
CONSTS = {"Some", "None", "Unknown", "gnd", "Static", "Dc", "Transient", "Tran", "Ac", "Noise"}
PRIM = set("bool f16 f32 f64 f128 usize isize str u1 u2 u3 u4 u5 u8 u10 u16 u32 u64 u128 i2 i8 i16 i32 i40 i64 i128".split())

TOK = re.compile(r"""
 (?P<com>//[^\n]*)
|(?P<str>"(?:\\.|[^"\\])*")
|(?P<attr>\#\[[^\]\n]*\])
|(?P<mac>\b(?:emit|include_str)!)
|(?P<arr>\+>|<-)
|(?P<num>\b0x[0-9A-Fa-f_]+(?:[ui]\d+)?\b|\b\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?(?:[TGMkmunpfa](?![A-Za-z0-9_])|[ui]\d+\b|f\d+\b)?)
|(?P<life>'static)
|(?P<id>[A-Za-z_][A-Za-z0-9_]*)
""", re.X)

def hl_code(src):
    out, i = [], 0
    for m in TOK.finditer(src):
        out.append(html.escape(src[i:m.start()], quote=False))
        kind, t = m.lastgroup, m.group()
        e = html.escape(t, quote=False)
        if kind == "id":
            nxt = src[m.end():m.end() + 1]
            if t in KW or (t in ("potential", "flow", "value", "resolve") and nxt == ":"):
                cls = "kw"
            elif t in CONSTS:
                cls = "cn"
            elif t in PRIM or t[0].isupper():
                cls = "ty"
            elif nxt == "(":
                cls = "fn"
            else:
                cls = None
            out.append(f'<span class="{cls}">{e}</span>' if cls else e)
        else:
            cls = {"com": "com", "str": "st", "attr": "at", "mac": "mc", "arr": "ar", "num": "nu", "life": "kw"}[kind]
            out.append(f'<span class="{cls}">{e}</span>')
        i = m.end()
    out.append(html.escape(src[i:], quote=False))
    return "".join(out)

def hl_toml(src):
    lines = []
    for line in src.split("\n"):
        e = html.escape(line, quote=False)
        if line.strip().startswith("#"):
            lines.append(f'<span class="com">{e}</span>'); continue
        if re.match(r"\s*\[", line) and "=" not in line:
            lines.append(f'<span class="kw">{e}</span>'); continue
        # trailing comment
        com = ""
        m = re.search(r'\s#[^"]*$', line)
        if m:
            com = f'<span class="com">{html.escape(m.group(), quote=False)}</span>'
            line = line[:m.start()]
        parts = re.split(r'("(?:[^"\\]|\\.)*")', line)
        buf = []
        for j, p in enumerate(parts):
            if j % 2:
                buf.append(f'<span class="st">{html.escape(p, quote=False)}</span>')
            else:
                p2 = html.escape(p, quote=False)
                p2 = re.sub(r"^(\s*)([A-Za-z_][\w-]*)(\s*=)", r'\1<span class="ty">\2</span>\3', p2)
                p2 = re.sub(r"([{,]\s*)([A-Za-z_][\w-]*)(\s*=)", r'\1<span class="fn">\2</span>\3', p2)
                p2 = re.sub(r"(=\s*)(\d+)\b", r'\1<span class="nu">\2</span>', p2)
                buf.append(p2)
        lines.append("".join(buf) + com)
    return "\n".join(lines)

def hl_diag(src):
    lines = []
    for line in src.split("\n"):
        e = html.escape(line, quote=False)
        if line.startswith("error"):
            e = re.sub(r"^(error\[E\d+\])", r'<span class="er">\1</span>', e)
            e = f'<span class="dm">{e}</span>'
        elif "= help" in line:
            e = e.replace("= help:", '= <span class="hp">help:</span>')
        elif re.search(r"\^+", line) or re.search(r"-{3,}", line):
            e = re.sub(r"(\^+)", r'<span class="er">\1</span>', e)
            e = re.sub(r"(-{3,})", r'<span class="lb2">\1</span>', e)
            e = f'<span class="lb">{e}</span>' if "^" in line else e
        elif re.match(r"\s*-->", line) or re.match(r"\s*\d*\s*\|", line):
            e = re.sub(r"^(\s*\d*\s*\|)", r'<span class="gut">\1</span>', e)
            e = re.sub(r"^(\s*--&gt;)", r'<span class="gut">\1</span>', e)
        lines.append(e)
    return "\n".join(lines)

LANG_LABEL = {"msl": "msl", "rust": "rust · host", "toml": "toml", "text": "", "diag": "terminal", "sh": "shell"}

store = []
def keep(s):
    store.append(s)
    return f"\x00{len(store)-1}\x00"

def pre_block(m):
    # Extra attributes of the <pre> (data-cli-output: a listing the command line prints) are kept on the block.
    lang, attrs, raw = m.group(1), m.group(2), m.group(3)
    raw = raw.strip("\n")
    if lang in ("msl", "rust"):
        body = hl_code(raw)
    elif lang == "toml":
        body = hl_toml(raw)
    elif lang == "diag":
        body = hl_diag(raw)
    else:
        body = html.escape(raw, quote=False)
    label = LANG_LABEL.get(lang, lang)
    if "data-cli-output" in attrs:
        label = "output"
    lab = f'<span class="lang">{label}</span>' if label else ""
    return keep(f'<div class="code {lang}"{attrs}>{lab}<pre><code>{body}</code></pre></div>')

content = re.sub(r'<pre data-lang="(\w+)"([^>]*)>(.*?)</pre>', pre_block, content, flags=re.S)
content = re.sub(r'<pre class="mermaid">(.*?)</pre>', lambda m: keep(f'<div class="diagram"><pre class="mermaid">{m.group(1)}</pre></div>'), content, flags=re.S)

def code_inline(m):
    raw = html.unescape(m.group(1))
    return f"<code>{html.escape(raw, quote=False)}</code>"
content = re.sub(r"<code>(.*?)</code>", code_inline, content, flags=re.S)

content = re.sub(r"\x00(\d+)\x00", lambda m: store[int(m.group(1))], content)

chapters = re.findall(r'<section id="([\w-]+)" class="chapter">\s*<p class="ch-num">Chapter (\d+)</p>\s*<h2>(.*?)</h2>', content)
nav = "\n".join(
    f'<li><a href="#{cid}" data-target="{cid}"><span class="n">{num}</span><span class="t">{title}</span></a></li>'
    for cid, num, title in chapters)

template = (HERE / "template.html").read_text()
out = template.replace("{{NAV}}", nav).replace("{{CONTENT}}", content)
dest = HERE / "index.html"
dest.write_text(out)
print(dest, len(out))
