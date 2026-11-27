#!/usr/bin/env python3
"""Rewrites the `P0001` refusal sites of `crates/piperine-lang/src` from free text to named forms (T17-T20).

Usage: generate.py <path-prefix>... : rewrites every site of the files under those prefixes (relative to
`crates/piperine-lang/src`), records the forms in `registry.json` (this directory) and regenerates
`crates/piperine-lang/src/refusal/forms.rs` from the registry. Sites it cannot read (a dynamic message) are
listed on stdout and left alone: those are rewritten by hand.

This is build tooling kept with the feature, not part of the crate.
"""
import json
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '../../../..'))
SRC = os.environ.get('REFUSAL_SRC') or os.path.join(ROOT, 'crates/piperine-lang/src')
REGISTRY = os.environ.get('REFUSAL_REGISTRY') or os.path.join(os.path.dirname(__file__), 'registry.json')
FORMS = os.path.join(SRC, 'refusal/forms.rs')

FAMILIES = ['Value', 'Pattern', 'Analog', 'Digital', 'Structure', 'Generics', 'Constant']
STOP = {'a', 'an', 'the', 'this', 'that', 'is', 'are', 'of', 'in', 'as', 'to', 'it', 'its', 'by', 'at', 'on', 'with', 'for', 'no', 'not'}
KEEP_SHORT = {'no', 'not', 'on', 'in', 'as', 'of', 'with', 'by'}


def family_of(path):
    base = path.replace('\\', '/')
    if base.startswith('elab/consts'):
        return 'Constant'
    name = os.path.basename(base)
    if 'pattern' in name:
        return 'Pattern'
    if re.search(r'analog|nature|noise|contribute|derivative|symbolic|event|delay|signal_flow|table_model|intrinsic|level|reaction|operators|partial', name):
        return 'Analog'
    if re.search(r'carrier|drive|digital|element|handle|bundle|loops|resolver|logic', name):
        return 'Digital'
    if re.search(r'child|instance|reach|hoist|templates|generation|expansion|body|module|structural|variant|unroll|host_source|placement|crate|result', name):
        return 'Structure'
    if re.search(r'generic|substitution|bound|associated|derive|enum|trait|impl|methods', name):
        return 'Generics'
    return 'Value'


class Source:
    """A Rust source text with the scanning helpers a call-site rewrite needs."""

    def __init__(self, text):
        self.text = text

    def skip_string(self, at):
        """The index after the string literal starting at `at`."""
        i = at + 1
        while self.text[i] != '"':
            i += 2 if self.text[i] == '\\' else 1
        return i + 1

    def matching(self, open_at):
        """The index of the bracket closing the one at `open_at`."""
        pairs = {'(': ')', '[': ']', '{': '}'}
        depth = 0
        i = open_at
        while i < len(self.text):
            ch = self.text[i]
            if ch == '"':
                i = self.skip_string(i)
                continue
            if ch == "'" and re.match(r"'(\\.|[^\\'])'", self.text[i:i + 4]):
                i += 3 if self.text[i + 1] != '\\' else 4
                continue
            if ch in pairs:
                depth += 1
            elif ch in ')]}':
                depth -= 1
                if depth == 0:
                    return i
            i += 1
        raise ValueError('unbalanced')

    def arguments(self, open_at, close_at):
        """The top-level comma-separated arguments between two brackets."""
        args, depth, start, i = [], 0, open_at + 1, open_at + 1
        while i < close_at:
            ch = self.text[i]
            if ch == '"':
                i = self.skip_string(i)
                continue
            if ch == "'" and re.match(r"'(\\.|[^\\'])'", self.text[i:i + 4]):
                i += 3 if self.text[i + 1] != '\\' else 4
                continue
            if ch in '([{':
                depth += 1
            elif ch in ')]}':
                depth -= 1
            elif ch == ',' and depth == 0:
                args.append(self.text[start:i].strip())
                start = i + 1
            i += 1
        tail = self.text[start:close_at].strip()
        if tail:
            args.append(tail)
        return args


