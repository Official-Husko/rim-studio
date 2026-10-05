#!/usr/bin/env python3
"""Build a realistic mod-record dataset for IPC and serialization benchmarks.

Source: the real About.xml files under the Steam workshop folder (read only).
Output: <out>/mods3000.json (full records) and <out>/mods5000.json.
Records are sampled with replacement from the real ones; duplicates get a unique
packageId suffix and workshop id so the strings are not identical, but the
length distribution, list sizes and field mix stay realistic.
"""
import json, os, random, sys
from lxml import etree

root = os.environ.get('RS_WORKSHOP_DIR') or sys.argv[2]  # steam workshop content/294100 folder (read only)
out = sys.argv[1]
os.makedirs(out, exist_ok=True)
rnd = random.Random(20261004)


def texts(el, tag):
    node = el.find(tag)
    if node is None:
        return []
    return [(li.text or '').strip() for li in node.findall('li') if (li.text or '').strip()]


def deps(el):
    node = el.find('modDependencies')
    res = []
    if node is None:
        return res
    for li in node.findall('li'):
        res.append({
            'packageId': (li.findtext('packageId') or '').strip().lower(),
            'displayName': (li.findtext('displayName') or '').strip(),
            'steamWorkshopUrl': (li.findtext('steamWorkshopUrl') or '').strip(),
            'downloadUrl': (li.findtext('downloadUrl') or '').strip(),
        })
    return res


base = []
for d in sorted(os.listdir(root)):
    p = os.path.join(root, d, 'About', 'About.xml')
    if not os.path.exists(p):
        continue
    try:
        t = etree.parse(p, etree.XMLParser(recover=True, huge_tree=True)).getroot()
    except Exception:
        continue
    if t is None:
        continue
    pid = (t.findtext('packageId') or '').strip().lower()
    if not pid:
        continue
    desc = t.findtext('description') or ''
    authors = texts(t, 'authors')
    base.append({
        'packageId': pid,
        'name': (t.findtext('name') or '').strip(),
        'author': (t.findtext('author') or '').strip() or (authors[0] if authors else ''),
        'authors': authors,
        'description': desc,
        'url': (t.findtext('url') or '').strip(),
        'modVersion': (t.findtext('modVersion') or '').strip(),
        'supportedVersions': texts(t, 'supportedVersions'),
        'modDependencies': deps(t),
        'loadBefore': [x.lower() for x in texts(t, 'loadBefore')],
        'loadAfter': [x.lower() for x in texts(t, 'loadAfter')],
        'forceLoadBefore': [x.lower() for x in texts(t, 'forceLoadBefore')],
        'forceLoadAfter': [x.lower() for x in texts(t, 'forceLoadAfter')],
        'incompatibleWith': [x.lower() for x in texts(t, 'incompatibleWith')],
        'workshopId': int(d) if d.isdigit() else 0,
        'dir': d,
    })

print('real About.xml records:', len(base), file=sys.stderr)
TAGS = ['Combat', 'Weapons', 'Apparel', 'Animals', 'Quality of Life', 'UI', 'Mechanoids', 'Factions', 'Biotech', 'Royalty',
        'Ideology', 'Anomaly', 'Odyssey', 'Furniture', 'Production', 'Research', 'Storage', 'Medical', 'Culture', 'Fix']


def make(n):
    recs = []
    for i in range(n):
        b = base[i % len(base)] if i < len(base) else rnd.choice(base)
        dup = i // len(base)
        r = dict(b)
        if i >= len(base):
            r['packageId'] = f"{b['packageId']}.v{dup}"
            r['name'] = f"{b['name']} {dup}"
            r['workshopId'] = b['workshopId'] + dup * 1000003
        wid = r['workshopId']
        r['id'] = i
        r['path'] = f"/home/user/.steam/steam/steamapps/workshop/content/294100/{wid}"
        r['source'] = 'workshop'
        r['sizeBytes'] = int(rnd.lognormvariate(16.0, 1.4))
        r['updatedUnix'] = 1_600_000_000 + rnd.randrange(0, 190_000_000)
        r['enabled'] = rnd.random() < 0.2
        r['loadIndex'] = None
        r['tags'] = rnd.sample(TAGS, rnd.randint(0, 4))
        r['hasPreview'] = rnd.random() < 0.97
        r['flags'] = rnd.randrange(0, 64)
        r['errors'] = 0 if rnd.random() < 0.9 else rnd.randint(1, 3)
        r['warnings'] = 0 if rnd.random() < 0.8 else rnd.randint(1, 5)
        del r['dir']
        recs.append(r)
    # assign load indexes to enabled mods
    k = 0
    for r in recs:
        if r['enabled']:
            r['loadIndex'] = k
            k += 1
    return recs


for n in (3000, 5000):
    recs = make(n)
    with open(os.path.join(out, f'mods{n}.json'), 'w') as f:
        json.dump(recs, f, ensure_ascii=False, separators=(',', ':'))
    sz = os.path.getsize(os.path.join(out, f'mods{n}.json'))
    print(f'mods{n}.json: {sz} bytes ({sz/1024:.0f} KiB), enabled={sum(1 for r in recs if r["enabled"])}', file=sys.stderr)
