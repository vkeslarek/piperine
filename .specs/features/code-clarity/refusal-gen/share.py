"""T21: every form built at more than one site outside `refusal/` becomes one method of `RefusedForm`
(`refusal/repeated.rs`), and the sites call it. Idempotent: run again after a change to re-derive the file."""
import collections, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, '..', '..', '..', '..', 'crates', 'piperine-lang', 'src')
FAMILIES = ['Value', 'Pattern', 'Analog', 'Digital', 'Structure', 'Generics', 'Constant']
USE = re.compile(r'\b(' + '|'.join(FAMILIES) + r')Form::([A-Za-z]+)\b')
REPEATED = os.path.join(SRC, 'refusal', 'repeated.rs')


def sources():
    for directory, _, files in os.walk(SRC):
        for name in files:
            path = os.path.join(directory, name)
            if name.endswith('.rs') and not os.path.relpath(path, SRC).startswith('refusal' + os.sep) and path != REPEATED:
                yield path


METHODS = {'Statement': 'statement_that_failed_to_lower'}


def snake(name):
    if name in METHODS:
        return METHODS[name]
    return re.sub(r'(?<!^)(?=[A-Z])', '_', name).lower()


def main():
    registry = json.load(open(os.path.join(HERE, 'registry.json')))
    text_of = {(e['family'], e['name']): t for t, e in registry.items()}
    counts = collections.Counter()
    for path in sources():
        for m in USE.finditer(open(path).read()):
            counts[(m.group(1), m.group(2))] += 1
    existing = open(REPEATED).read() if os.path.exists(REPEATED) else ''
    shared = {k for k, n in counts.items() if n > 1} | {(f, n) for f, n in re.findall(r'(\w+)Form::(\w+)\.into\(\)', existing)}
    for path in sources():
        text = open(path).read()
        new = USE.sub(lambda m: f'RefusedForm::{snake(m.group(2))}()' if (m.group(1), m.group(2)) in shared else m.group(0), text)
        if new == text:
            continue
        families = sorted({f for f in FAMILIES if f + 'Form::' in new})
        new = re.sub(r'^use crate::refusal::forms::[^\n]*\n', '', new, flags=re.M)
        if families:
            wanted = ', '.join(f + 'Form' for f in families)
            line = f'use crate::refusal::forms::{{{wanted}}};\n' if len(families) > 1 else f'use crate::refusal::forms::{families[0]}Form;\n'
            new = add_use(new, line)
        if not re.search(r'^use crate::refusal::(\{[^}]*\bRefusedForm\b|RefusedForm;)', new, re.M) and not re.search(r'^use crate::refusal::\{[^}]*RefusedForm', new, re.M):
            new = add_use(new, 'use crate::refusal::RefusedForm;\n')
        open(path, 'w').write(new)
    out = ['//! The forms refused from more than one place: each is built here once and the sites call the method, so a refusal has',
           '//! one owner whatever the number of places that raise it.', '', 'use super::forms::{' + ', '.join(f + 'Form' for f in FAMILIES) + '};', 'use super::RefusedForm;', '', 'impl RefusedForm {']
    for family, name in sorted(shared):
        words = text_of[(family, name)].replace('\n', ' ')
        out += [f'    /// The form of {words}, refused from more than one place.', f'    pub fn {snake(name)}() -> RefusedForm {{', f'        {family}Form::{name}.into()', '    }', '']
    out.pop()
    out.append('}')
    open(REPEATED, 'w').write('\n'.join(out) + '\n')
    print(len(shared), 'shared forms')


def add_use(text, line):
    match = re.search(r'^use crate::[^\n]*\n', text, re.M) or re.search(r'^use [^\n]*\n', text, re.M)
    return text[:match.start()] + line + text[match.start():] if match else line + text


if __name__ == '__main__':
    main()
