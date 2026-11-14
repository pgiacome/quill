#!/usr/bin/env python3
"""Community overlay check (docs/decisions/language-packs.md).

`lang/<code>/*.strings` translates Quill's own `qll_*` strings on top of the
Telegram pack. This checker reads them together with `lang/en/*.strings` and
fails when:

- an entry uses a `lng_*` key: those come from Telegram, not from Quill;
- an entry gives a form (`#one`, `#few`, …) to an ordinary English key, or no
  form to a plural one;
- the same base key appears twice in one file, or two files disagree on its
  kind or on the value of a form (one key, one translation);
- a value has unbalanced `{`/`}`;
- a plural entry lacks `#one`, `#few`, `#many` or `#other`;
- the `{tags}` of a value differ from the English value (the same form when
  English has it, `#other` otherwise) or repeat a different number of times;
- a present overlay file misses an English `qll_*` key of its counterpart,
  or a plural key has no `#other` form.

Entries whose base key is not in English are skipped: the calls branch will
add `lang/ru/calls.strings` before its English keys land.

`--require-all` also requires an overlay file for every English file that
contains at least one `qll_*` key. It fails until every screen is translated,
so the per-task gate is the plain form.

Usage: python3 scripts/check-lang-overlay.py <code> [--require-all]
       (from the repository root)
"""
import os
import re
import sys
from collections import Counter

FORMS = ('zero', 'one', 'two', 'few', 'many', 'other')
TAG = re.compile(r'\{([^{}]*)\}')


class Broken(Exception):
    """A `.strings` file does not parse, as `src/lang/parse.rs` would say."""


def parse_strings(text):
    """`(key, value, line)` entries; the rules of `src/lang/parse.rs`."""
    entries = []
    size = len(text)
    i = 0
    line = 1

    def skip_blank(i, line):
        while i < size:
            c = text[i]
            if c == '\n':
                line += 1
                i += 1
            elif c.isspace():
                i += 1
            elif c == '/':
                if i + 1 >= size:
                    raise Broken('line %d: unexpected %r' % (line, '/'))
                nxt = text[i + 1]
                if nxt == '/':
                    i += 2
                    while i < size and text[i] != '\n':
                        i += 1
                elif nxt == '*':
                    i += 2
                    prev = ' '
                    while i < size:
                        c2 = text[i]
                        if c2 == '\n':
                            line += 1
                        i += 1
                        if prev == '*' and c2 == '/':
                            break
                        prev = c2
                    else:
                        raise Broken('line %d: unterminated comment' % line)
                else:
                    raise Broken('line %d: unexpected %r then %r' % (line, '/', nxt))
            else:
                break
        return i, line

    def quoted(i, line):
        if i >= size or text[i] != '"':
            raise Broken('line %d: expected %r, found %r'
                         % (line, '"', text[i] if i < size else None))
        i += 1
        out = []
        while i < size:
            c = text[i]
            if c == '"':
                return ''.join(out), i + 1, line
            if c == '\\':
                i += 1
                if i >= size:
                    raise Broken('line %d: unterminated string' % line)
                esc = text[i]
                if esc == 'n':
                    out.append('\n')
                elif esc == 't':
                    out.append('\t')
                elif esc in '"\\':
                    out.append(esc)
                else:
                    raise Broken('line %d: unknown escape \\%r' % (line, esc))
                i += 1
            else:
                if c == '\n':
                    line += 1
                out.append(c)
                i += 1
        raise Broken('line %d: unterminated string' % line)

    while True:
        i, line = skip_blank(i, line)
        if i >= size:
            return entries
        key_line = line
        key, i, line = quoted(i, line)
        i, line = skip_blank(i, line)
        if i >= size or text[i] != '=':
            raise Broken('line %d: expected %r, found %r'
                         % (line, '=', text[i] if i < size else None))
        i += 1
        i, line = skip_blank(i, line)
        value, i, line = quoted(i, line)
        i, line = skip_blank(i, line)
        if i >= size or text[i] != ';':
            raise Broken('line %d: expected %r, found %r'
                         % (line, ';', text[i] if i < size else None))
        i += 1
        entries.append((key, value, key_line))


def split_plural(key):
    """`foo#few` → (`foo`, `few`); an ordinary key → (`foo`, None)."""
    base, sep, suffix = key.rpartition('#')
    if sep and suffix in FORMS:
        return base, suffix
    return key, None


def braces_balanced(value):
    """No nesting and no stray brace, as `parse::braces_balanced`."""
    open_brace = False
    for c in value:
        if c == '{':
            if open_brace:
                return False
            open_brace = True
        elif c == '}':
            if not open_brace:
                return False
            open_brace = False
    return not open_brace


def strings_files(directory):
    if not os.path.isdir(directory):
        return []
    return sorted(name for name in os.listdir(directory) if name.endswith('.strings'))


def read_file(directory, name):
    path = os.path.join(directory, name)
    with open(path, encoding='utf-8') as file:
        return file.read()


