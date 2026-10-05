"""Scan About.xml of mods in several roots and report duplicate packageIds (read-only research script).
Usage: scan_packageids.py LABEL=PATH [LABEL=PATH ...] ; env MAXDEPTH (default 1) = how deep below PATH to look for About/About.xml.
Deterministic output (sorted)."""
import os, sys, collections, re
from lxml import etree

maxdepth = int(os.environ.get('MAXDEPTH', '1'))

def find_about(root, depth):
    """Yield mod roots (dirs containing About/About.xml, case-insensitive) up to depth levels below root."""
    out = []
    def has_about(d):
        try:
            names = {n.lower(): n for n in os.listdir(d)}
        except OSError:
            return None
        if 'about' in names:
            ad = os.path.join(d, names['about'])
            try:
                inner = {n.lower(): n for n in os.listdir(ad)}
            except OSError:
                return None
            if 'about.xml' in inner:
                return os.path.join(ad, inner['about.xml'])
        return None
    def rec(d, lvl):
        a = has_about(d)
        if a:
            out.append((d, a)); return
        if lvl >= depth: return
        try:
            subs = sorted(e.path for e in os.scandir(d) if e.is_dir(follow_symlinks=True))
        except OSError:
            return
        for s in subs: rec(s, lvl + 1)
    try:
        subs = sorted(e.path for e in os.scandir(root) if e.is_dir(follow_symlinks=True))
    except OSError:
        return out
    for s in subs: rec(s, 1)
    return out

def pkg(about):
    try:
        raw = open(about, 'rb').read()
        parser = etree.XMLParser(recover=True, resolve_entities=False, no_network=True)
        t = etree.fromstring(raw, parser)
        if t is None: return None, None
        pid = t.findtext('packageId')
        name = t.findtext('name')
        return (pid or '').strip(), (name or '').strip()
    except Exception as e:
        return None, None

rows = []
for arg in sys.argv[1:]:
    label, path = arg.split('=', 1)
    mods = find_about(path, maxdepth)
    for d, a in mods:
        pid, name = pkg(a)
        rows.append((label, d, pid, name))
    print(f'{label}: {len(mods)} mod roots found under {path} (maxdepth={maxdepth})')

by = collections.defaultdict(list)
for label, d, pid, name in rows:
    key = (pid or '').lower()
    by[key].append((label, d))
dups = {k: v for k, v in by.items() if len(v) > 1 and k}
print('missing packageId:', sum(1 for r in rows if not r[2]))
print('distinct packageIds:', len([k for k in by if k]))
print('duplicate packageIds (any root):', len(dups))
cross = {k: v for k, v in dups.items() if len({l for l, _ in v}) > 1}
print('duplicates spanning more than one root:', len(cross))
for k in sorted(cross)[:15]:
    print('  ', k, '->', [(l, os.path.basename(d)) for l, d in cross[k]])
within = {k: v for k, v in dups.items() if len({l for l, _ in v}) == 1}
print('duplicates within a single root:', len(within))
for k in sorted(within)[:15]:
    print('  ', k, '->', [(l, os.path.basename(d)) for l, d in within[k]])
