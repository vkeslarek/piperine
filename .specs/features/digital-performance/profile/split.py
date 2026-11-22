import sys, re, collections
# split.py <fold>: each sample goes to the category of its innermost frame that matches one; and the
# share of samples whose leaf is in the allocator
CATS = [
    ("generated code (design::, its rt)", r"^design::"),
    ("shape walk per call (cell_lens, validate)", r"cell_lens|piperine_abi::validate"),
    ("ABI frame (caller/thunk, mutex, guarded)", r"^piperine_abi::"),
    ("observations and delivery", r"observe::|Obs::|Observation|Delivery|protocol::sink|Protocol::collect|drain_observations|adapters::\{impl#1\}::drain|observe_step"),
    ("journal (mark/rewind/release, store and queue logs)", r"journal|Journal|::mark$|::rewind$|::release$|Checkpoint|checkpoint"),
    ("queue and scheduling (push/take_due, fired_reactions, horizons, dirty set, dag walk)", r"queue::|Queue::|fired_reactions|earliest_other|DirtySet|run::walk|::horizon$|next_due|SettleCommits|settle::"),
    ("commit apply and store (apply_due, write_net, canonical checks, net_layout)", r"apply_due|write_net|fire_net|check_canonical|is_canonical|store::|Store::|net_layout|run::apply$|layout::"),
    ("per-call marshalling, solver side (call_function/call_reaction buffers)", r"call_function|call_reaction|cell_bytes|Aligned"),
    ("run_reactions bookkeeping", r"run_reactions"),
    ("instant loop and protocol (microsteps net scans, BTreeSet, advance, transient loop)", r"microsteps|Digital::advance|transient_loop|run_whole_instant|BTree|run_instant"),
]
ALLOC = re.compile(r"^(malloc|free|calloc|realloc|cfree|mprotect|_int_\w+|__rdl_\w+|alloc::alloc::|alloc::raw_vec::|std::sys::alloc|std::alloc::)")
f = sys.argv[1]
rows = [l.rstrip("\n").split(";") for l in open(f) if "transient_loop" in l]
n = len(rows)
cnt = collections.Counter(); alloc = 0; alloc_by = collections.Counter()
for fr in rows:
    cat = "other"
    for fn in fr:
        hit = next((c for c, p in CATS if re.search(p, fn)), None)
        if hit:
            cat = hit
            break
    cnt[cat] += 1
    leaf = [x for x in fr if x != "??"]
    if leaf and ALLOC.search(leaf[0]) or (fr and fr[0] in ("??",) and any(ALLOC.search(x) for x in fr[:3])):
        alloc += 1; alloc_by[cat] += 1
print(f"{n} samples of the run (the transient loop)")
for c, k in cnt.most_common():
    print(f"{100*k/n:5.1f}%  {c}   (allocator leaf: {100*alloc_by[c]/n:.1f}%)")
print(f"{100*alloc/n:5.1f}%  of all samples have their leaf in malloc/free/realloc (allocation, spread over the rows above)")