def load_english(directory, errors):
    """base key → {'plural': bool, 'text': str|None, 'forms': dict, 'file': name}."""
    english = {}
    for name in strings_files(directory):
        try:
            entries = parse_strings(read_file(directory, name))
        except (Broken, OSError) as err:
            errors.append('%s/%s: %s' % (directory, name, err))
            continue
        for key, value, line in entries:
            base, form = split_plural(key)
            record = english.get(base)
            if record is None:
                record = {'plural': form is not None, 'text': None,
                          'forms': {}, 'file': name}
                english[base] = record
            if (record['plural']) != (form is not None):
                errors.append('%s/%s:%d: key %r changes kind between files'
                              % (directory, name, line, base))
                continue
            if form is None:
                record['text'] = value
            else:
                record['forms'][form] = value
    return english


def tags(value):
    return Counter(TAG.findall(value or ''))


def check_overlay(code, root, errors):
    directory = os.path.join(root, 'lang', code)
    english_dir = os.path.join(root, 'lang', 'en')
    english = load_english(english_dir, errors)
    english_by_file = {}
    for name in strings_files(english_dir):
        try:
            entries = parse_strings(read_file(english_dir, name))
        except (Broken, OSError):
            continue
        english_by_file[name] = entries

    present = strings_files(directory)
    # One key, one translation: the same base key may sit in two overlay
    # files only with the same kind and the same value for every form.
    defined = {}
    kinds = {}
    for name in present:
        relative = 'lang/%s/%s' % (code, name)
        try:
            entries = parse_strings(read_file(directory, name))
        except (Broken, OSError) as err:
            errors.append('%s: %s' % (relative, err))
            continue
        seen = set()
        local = {}
        for key, value, line in entries:
            where = '%s:%d' % (relative, line)
            base, form = split_plural(key)
            if key.startswith('lng_') or base.startswith('lng_'):
                errors.append('%s: %r is a Telegram key; the overlay translates '
                              'qll_* only' % (where, key))
                continue
            if key in seen:
                errors.append('%s: duplicate key %r' % (where, key))
                continue
            seen.add(key)
            if not braces_balanced(value):
                errors.append('%s: %r has unbalanced braces' % (where, key))
                continue
            local.setdefault(base, set()).add(form)
            previous_kind = kinds.get(base)
            if previous_kind is None:
                kinds[base] = (form is not None, relative, line)
            elif previous_kind[0] != (form is not None):
                errors.append('%s: %r is ordinary here but plural in %s:%d'
                              % (where, base, previous_kind[1], previous_kind[2]))
            previous = defined.get((base, form))
            if previous is None:
                defined[(base, form)] = (value, relative, line)
            elif previous[0] != value:
                errors.append('%s: %r differs from its translation in %s:%d'
                              % (where, key, previous[1], previous[2]))
            record = english.get(base)
            if record is None:
                continue
            if record['plural'] and form is None:
                errors.append('%s: %r is plural in English and needs a form '
                              '(#one, #few, #many, #other)' % (where, base))
                continue
            if not record['plural'] and form is not None:
                errors.append('%s: %r is ordinary in English and takes no form'
                              % (where, base))
                continue
            if record['plural']:
                reference = record['forms'].get(form)
                if reference is None:
                    reference = record['forms'].get('other')
            else:
                reference = record['text']
            if tags(value) != tags(reference):
                errors.append('%s: %r has the tags %s, English has %s'
                              % (where, key, sorted(tags(value).elements()),
                                 sorted(tags(reference).elements())))

        for base, forms in local.items():
            if forms == {None}:
                continue
            missing = [form for form in ('one', 'few', 'many', 'other')
                       if form not in forms]
            if missing:
                errors.append('%s: %r has no %s'
                              % (relative, base,
                                 ', '.join('#' + form for form in missing)))

        counterpart = english_by_file.get(name)
        if counterpart is None:
            continue
        for key, value, line in counterpart:
            base, form = split_plural(key)
            if not base.startswith('qll_'):
                continue
            if base not in local:
                errors.append('%s: missing %r (lang/en/%s:%d)'
                              % (relative, base, name, line))
            elif english[base]['plural'] and 'other' not in local[base]:
                errors.append('%s: %r has no #other form' % (relative, base))
    return english_by_file, present


def require_all(code, english_by_file, present, errors):
    for name, entries in sorted(english_by_file.items()):
        if not any(split_plural(key)[0].startswith('qll_')
                   for key, _, _ in entries):
            continue
        if name not in present:
            errors.append('lang/%s/%s: missing (lang/en/%s has qll_* keys)'
                          % (code, name, name))


def main():
    arguments = [arg for arg in sys.argv[1:] if not arg.startswith('--')]
    if len(arguments) != 1:
        print('usage: python3 scripts/check-lang-overlay.py <code> [--require-all]')
        return 2
    code = arguments[0]
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    errors = []
    english_by_file, present = check_overlay(code, root, errors)
    if '--require-all' in sys.argv:
        require_all(code, english_by_file, present, errors)
    if errors:
        for error in errors:
            print(error)
        return 1
    print('OK %s' % code)
    return 0


if __name__ == '__main__':
    sys.exit(main())
