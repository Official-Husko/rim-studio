#!/usr/bin/env python3
"""Scan a folder of RimWorld mods and report publish-relevant facts as JSON.

Usage: scan_mods.py MODS_ROOT OUT.json
For every immediate subfolder that has About/About.xml (case-insensitive) it records:
  packageId, name, supportedVersions, description length, About/Preview.png size and dimensions,
  About/PublishedFileId.txt content (digits only), total size and file count, and the size/count of
  entries matching a candidate 'never ship' list (top-level dirs and glob-like file patterns).
Deterministic: sorted traversal, no timestamps in the output.
"""
import json, os, re, struct, sys, fnmatch
from lxml import etree

NEVER_DIRS = {'.git', '.svn', '.hg', '.vs', '.idea', '.vscode', 'bin', 'obj', 'node_modules', '__pycache__'}
NEVER_TOP_NAMES = {'source', 'raw assets', 'rawassets', 'raw', 'src', 'psd', 'work', 'wip', 'backup', 'backups', 'docs', 'art', 'references'}
NEVER_FILE_GLOBS = ['*.psd', '*.ai', '*.xcf', '*.kra', '*.blend', '*.sln', '*.csproj', '*.pdb', '*.zip', '*.7z', '*.rar',
                    '*.bak', '*.tmp', '*.log', 'thumbs.db', '.ds_store', 'desktop.ini', '.gitignore', '.gitattributes', '*.md', '*.url', '*.user', '*.suo']

def png_info(path):
    try:
        with open(path, 'rb') as f:
            head = f.read(32)
        if head[:8] == b'\x89PNG\r\n\x1a\n' and head[12:16] == b'IHDR':
            w, h = struct.unpack('>II', head[16:24])
            return {'format': 'png', 'w': w, 'h': h}
        if head[:3] == b'\xff\xd8\xff':
            return {'format': 'jpeg-bytes'}
        if head[:6] in (b'GIF87a', b'GIF89a'):
            return {'format': 'gif-bytes'}
        return {'format': 'other'}
    except OSError as e:
        return {'format': 'unreadable', 'err': str(e)}

def find_ci(base, name):
    try:
        for e in os.listdir(base):
            if e.lower() == name.lower():
                return os.path.join(base, e)
    except OSError:
        pass
    return None

def parse_about(path):
    out = {}
    try:
        root = etree.parse(path, etree.XMLParser(recover=True, resolve_entities=False)).getroot()
    except Exception as e:
        return {'parse_error': str(e)}
    def txt(tag):
        n = root.find(tag)
        return (n.text or '').strip() if n is not None and n.text else ''
    out['packageId'] = txt('packageId')
    out['name'] = txt('name')
    out['author'] = txt('author')
    out['has_authors_list'] = root.find('authors') is not None
    out['description_len'] = len(txt('description'))
    sv = root.find('supportedVersions')
    out['supportedVersions'] = [ (li.text or '').strip() for li in sv.findall('li')] if sv is not None else None
    out['has_targetVersion'] = root.find('targetVersion') is not None
    deps = root.find('modDependencies')
    out['dep_count'] = len(deps.findall('li')) if deps is not None else 0
    out['has_descriptionsByVersion'] = root.find('descriptionsByVersion') is not None
    return out

PKG_RE = re.compile(r'(?=.{1,60}$)^(?!\.)(?=.*?[.])(?!.*([.])\1+)[a-zA-Z0-9.]{1,}[a-zA-Z0-9]{1}$')

def main():
    root, out = sys.argv[1], sys.argv[2]
    res = []
    for name in sorted(os.listdir(root)):
        base = os.path.join(root, name)
        if not os.path.isdir(base):
            continue
        about_dir = find_ci(base, 'About')
        if not about_dir:
            continue
        about_xml = find_ci(about_dir, 'About.xml')
        rec = {'folder': name, 'has_about_xml': bool(about_xml)}
        if about_xml:
            rec.update(parse_about(about_xml))
            rec['packageId_valid'] = bool(PKG_RE.match(rec.get('packageId', ''))) and 'ludeon' not in rec.get('packageId', '').lower()
        prev = os.path.join(about_dir, 'Preview.png')
        rec['preview_exact_case'] = os.path.isfile(prev)
        prev_any = find_ci(about_dir, 'Preview.png')
        if prev_any and os.path.isfile(prev_any):
            rec['preview_name_on_disk'] = os.path.basename(prev_any)
            rec['preview_bytes'] = os.path.getsize(prev_any)
            rec['preview'] = png_info(prev_any)
        pf = find_ci(about_dir, 'PublishedFileId.txt')
        if pf:
            try:
                rec['published_file_id_raw_len'] = os.path.getsize(pf)
                rec['published_file_id'] = open(pf, 'rb').read().decode('utf-8', 'replace').strip()
            except OSError:
                pass
        total = 0; files = 0
        flagged = {}
        flagged_bytes = 0; flagged_files = 0
        top_entries = []
        for dp, dns, fns in os.walk(base):
            dns.sort(); fns.sort()
            rel_dir = os.path.relpath(dp, base)
            parts = [] if rel_dir == '.' else rel_dir.split(os.sep)
            # prune never-dirs but account for their size
            keep = []
            for d in dns:
                low = d.lower()
                top = (len(parts) == 0)
                if low in NEVER_DIRS or (top and low in NEVER_TOP_NAMES):
                    sub_b = 0; sub_f = 0
                    for dp2, _d2, fn2 in os.walk(os.path.join(dp, d)):
                        for f2 in fn2:
                            try:
                                sub_b += os.path.getsize(os.path.join(dp2, f2)); sub_f += 1
                            except OSError:
                                pass
                    key = os.path.join(*(parts + [d])).replace(os.sep, '/')
                    flagged[key + '/'] = {'bytes': sub_b, 'files': sub_f}
                    flagged_bytes += sub_b; flagged_files += sub_f
                    total += sub_b; files += sub_f
                else:
                    keep.append(d)
            dns[:] = keep
            for f in fns:
                p = os.path.join(dp, f)
                try:
                    sz = os.path.getsize(p)
                except OSError:
                    continue
                total += sz; files += 1
                low = f.lower()
                if any(fnmatch.fnmatch(low, g) for g in NEVER_FILE_GLOBS) and not (low == 'preview.png'):
                    rel = os.path.join(*(parts + [f])).replace(os.sep, '/')
                    flagged[rel] = {'bytes': sz, 'files': 1}
                    flagged_bytes += sz; flagged_files += 1
        rec['total_bytes'] = total
        rec['total_files'] = files
        rec['flagged_bytes'] = flagged_bytes
        rec['flagged_files'] = flagged_files
        rec['flagged_top'] = dict(sorted(flagged.items(), key=lambda kv: -kv[1]['bytes'])[:12])
        rec['top_level'] = sorted(os.listdir(base))
        res.append(rec)
    json.dump(res, open(out, 'w'), indent=1, sort_keys=True)
    print('mods with About:', len(res))

if __name__ == '__main__':
    main()