OPERATORS = {'+': 'Add', '-': 'Subtract', '*': 'Multiply', '/': 'Divide', '%': 'Remainder', '^': 'BitXor', '&': 'BitAnd', '|': 'BitOr',
             '<<': 'ShiftLeft', '>>': 'ShiftRight', '&&': 'LogicalAnd', '||': 'LogicalOr', '==': 'Equal', '!=': 'NotEqual', '<': 'Less',
             '<=': 'LessOrEqual', '>': 'Greater', '>=': 'GreaterOrEqual'}

# names the words of the text cannot give: it collides with another form's words
OVERRIDES = {'a `bool` where an `f64` is required': 'BoolWhereFloatIsRequired', 'an `f64` where a `bool` is required': 'FloatWhereBoolIsRequired', 'a composite value where an `f64` is required': 'CompositeValueWhereFloatIsRequired', 'an integer where an `f64` is required': 'IntegerWhereFloatIsRequired', 'a number-width value where an `f64` is required': 'NumberWidthValueWhereFloatIsRequired', 'a comparison of an `f64` and a `bool`': 'ComparisonOfFloatAndBool', 'a row outside Appendix H.5 to H.7': 'RowOutsideAppendixRows', 'an operator other than a comparison on `Option`s': 'OperatorOtherThanComparisonOnOptions', '{}': 'FormWithNoDerivativeRule', 'an iterator method outside Appendix H.7': 'IteratorMethodOutsideAppendix', 'this expression': 'ThisExpression', 'a function without a body': 'LoweredFunctionWithoutBody', 'a method call': 'LoweredMethodCall', 'this type': 'ThisType', 'this pattern': 'ThisPattern', '`#[repr(..)]` other than `#[repr(uN)]` with `N` from 1 to 64': 'ReprOtherThanSizedInteger', "`logic` without the standard library's `Logic`": 'LogicWithoutStandardLibraryLogic', "a call of a trait's function with no `self`": 'CallOfTraitFunctionWithNoReceiver', 'comparison of values that are neither `f64`, `bool` nor an integer': 'ComparisonOfValuesThatAreNeitherFloatBoolNorInteger', 'literal': 'LiteralExpression', 'type': 'TypeName', 'binding': 'BindingPattern', 'unit type': 'UnitType'}


class Registry:
    """Every form seen: its text, family and variant name; persisted between runs."""

    def __init__(self):
        self.forms = {}
        if os.path.exists(REGISTRY):
            self.forms = json.load(open(REGISTRY))

    def save(self):
        json.dump(self.forms, open(REGISTRY, 'w'), indent=1, sort_keys=True)

    def variant(self, text, family):
        if text in self.forms:
            return self.forms[text]['family'], self.forms[text]['name']
        taken = {(f['family'], f['name']) for f in self.forms.values()}
        operator = re.fullmatch(r'operator `(.+)`', text)
        if text in OVERRIDES or operator:
            name = OVERRIDES.get(text) or 'Operator' + OPERATORS[operator.group(1)]
            self.forms[text] = {'family': family, 'name': name}
            return family, name
        words = self.words(text)
        for count in range(min(7, len(words)), len(words) + 1):
            name = ''.join(w.capitalize() for w in words[:count])
            if (family, name) not in taken:
                break
        else:
            name = ''.join(w.capitalize() for w in words)
            n = 2
            while (family, name) in taken:
                name = ''.join(w.capitalize() for w in words) + 'Again' * (n - 1)
                n += 1
        self.forms[text] = {'family': family, 'name': name}
        return family, name

    @staticmethod
    def words(text):
        text = re.sub(r"'s\b", '', re.sub(r'\{[^}]*\}', ' ', text))
        words = [w.lower() for w in re.findall(r"[A-Za-z]+", text)]
        trimmed = list(words)
        while trimmed and trimmed[0] in ('a', 'an', 'the', 'this'):
            trimmed.pop(0)
        trimmed = [w for w in trimmed if w not in ('a', 'an', 'the')] or ['refused']
        return trimmed[:12]


def parse_literal(arg):
    """The text of a plain string-literal argument, or None."""
    match = re.fullmatch(r'"((?:[^"\\]|\\.)*)"(?:\.to_string\(\))?', arg, re.S)
    return match.group(1) if match else None


