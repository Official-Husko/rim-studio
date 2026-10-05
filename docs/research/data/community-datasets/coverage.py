#!/usr/bin/env python3
"""Measure community dataset coverage against a scanned mod library.

Usage: coverage.py --library library.json --rules communityRules.json --steamdb steamDB.json
       --uti replacements.json.gz --nvw ModIdsToFix.1.6.xml --modsconfig ModsConfig.xml [--game 1.6]
Prints one JSON object. Deterministic, offline, stdlib only (+ lxml for XML).
"""
import argparse, gzip, json, re, sys, collections
from lxml import etree

ap = argparse.ArgumentParser()
for a in ("library", "rules", "steamdb", "uti", "nvw", "modsconfig"):
    ap.add_argument("--" + a, required=True)
ap.add_argument("--game", default="1.6")
a = ap.parse_args()
L = json.load(open(a.library))["mods"]
CR = json.load(open(a.rules))["rules"]
SDB = json.load(open(a.steamdb))["database"]
UTI = json.loads(gzip.open(a.uti).read().decode("utf-8-sig"))["rules"]
NVW = {e.text.strip().lower() for e in etree.parse(a.nvw).getroot().iter("li") if e.text}
active = [e.text.strip().lower() for e in etree.parse(a.modsconfig).getroot().find("activeMods")]
act = set(active)
out = {"active_list_len": len(active), "active_distinct": len(act)}
inst = {}
for m in L:
    p = (m.get("packageId") or "").lower()
    if p:
        inst.setdefault(p, []).append(m)
out["library_mods"] = len(L)
out["library_distinct_ids"] = len(inst)
out["active_ids_found_in_library"] = len(act & set(inst))
out["active_ids_missing_from_library"] = len(act - set(inst))
# community rules
edges = []  # (kind, subj, target)
flags = collections.Counter()
for k, r in CR.items():
    s = k.lower()
    for kind in ("loadAfter", "loadBefore", "incompatibleWith"):
        for t, v in (r.get(kind) or {}).items():
            edges.append((kind, s, t.lower()))
    for kind in ("loadTop", "loadBottom"):
        if kind in r and s in act:
            flags[kind] += 1
for kind in ("loadAfter", "loadBefore", "incompatibleWith"):
    es = [e for e in edges if e[0] == kind]
    out["rules_" + kind] = {
        "total": len(es),
        "subject_active": sum(e[1] in act for e in es),
        "both_active": sum(e[1] in act and e[2] in act for e in es),
        "both_installed": sum(e[1] in inst and e[2] in inst for e in es),
    }
out["rules_loadTop_loadBottom_active_subjects"] = dict(flags)
# redundancy with About.xml effective rules (both active)
def about_edges(m):
    e = m["effective"]
    s = m["packageId"].lower()
    res = set()
    for t in e["loadAfter"] + e["forceLoadAfter"] + e["modDependencies"]:
        res.add(("after", s, (t["packageId"] if isinstance(t, dict) else t).lower()))
    for t in e["loadBefore"] + e["forceLoadBefore"]:
        res.add(("before", s, (t["packageId"] if isinstance(t, dict) else t).lower()))
    return res
AE = set()
for p in act:
    for m in inst.get(p, []):
        AE |= about_edges(m)
def norm(kind, s, t):  # to (earlier, later)
    return (t, s) if kind in ("loadAfter", "after") else (s, t)
aboutN = {norm(*e) for e in AE}
cr_active = [(k, s, t) for (k, s, t) in edges if k != "incompatibleWith" and s in act and t in act]
crN = {norm(*e) for e in cr_active}
out["order_edges_from_about_active"] = len(aboutN)
out["order_edges_from_community_both_active"] = len(crN)
out["community_edges_already_implied_by_about"] = len(crN & aboutN)
out["community_edges_adding_new_constraint"] = len(crN - aboutN)
out["community_edges_contradicting_about"] = len({(b, a_) for a_, b in aboutN} & crN)
# cycles in the community-only active graph and in union
def sccs(pairs):
    g = collections.defaultdict(list)
    for x, y in pairs:
        g[x].append(y); g.setdefault(y, [])
    idx, low, st, on, res, c = {}, {}, [], set(), [], [0]
    sys.setrecursionlimit(100000)
    def sc(v):
        idx[v] = low[v] = c[0]; c[0] += 1; st.append(v); on.add(v)
        for w in g[v]:
            if w not in idx: sc(w); low[v] = min(low[v], low[w])
            elif w in on: low[v] = min(low[v], idx[w])
        if low[v] == idx[v]:
            comp = []
            while True:
                w = st.pop(); on.discard(w); comp.append(w)
                if w == v: break
            if len(comp) > 1: res.append(sorted(comp))
    for v in list(g):
        if v not in idx: sc(v)
    return res
