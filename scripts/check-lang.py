#!/usr/bin/env python3
"""Interface strings check (docs/decisions/language-packs.md).

Fails when:
- a UI file has more hard-coded English text than its entry in the
  baseline (scripts/lang-baseline.txt, "<count> <path>"); files not listed
  must have none;
- a key in lang/en/*.strings is not used anywhere in src/;
- a qll_* key repeats the English of an lng_* key (same text and plural
  forms, ignoring case and a trailing period). Telegram already translates
  lng_* in every language; a qll_* gets a translation only from a
  hand-written overlay. Reuse the lng_* key instead. Exempt are the
  duplicates that predate the check (scripts/lng-reuse-baseline.txt, may
  only shrink) and homonyms listed with a reason in
  scripts/lng-reuse-homonyms.txt; a stale entry in either fails;
- a status note (set_note / set_status_note / StatusNote::*) is built from a
  Rust Debug dump (`{err:?}`): map the error with user_error() instead.

Text is a string literal of at least two words that starts with a capital
letter, outside test files and the lines of logs, panics and attributes.
Test, demo and fixture files are skipped.

--update lowers the English baseline entries to the current counts, drops
the ones now at zero and drops lng-reuse baseline keys that no longer
repeat an lng_* text; it never adds or raises an entry.

Usage: python3 scripts/check-lang.py [--update]   (from the repository root)
"""
import os
import re
import sys

BASELINE = 'scripts/lang-baseline.txt'
LNG_BASELINE = 'scripts/lng-reuse-baseline.txt'
LNG_HOMONYMS = 'scripts/lng-reuse-homonyms.txt'
ENGLISH_PAIR = re.compile(
    r'^"((?:[^"\\]|\\.)*)"\s*=\s*"((?:[^"\\]|\\.)*)"\s*;'
)
SKIP_PATH = re.compile(r'test|demo|fixture')
SKIP_LINE = re.compile(
    r'^\s*(#\[|//)|\b(debug|info|warn|error|trace|log|eprintln|println|panic|unreachable|todo|assert\w*)!'
    r'|\.expect\(|\bQUILL_|concat!|include_str!'
)
LITERAL = re.compile(r'"((?:[^"\\]|\\.)*)"')
TEXT = re.compile(r"^[A-Z][a-z']+[,]? [A-Za-z'{}]+")


def ui_files():
    for root, _, names in os.walk('src/ui'):
        for name in sorted(names):
            path = os.path.join(root, name)
            if name.endswith('.rs') and not SKIP_PATH.search(path):
                yield path


# A literal handed straight to a text-taking call or a status note, even a
# single lowercase word ("pinned"): it shows on screen, so it needs a key.
DIRECT = re.compile(
    r'(?:\.(?:child|label|tooltip|placeholder|aria_label|accessibility_label|title|description)'
    r'|StatusNote::(?:info|error|silent))\(\s*"((?:[^"\\]|\\.)*)"\s*\)'
)
# Names and symbols that are the same in every language.
DIRECT_OK = {'GIF', 'HDR', 'Quill', 't.me/'}
DIRECT_WORD = re.compile(r'[A-Za-z]{2,}')


def direct_count(line):
    """Direct text literals the capitalised-phrase rule does not already count."""
    return sum(
        1
        for lit in DIRECT.findall(line)
        if lit not in DIRECT_OK
        and DIRECT_WORD.search(re.sub(r'\\u\{[0-9A-Fa-f]+\}', '', lit))
        and '::' not in lit
        and not lit.isupper()
        and not TEXT.match(lit)
    )


def english_count(path):
    count = 0
    in_tests = False
    # Join Rust string literals split over lines (`"first \
    # second"`): the line scanner used to miss them entirely.
    text = re.sub(r'\\\n[ \t]*', ' ', open(path, encoding='utf-8').read())
    for line in text.splitlines():
        if line.startswith('#[cfg(test)]'):
            in_tests = True
        if in_tests or SKIP_LINE.search(line):
            continue
        for lit in LITERAL.findall(line):
            if TEXT.match(lit) and '::' not in lit:
                count += 1
        count += direct_count(line)
    return count


def load_baseline():
    out = {}
    if os.path.exists(BASELINE):
        for line in open(BASELINE, encoding='utf-8'):
            parts = line.split()
            if len(parts) == 2 and not line.startswith('#'):
                out[parts[1]] = int(parts[0])
    return out


def english_values():
    """Base key -> {form or '': value} from every lang/en file."""
    out = {}
    for name in sorted(os.listdir('lang/en')):
        if not name.endswith('.strings'):
            continue
        path = os.path.join('lang/en', name)
        for line in open(path, encoding='utf-8'):
            match = ENGLISH_PAIR.match(line.strip())
            if not match:
                continue
            key, value = match.groups()
            base, _, form = key.partition('#')
            out.setdefault(base, {})[form] = value
    return out


def same_text(value):
    """Comparison form: case and one trailing period don't make new text."""
    text = value.strip().casefold()
    return text[:-1] if text.endswith('.') else text


def text_shape(forms):
    return tuple(sorted((form, same_text(value)) for form, value in forms.items()))


def lng_duplicates(values):
    """qll_* key -> the lng_* keys whose English and forms it repeats."""
    by_text = {}
    for key, forms in values.items():
        if key.startswith('lng_'):
            by_text.setdefault(text_shape(forms), []).append(key)
    out = {}
    for key, forms in values.items():
        if key.startswith('qll_'):
            match = by_text.get(text_shape(forms))
            if match:
                out[key] = sorted(match)
    return out