def parse_format(source_args):
    """(template, [argument expressions]) of a `format!(..)` argument, or None."""
    match = re.fullmatch(r'&?format!\((.*)\)', source_args, re.S)
    if not match:
        return None
    inner = Source('(' + match.group(1) + ')')
    parts = inner.arguments(0, len(inner.text) - 1)
    literal = parse_literal(parts[0])
    if literal is None:
        return None
    return literal, parts[1:]


def detail_expressions(template, args):
    """The template with each placeholder as `{}` and the expression that gives each detail."""
    out, details, position, i = '', [], 0, 0
    pattern = re.compile(r'\{\{|\}\}|\{([^{}]*)\}')
    last = 0
    named_extra = {}
    for m in pattern.finditer(template):
        out += template[last:m.start()]
        last = m.end()
        token = m.group(0)
        if token in ('{{', '}}'):
            out += token
            continue
        inner = m.group(1)
        name, _, spec = inner.partition(':')
        if name == '':
            expression = args[position]
            position += 1
        elif name.isdigit():
            expression = args[int(name)]
        elif re.fullmatch(r'[A-Za-z_]\w*', name):
            named = [a for a in args if re.match(r'%s\s*=' % re.escape(name), a)]
            expression = named[0].split('=', 1)[1].strip() if named else name
        else:
            return None
        out += '{}'
        details.append(f'format!("{{{":" + spec if spec else ""}}}", {expression})' if spec else f'{expression}.to_string()')
    out += template[last:]
    return out, details


CHANNELS = [
    # (regex of the call head, channel kind, index of the `what` argument)
    (re.compile(r'\b(?:self|this)\.unsupported\('), 'diagnostic', 2),
    (re.compile(r'\bself\.unsupported_unless_refused\('), 'unless', 3),
    (re.compile(r'\bConstFault::refused\('), 'const', 1),
    (re.compile(r'\bSelf::unsupported\('), 'const', 1),
    (re.compile(r'\bself\.refuse_place\('), 'diagnostic', 2),
    (re.compile(r'\bself\.refuse_silent_failure\('), 'diagnostic', 2),
]


def rewrite_file(path, registry, report):
    full = os.path.join(SRC, path)
    text = open(full).read()
    family = family_of(path)
    out, cursor = [], 0
    source = Source(text)
    sites = []
    for regex, kind, index in CHANNELS:
        for m in regex.finditer(text):
            open_at = m.end() - 1
            if kind == 'const' and regex.pattern.startswith(r'\bSelf') and '/consts/' not in '/' + path:
                continue
            sites.append((m.start(), open_at, kind, index))
    sites.sort()
    for start, open_at, kind, index in sites:
        if start < cursor:
            continue
        close_at = source.matching(open_at)
        args = source.arguments(open_at, close_at)
        if len(args) <= index:
            report.append((path, text[:start].count('\n') + 1, 'arity', text[start:close_at + 1][:100]))
            continue
        what = args[index]
        if kind == 'const':
            inner = re.fullmatch(r'RefusedForm::Text\((.*)\)', what, re.S)
            if inner:
                what = inner.group(1)
        literal = parse_literal(what)
        details = []
        if literal is not None:
            template = literal
        else:
            formatted = parse_format(what[:-0] if False else what)
            if formatted is None:
                report.append((path, text[:start].count('\n') + 1, 'dynamic', what[:100]))
                continue
            built = detail_expressions(*formatted)
            if built is None:
                report.append((path, text[:start].count('\n') + 1, 'format', what[:100]))
                continue
            template, details = built
        fam, name = registry.variant(template, family)
        form = f'{fam}Form::{name}'
        head = text[start:open_at + 1]
        rest = [a for i, a in enumerate(args) if i != index]
        if kind == 'const' and head.startswith('Self::unsupported'):
            head = 'ConstFault::refused('
        if kind == 'const':
            rest = [args[0]]
        if details:
            channel = head.replace('unsupported_unless_refused(', 'refuse_unless_refused_with(').replace('unsupported(', 'refuse_with(').replace('refused(', 'refused_with(')
            call_args = rest[:index] + [form, '[' + ', '.join(details) + ']'] if kind != 'const' else [args[0], form, '[' + ', '.join(details) + ']']
        else:
            channel = head.replace('unsupported_unless_refused(', 'refuse_unless_refused(').replace('unsupported(', 'refuse(')
            call_args = rest[:index] + [form] if kind != 'const' else [args[0], form]
        if kind != 'const' and len(rest) > index:
            call_args = rest[:index] + call_args[index:]
        out.append(text[cursor:start])
        out.append(channel + ', '.join(call_args) + ')')
        cursor = close_at + 1
    out.append(text[cursor:])
    new = ''.join(out)
    if new != text:
        new = ensure_imports(new, registry)
        open(full, 'w').write(new)