out["cycles_about_only_active"] = [len(c) for c in sccs(aboutN)]
out["cycles_community_only_active"] = [len(c) for c in sccs(crN)]
out["cycles_union_active"] = [len(c) for c in sccs(aboutN | crN)]
# incompatibilities among active
inc = [(s, t) for (k, s, t) in edges if k == "incompatibleWith" and s in act and t in act]
out["incompatible_pairs_both_active"] = len(inc)
# SteamDB agreement
agree = collections.Counter()
for m in L:
    if m["source"] != "workshop": continue
    wid = m.get("workshopId") or m["folder"]
    e = SDB.get(str(wid))
    if e is None: agree["workshop_folder_absent_from_steamdb"] += 1; continue
    agree["present"] += 1
    sp = (e.get("packageId") or e.get("packageid") or "").lower()
    ap_ = (m.get("packageId") or "").lower()
    if not sp: agree["steamdb_no_packageid"] += 1
    elif sp == ap_: agree["packageid_equal_case_insens"] += 1
    else: agree["packageid_differs"] += 1
    if sp and sp == ap_ and (e.get("packageId") or e.get("packageid")) != m.get("packageId"): agree["packageid_case_only_diff"] += 1
    gv = [x for x in (e.get("gameVersions") or []) if x]
    if gv:
        agree["steamdb_has_gameversions"] += 1
        sv = set(m.get("supportedVersions") or [])
        if set(gv) == sv: agree["gameversions_equal"] += 1
        elif a.game in gv and a.game not in sv: agree["steamdb_says_current_about_says_not"] += 1
        elif a.game in sv and a.game not in gv: agree["about_says_current_steamdb_says_not"] += 1
        else: agree["gameversions_differ_other"] += 1
    else: agree["steamdb_no_gameversions"] += 1
    if e.get("unpublished"): agree["steamdb_unpublished_flag"] += 1
    deps = e.get("dependencies") or {}
    aboutdeps = {d["packageId"].lower() if isinstance(d, dict) else d.lower() for d in m["effective"]["modDependencies"]}
    if deps: agree["steamdb_has_dependencies"] += 1
    if aboutdeps: agree["about_has_dependencies"] += 1
out["steamdb_vs_about"] = dict(agree)
# Use This Instead
byid = {r["oldWorkshopId"]: r for r in UTI}
byold = collections.defaultdict(list)
for r in UTI:
    if r["oldPackageId"]: byold[str(r["oldPackageId"]).lower()].append(r)
hit_w, hit_p = set(), set()
for m in L:
    if m["source"] != "workshop": continue
    wid = str(m.get("workshopId") or m["folder"])
    if wid in byid: hit_w.add(wid)
    if m["packageId"].lower() in byold: hit_p.add(m["packageId"].lower())
act_hit = set()
for m in L:
    if m["source"] == "workshop" and str(m.get("workshopId") or m["folder"]) in byid and m["packageId"].lower() in act:
        act_hit.add(str(m.get("workshopId") or m["folder"]))
rep_cur = [w for w in hit_w if a.game in byid[w]["newVersions"]]
new_installed = [w for w in hit_w if str(byid[w]["newWorkshopId"]) in {str(m.get("workshopId") or m["folder"]) for m in L if m["source"] == "workshop"}]
out["use_this_instead"] = {"rules": len(UTI), "installed_workshop_hits": len(hit_w), "hits_active": len(act_hit),
    "hits_with_replacement_supporting_game": len(rep_cur), "replacement_already_installed": len(new_installed),
    "by_old_packageid_hits": len(hit_p)}
# no version warning
nv_inst = [p for p in NVW if p in inst]
nv_act = [p for p in NVW if p in act]
nv_missing_cur = [p for p in nv_act if any(a.game not in (m.get("supportedVersions") or []) for m in inst[p])]
out["no_version_warning_1_6"] = {"entries": len(NVW), "installed": len(nv_inst), "active": len(nv_act), "active_not_declaring_game_version": len(nv_missing_cur)}
act_nodecl = [p for p in act if p in inst and any(a.game not in (m.get("supportedVersions") or []) for m in inst[p]) and inst[p][0]["source"] != "official"]
out["active_mods_not_declaring_game_version"] = len(act_nodecl)
out["active_not_declaring_and_not_in_nvw"] = len([p for p in act_nodecl if p not in NVW])
json.dump(out, sys.stdout, indent=1, sort_keys=True)
