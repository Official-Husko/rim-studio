#!/usr/bin/env python3
"""Census of scan pitfalls below one or more roots: depth, long paths, non-UTF-8 names, symlinks,
permission errors, huge XML files, BOMs, empty XML, case variants of Defs/About. Usage: pitfalls.py ROOT..."""
import os, sys, collections
sys.dont_write_bytecode = True
for root in sys.argv[1:]:
    rootb = os.fsencode(root)
    st = collections.Counter(); big = []; maxdepth = (0, b''); longest = (0, b''); bom = 0; nonutf = []
    for dp, dns, fns in os.walk(rootb, onerror=lambda e: st.update(["walk_errors"])):
        depth = dp.count(b'/') - rootb.count(b'/')
        if depth > maxdepth[0]: maxdepth = (depth, dp)
        for n in dns + fns:
            try: n.decode('utf-8')
            except UnicodeDecodeError: nonutf.append(os.path.join(dp, n))
        for d in list(dns):
            if os.path.islink(os.path.join(dp, d)): st['symlink_dirs'] += 1
            if d.lower() in (b'defs', b'about') and d not in (b'Defs', b'About'): st['case_variant_' + d.decode('utf8','replace')] += 1
        for f in fns:
            p = os.path.join(dp, f); st['files'] += 1
            if len(p) > longest[0]: longest = (len(p), p)
            if os.path.islink(p): st['symlink_files'] += 1; continue
            if f.lower().endswith(b'.xml'):
                try:
                    s = os.stat(p).st_size; st['xml'] += 1
                    if s == 0: st['xml_empty'] += 1
                    if s > 1_000_000: big.append((s, p))
                    with open(p, 'rb') as fh:
                        h = fh.read(4)
                        if h.startswith(b'\xef\xbb\xbf'): bom += 1
                        elif h[:2] in (b'\xff\xfe', b'\xfe\xff'): st['xml_utf16_bom'] += 1
                except PermissionError: st['permission_denied'] += 1
                except OSError: st['io_error'] += 1
    big.sort(reverse=True)
    print('ROOT', root); print(' ', dict(st), 'xml_utf8_bom', bom, 'nonutf8_names', len(nonutf))
    print('  max depth', maxdepth[0], 'longest path chars', longest[0], 'xml>1MB', len(big), 'top sizes', [s for s, _ in big[:5]])
    for s, p in big[:3]: print('   ', s, os.fsdecode(p)[len(root):][:140])
    for p in nonutf[:3]: print('  nonutf8', os.fsdecode(p)[len(root):][:100])
