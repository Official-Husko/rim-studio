#!/usr/bin/env python3
"""Census of publish-unfriendly content inside a folder of RimWorld mods.

Usage: junk_census.py MODS_ROOT OUT.json
For each mod folder with About/ it counts, per category, whether the category is present and how many bytes/files it holds.
Categories are by NAME only (no content inspection). Deterministic output (sorted keys, no timestamps).
"""
import json, os, sys, fnmatch, collections

DIR_CATS = {
    'vcs_dir': {'.git', '.svn', '.hg'},
    'ide_dir': {'.vs', '.idea', '.vscode'},
    'build_dir': {'bin', 'obj'},
    'node_dir': {'node_modules', '__pycache__'},
}
TOP_DIR_CATS = {'source_dir': {'source', 'sources', 'src'}, 'raw_art_dir': {'raw assets', 'rawassets', 'raw', 'psd', 'wip', 'work'}}
FILE_CATS = {
    'pdb': ['*.pdb'],
    'project_files': ['*.sln', '*.csproj', '*.user', '*.suo', '*.vbproj'],
    'layered_art': ['*.psd', '*.xcf', '*.ai', '*.kra', '*.blend', '*.clip', '*.sai', '*.afphoto'],
    'archives': ['*.zip', '*.rar', '*.7z', '*.tar', '*.gz'],
    'backup_tmp': ['*.bak', '*.tmp', '*.orig', '*~', '*.swp'],
    'os_junk': ['thumbs.db', '.ds_store', 'desktop.ini'],
    'vcs_files': ['.gitignore', '.gitattributes', '.gitmodules', '.editorconfig'],
    'logs': ['*.log'],
    'markdown_docs': ['*.md'],
}

def main():
    root, out = sys.argv[1], sys.argv[2]
    mods = []
    for name in sorted(os.listdir(root)):
        base = os.path.join(root, name)
        if not os.path.isdir(base):
            continue
        if not any(e.lower() == 'about' for e in os.listdir(base)):
            continue
        mods.append((name, base))
    present = collections.Counter(); bytes_by = collections.Counter(); files_by = collections.Counter()
    totals = []
    for name, base in mods:
        seen = set(); mod_total = 0; mod_files = 0
        for dp, dns, fns in os.walk(base):
            dns.sort(); fns.sort()
            rel = os.path.relpath(dp, base)
            depth0 = rel == '.'
            keep = []
            for d in dns:
                low = d.lower()
                cat = None
                for c, names in DIR_CATS.items():
                    if low in names: cat = c
                if depth0:
                    for c, names in TOP_DIR_CATS.items():
                        if low in names: cat = c
                if cat:
                    sb = 0; sf = 0
                    for dp2, _x, fn2 in os.walk(os.path.join(dp, d)):
                        for f2 in fn2:
                            try: sb += os.path.getsize(os.path.join(dp2, f2)); sf += 1
                            except OSError: pass
                    seen.add(cat); bytes_by[cat] += sb; files_by[cat] += sf
                    mod_total += sb; mod_files += sf
                else:
                    keep.append(d)
            dns[:] = keep
            for f in fns:
                try: sz = os.path.getsize(os.path.join(dp, f))
                except OSError: continue
                mod_total += sz; mod_files += 1
                low = f.lower()
                for c, globs in FILE_CATS.items():
                    if any(fnmatch.fnmatch(low, g) for g in globs):
                        seen.add(c); bytes_by[c] += sz; files_by[c] += 1
        for c in seen: present[c] += 1
        totals.append((mod_total, mod_files, name))
    totals.sort(reverse=True)
    sizes = sorted(t[0] for t in totals)
    def pct(p): return sizes[min(len(sizes)-1, int(p*len(sizes)))]
    res = {
        'mods': len(mods),
        'present_in_mods': dict(sorted(present.items())),
        'bytes_by_category': dict(sorted(bytes_by.items())),
        'files_by_category': dict(sorted(files_by.items())),
        'size_percentiles_bytes': {'p50': pct(.5), 'p90': pct(.9), 'p99': pct(.99), 'max': sizes[-1]},
        'largest': [{'folder': n, 'bytes': b, 'files': f} for b, f, n in totals[:8]],
        'over_100MB': sum(1 for s in sizes if s > 100*1048576),
        'over_1GiB': sum(1 for s in sizes if s > 1024*1048576),
        'file_count_max': max(t[1] for t in totals),
        'over_10k_files': sum(1 for t in totals if t[1] > 10000),
    }
    json.dump(res, open(out, 'w'), indent=1, sort_keys=True)
    print(json.dumps(res, indent=1, sort_keys=True))

if __name__ == '__main__':
    main()