def read_list(path):
    if not os.path.exists(path):
        return []
    lines = (line.strip() for line in open(path, encoding='utf-8'))
    return [line for line in lines if line and not line.startswith('#')]


def parse_homonyms(lines):
    """`qll_key reason...` lines -> ({key: reason}, problems)."""
    out, problems = {}, []
    for line in lines:
        key, reason = (line.split(None, 1) + [''])[:2]
        if not reason.strip():
            problems.append('LNG-HOMONYM %s: give the reason its meaning differs from the lng_* text (%s).' % (key, LNG_HOMONYMS))
        if key in out:
            problems.append('LNG-HOMONYM %s: listed twice in %s; keep one line.' % (key, LNG_HOMONYMS))
        out[key] = reason.strip()
    return out, problems


def lng_reuse_problems(values, baseline, homonyms):
    """Messages for qll_* keys that repeat an lng_* text, plus the baseline
    entries that are still duplicates (what --update keeps)."""
    duplicates = lng_duplicates(values)
    problems = []
    for key, lngs in sorted(duplicates.items()):
        if key in baseline or key in homonyms:
            continue
        forms = values[key]
        text = forms.get('') or forms.get('other') or next(iter(forms.values()))
        problems.append(
            'LNG-REUSE %s = "%s" repeats %s: use the lng_* key with the same meaning '
            '(Telegram translates it into every language) and remove %s. '
            'If no meaning matches, list %s with the reason in %s.'
            % (key, text, ', '.join(lngs), key, key, LNG_HOMONYMS))
    for key in sorted(set(baseline) - set(duplicates)):
        problems.append('LNG-REUSE-STALE %s: no longer repeats an lng_* text; run --update to shrink %s.' % (key, LNG_BASELINE))
    for key in sorted(set(homonyms) - set(duplicates)):
        problems.append('LNG-HOMONYM-STALE %s: no longer repeats an lng_* text; remove it from %s.' % (key, LNG_HOMONYMS))
    return problems, set(baseline) & set(duplicates)


def write_lng_baseline(keys):
    lines = [
        '# qll_* keys whose English already equals an lng_* key (case and a',
        '# trailing period ignored). Telegram translates lng_*; a qll_* only gets',
        '# a hand-written overlay. Reuse the lng_* key. This baseline predates the',
        '# check and may only shrink (run --update). A real homonym goes in',
        '# scripts/lng-reuse-homonyms.txt with its reason instead.',
    ]
    lines.extend(sorted(keys))
    open(LNG_BASELINE, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')


def unused_keys():
    used = set()
    for root, _, names in os.walk('src'):
        for name in names:
            if name.endswith('.rs'):
                text = open(os.path.join(root, name), encoding='utf-8').read()
                used.update(re.findall(r'keys::([A-Z0-9_]+)', text))
    out = []
    for name in sorted(os.listdir('lang/en')):
        if not name.endswith('.strings'):
            continue
        for key in re.findall(r'^"([a-z0-9_]+)(?:#[a-z]+)?"\s*=', open(os.path.join('lang/en', name), encoding='utf-8').read(), re.M):
            const = 'Q_' + key[4:].upper() if key.startswith('qll_') else key[4:].upper()
            if const not in used:
                out.append('%s (lang/en/%s)' % (key, name))
    return sorted(set(out))


NOTE_START = re.compile(r'\b(?:set_note|set_status_note|StatusNote::\w+)\(')
DEBUG_DUMP = re.compile(r'\{[^{}]*:#?\?\}')


def debug_dump_notes(path):
    """Statements that build a status note from a Debug format."""
    text = open(path, encoding='utf-8').read()
    cut = text.find('#[cfg(test)]')
    if cut >= 0:
        text = text[:cut]
    out = []
    for m in NOTE_START.finditer(text):
        end = text.find(';', m.end())
        statement = text[m.start():end if end >= 0 else len(text)]
        if DEBUG_DUMP.search(statement):
            out.append(text.count('\n', 0, m.start()) + 1)
    return sorted(set(out))


def main():
    update = '--update' in sys.argv
    baseline = load_baseline()
    counts = {path: english_count(path) for path in ui_files()}
    failed = False
    for path, count in sorted(counts.items()):
        allowed = baseline.get(path, 0)
        if count > allowed:
            failed = True
            print('ENGLISH %s: %d hard-coded strings (allowed %d). Use tr(keys::…) and add the text to lang/en/*.strings.' % (path, count, allowed))
    if update:
        lines = ['# Hard-coded English strings per UI file (scripts/check-lang.py). May only shrink.']
        for path in sorted(baseline):
            count = min(counts.get(path, 0), baseline[path])
            if count:
                lines.append('%d %s' % (count, path))
        open(BASELINE, 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
    homonyms, problems = parse_homonyms(read_list(LNG_HOMONYMS))
    reuse, kept = lng_reuse_problems(english_values(), set(read_list(LNG_BASELINE)), homonyms)
    if update:
        write_lng_baseline(kept)
        reuse = [line for line in reuse if not line.startswith('LNG-REUSE-STALE')]
    for line in problems + reuse:
        failed = True
        print(line)
    for path in ui_files():
        for line in debug_dump_notes(path):
            failed = True
            print('DEBUG-NOTE %s:%d: a status note shows a Debug dump. Use user_error(&err) (src/ui/user_error.rs).' % (path, line))
    for key in unused_keys():
        failed = True
        print('UNUSED %s: no keys:: use in src/. Remove it.' % key)
    if failed:
        sys.exit(1)
    print('interface strings OK')


if __name__ == '__main__':
    main()
