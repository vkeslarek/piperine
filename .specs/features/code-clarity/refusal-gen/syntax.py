#!/usr/bin/env python3
"""(Run once, T17: it reads `types/unsupported.rs`, whose `binary`, `stmt` and `ty` it then leaves.) Writes `refusal/syntax.rs`: the forms of the syntax a kind of the POM names (T17), read from the arms of
`types/unsupported.rs` (`expr`, `binary`, `stmt`, `pat`, `ty`), and records their forms in the registry.
Run before `generate.py` regenerates `forms.rs`."""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(__file__))
import generate  # noqa: E402

SOURCE = os.path.join(generate.SRC, 'types/unsupported.rs')
TABLES = [('expression', 'expr', 'ExprKind', 'Value'), ('binary_operator', 'binary', 'BinOp', 'Value'), ('statement', 'stmt', 'StmtKind', 'Value'),
          ('pattern', 'pat', 'PatKind', 'Pattern'), ('type_form', 'ty', 'TypeKind', 'Value'), ('item', 'item', 'DeclKind', 'Value')]
DOCS = {'expression': 'The form of an expression the checker does not type', 'binary_operator': 'The form of a binary operator the checker does not type',
        'statement': 'The form of a statement the checker does not type', 'pattern': 'The form of a pattern the checker does not type',
        'type_form': 'The form of a type outside the supported subset', 'item': 'The form of a declaration the checker does not type where it is reached'}


def arms(text, function):
    body = text[text.index(f'fn {function}('):]
    body = body[:body.index('\n    }\n')]
    return re.findall(r'^\s*(\S.*?) => "((?:[^"\\]|\\.)*)",$', body, re.M)


def main():
    text = open(SOURCE).read()
    registry = generate.Registry()
    out = ['//! The forms of the syntax a kind of the object model names: what the checker refuses when it meets an expression, an',
           '//! operator, a statement, a pattern or a type it does not type. Every `match` is exhaustive with no wildcard arm, so a',
           '//! new kind fails to compile until it is classified.', '',
           'use super::forms::{PatternForm, ValueForm};', 'use super::RefusedForm;',
           'use crate::pom::{BinOp, DeclKind, ExprKind, PatKind, StmtKind, TypeKind};', '', 'impl RefusedForm {']
    for method, function, kind, family in TABLES:
        param = 'op: BinOp' if kind == 'BinOp' else f'kind: &{kind}'
        out.append(f'    /// {DOCS[method]}.')
        out.append(f'    pub fn {method}({param}) -> RefusedForm {{')
        out.append(f'        match {"op" if kind == "BinOp" else "kind"} {{')
        for pattern, words in arms(text, function):
            fam, name = registry.variant(words, family)
            out.append(f'            {pattern} => {fam}Form::{name}.into(),')
        out.append('        }')
        out.append('    }')
        out.append('')
    out.pop()
    out.append('}')
    open(os.path.join(generate.SRC, 'refusal/syntax.rs'), 'w').write('\n'.join(out) + '\n')
    registry.save()
    generate.write_forms(registry)


if __name__ == '__main__':
    main()
