import os, sys, collections
import vdf
acf = os.environ['ACF']  # path to appworkshop_294100.acf
content = os.environ['CONTENT']  # path to workshop/content/294100
d = vdf.load(open(acf, encoding='utf-8'))['AppWorkshop']
inst = d['WorkshopItemsInstalled']
det = d['WorkshopItemDetails']
disk = set(x for x in os.listdir(content))
print('installed entries', len(inst), 'details entries', len(det), 'folders on disk', len(disk))
print('installed not on disk:', sorted(set(inst) - disk))
print('on disk not in installed:', sorted(disk - set(inst)))
print('details not in installed:', sorted(set(det)-set(inst)), 'installed not in details:', sorted(set(inst)-set(det)))
# out-of-date detection
stale = []
for k, v in det.items():
    tu = int(v.get('timeupdated', 0)); ltu = int(v.get('latest_timeupdated', 0))
    m = v.get('manifest'); lm = v.get('latest_manifest')
    if tu != ltu or m != lm:
        stale.append((k, tu, ltu, m, lm))
print('items where timeupdated != latest_timeupdated or manifest != latest_manifest:', len(stale))
for s in stale[:10]: print(s)
# keys histogram in details
keys = collections.Counter()
for v in det.values():
    keys.update(v.keys())
print('detail keys histogram:', dict(keys))
keys2 = collections.Counter()
for v in inst.values(): keys2.update(v.keys())
print('installed keys histogram:', dict(keys2))
print('distinct subscribedby values:', len(set(v.get('subscribedby') for v in det.values())))
tt0 = sum(1 for v in det.values() if v.get('timetouched') == '0')
print('timetouched==0:', tt0)
# size consistency between installed.size and sum of file sizes on disk (sample)
import random
random.seed(1)
sample = random.sample(sorted(disk & set(inst)), 5)
for k in sample:
    total = 0
    for root, dirs, files in os.walk(os.path.join(content, k)):
        for f in files:
            try: total += os.path.getsize(os.path.join(root, f))
            except OSError: pass
    print('item', k, 'acf size', inst[k]['size'], 'disk size', total)
