#!/usr/bin/env python3
"""List every @tr("…") string in ui/**/*.slint (singular and plural forms).

    tools/extract-tr.py            print them
    tools/extract-tr.py --check    fail if i18n/ru/LC_MESSAGES/rivet.po misses one
"""
import pathlib, re, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
STR = r'"((?:[^"\\]|\\.)*)"'
SINGLE = re.compile(r'@tr\(\s*' + STR + r'\s*[,)]')
PLURAL = re.compile(r'@tr\(\s*' + STR + r'\s*\|\s*' + STR + r'\s*%')

def strings():
    found = {}
    for f in sorted((ROOT / 'ui').rglob('*.slint')):
        text = f.read_text()
        for m in PLURAL.finditer(text):
            found[m.group(1)] = m.group(2)
        for m in SINGLE.finditer(text):
            found.setdefault(m.group(1), None)
    return found

def po_ids(path):
    ids = set()
    for m in re.finditer(r'^msgid "((?:[^"\\]|\\.)*)"', path.read_text(), re.M):
        ids.add(m.group(1))
    return ids

if __name__ == '__main__':
    found = strings()
    if '--check' in sys.argv:
        have = po_ids(ROOT / 'i18n/ru/LC_MESSAGES/rivet.po')
        missing = [s for s in found if s not in have]
        for s in missing:
            print('missing:', s)
        sys.exit(1 if missing else 0)
    for s, plural in found.items():
        print(s + ('   ||   ' + plural if plural else ''))