def ensure_imports(text, registry):
    families = sorted({f for f in FAMILIES if f + 'Form::' in text})
    if not families:
        return text
    wanted = ', '.join(families and [f + 'Form' for f in families])
    line = f'use crate::refusal::forms::{{{wanted}}};\n' if len(families) > 1 else f'use crate::refusal::forms::{families[0]}Form;\n'
    text = re.sub(r'^use crate::refusal::forms::[^\n]*\n', '', text, flags=re.M)
    match = re.search(r'^use crate::[^\n]*\n', text, re.M) or re.search(r'^use [^\n]*\n', text, re.M)
    return text[:match.start()] + line + text[match.start():] if match else line + text


def rust_string(text):
    return '"' + text.replace('\\', '\\\\').replace('"', '\\"') + '"'


def write_forms(registry):
    by_family = {f: [] for f in FAMILIES}
    for text, entry in sorted(registry.forms.items(), key=lambda kv: (kv[1]['family'], kv[1]['name'])):
        by_family[entry['family']].append((entry['name'], text, entry.get('owner')))
    out = ['//! The refusal table: every form the front end refuses by name, grouped by family, with the words a user reads and who',
           '//! owns closing it. Generated once from the sites that refused in free text; a new form is one variant, one row (in the same order) of',
           '//! `ROWS` and one entry of `ALL`.',
           '', 'use super::{RefusalOwner, RefusalRow, RefusedForm};', '']
    for family in FAMILIES:
        rows = by_family[family]
        enum = family + 'Form'
        out.append(f'/// The {family.lower()} forms the front end refuses.')
        out.append('#[derive(Debug, Clone, Copy, PartialEq, Eq)]')
        out.append(f'pub enum {enum} {{')
        for name, text, _ in rows:
            out.append(f'    /// {text.replace(chr(10), " ")}')
            out.append(f'    {name},')
        out.append('}\n')
        out.append(f'impl {enum} {{')
        out.append('    /// Every form of the family, in declaration order.')
        out.append('    pub const ALL: &[' + enum + '] = &[')
        for name, _, _ in rows:
            out.append(f'        {enum}::{name},')
        out.append('    ];\n')
        out.append('    /// The rows of the family, one per form in declaration order.')
        out.append('    const ROWS: &[RefusalRow] = &[')
        for name, text, owner in rows:
            owner_code = f'RefusalOwner::Gap({rust_string(owner)})' if owner else 'RefusalOwner::Unswept'
            out.append(f'        RefusalRow {{ form: RefusedForm::{family}({enum}::{name}), text: {rust_string(text)}, owner: {owner_code} }},')
        out.append('    ];\n')
        out.append('    /// The row of the form: its words and its owner.')
        out.append('    pub fn row(self) -> &\'static RefusalRow {')
        out.append('        &Self::ROWS[self as usize]')
        out.append('    }')
        out.append('}\n')
        out.append(f'impl From<{enum}> for RefusedForm {{')
        out.append(f'    fn from(form: {enum}) -> RefusedForm {{')
        out.append(f'        RefusedForm::{family}(form)')
        out.append('    }')
        out.append('}\n')
    os.makedirs(os.path.dirname(FORMS), exist_ok=True)
    open(FORMS, 'w').write('\n'.join(out))


def main():
    registry = Registry()
    report = []
    for prefix in sys.argv[1:]:
        base = os.path.join(SRC, prefix)
        files = [prefix] if os.path.isfile(base) else [os.path.relpath(os.path.join(d, f), SRC) for d, _, fs in os.walk(base) for f in fs if f.endswith('.rs')]
        for path in sorted(files):
            rewrite_file(path, registry, report)
    registry.save()
    write_forms(registry)
    for entry in report:
        print('UNHANDLED %s:%d %s: %s' % entry)
    print(len(registry.forms), 'forms;', len(report), 'unhandled')


if __name__ == '__main__':
    main()
