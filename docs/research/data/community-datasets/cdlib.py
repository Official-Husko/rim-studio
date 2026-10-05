"""Shared helpers for the community-dataset analysis scripts.

Only the Python standard library plus orjson (optional, used when present).
Nothing here reads a path from the environment implicitly: every script takes
its inputs on the command line.
"""

import gzip
import json
import re
import statistics
import sys
from collections import Counter, defaultdict

try:  # orjson is only a speed-up; the stdlib parser is the fallback
    import orjson

    def fast_loads(data):
        return orjson.loads(data)

except ImportError:  # pragma: no cover

    def fast_loads(data):
        if isinstance(data, bytes):
            data = data.decode("utf-8-sig")
        return json.loads(data)


# The game's own packageId format check, re-expressed (decompiled:Verse/ModMetaData.cs):
# 1-60 chars, only ASCII letters, digits and dots, at least one dot, no leading dot,
# no run of dots, last character alphanumeric.
PACKAGE_ID_RE = re.compile(r"(?=.{1,60}$)^(?!\.)(?=.*?[.])(?!.*([.])\1+)[a-zA-Z0-9.]{1,}[a-zA-Z0-9]{1}$")


def read_bytes(path):
    """Read a file; transparently gunzip when the content starts with the gzip magic."""
    with open(path, "rb") as f:
        data = f.read()
    if data[:2] == b"\x1f\x8b":
        data = gzip.decompress(data)
    return data


def strip_bom(data):
    return data[3:] if data[:3] == b"\xef\xbb\xbf" else data


def load_json(path):
    """Parse a (possibly gzipped, possibly BOM-prefixed) JSON file."""
    data = strip_bom(read_bytes(path))
    return fast_loads(data)


class DuplicateKeyFinder:
    """object_pairs_hook that records duplicate keys per nesting path."""

    def __init__(self):
        self.duplicates = []  # (path_hint, key)

    def hook(self, pairs):
        seen = {}
        for k, v in pairs:
            if k in seen:
                self.duplicates.append(k)
            seen[k] = v
        return seen


def load_json_with_duplicates(path):
    """Parse with the stdlib parser and report every duplicated object key."""
    finder = DuplicateKeyFinder()
    data = strip_bom(read_bytes(path)).decode("utf-8")
    obj = json.loads(data, object_pairs_hook=finder.hook)
    return obj, finder.duplicates


def percentile(sorted_values, p):
    if not sorted_values:
        return None
    k = (len(sorted_values) - 1) * p / 100.0
    lo = int(k)
    hi = min(lo + 1, len(sorted_values) - 1)
    frac = k - lo
    return sorted_values[lo] + (sorted_values[hi] - sorted_values[lo]) * frac


def describe(values):
    """min / median / mean / p90 / p99 / max of a list of numbers."""
    if not values:
        return {"n": 0}
    s = sorted(values)
    return {
        "n": len(s),
        "min": s[0],
        "median": statistics.median(s),
        "mean": round(sum(s) / len(s), 3),
        "p90": percentile(s, 90),
        "p99": percentile(s, 99),
        "max": s[-1],
    }


def tarjan_scc(graph):
    """Iterative Tarjan SCC.  graph: dict node -> iterable of successor nodes.

    Returns a list of components (each a list of nodes).  Self loops are
    reported by the caller (a one-node component is only a cycle if it has a
    self edge).
    """
    index = {}
    low = {}
    on_stack = set()
    stack = []
    comps = []
    counter = [0]
    nodes = set(graph.keys())
    for succs in graph.values():
        nodes.update(succs)

    for root in sorted(nodes):
        if root in index:
            continue
        work = [(root, iter(sorted(graph.get(root, ()))))]
        index[root] = low[root] = counter[0]
        counter[0] += 1
        stack.append(root)
        on_stack.add(root)
        while work:
            node, it = work[-1]
            advanced = False
            for nxt in it:
                if nxt not in index:
                    index[nxt] = low[nxt] = counter[0]
                    counter[0] += 1
                    stack.append(nxt)
                    on_stack.add(nxt)
                    work.append((nxt, iter(sorted(graph.get(nxt, ())))))
                    advanced = True
                    break
                elif nxt in on_stack:
                    low[node] = min(low[node], index[nxt])
            if advanced:
                continue
            work.pop()
            if work:
                parent = work[-1][0]
                low[parent] = min(low[parent], low[node])
            if low[node] == index[node]:
                comp = []
                while True:
                    w = stack.pop()
                    on_stack.discard(w)
                    comp.append(w)
                    if w == node:
                        break
                comps.append(comp)
    return comps


def cyclic_components(graph):
    """Strongly connected components that really contain a cycle."""
    out = []
    for comp in tarjan_scc(graph):
        if len(comp) > 1:
            out.append(sorted(comp))
        else:
            n = comp[0]
            if n in graph.get(n, ()):
                out.append([n])
    return sorted(out, key=lambda c: (-len(c), c))


def norm_id(s):
    """Lower-case a packageId the way the game and RimSort compare them."""
    return s.lower() if isinstance(s, str) else ""


def eprint(*a):
    print(*a, file=sys.stderr)


def write_json(path, obj):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f, ensure_ascii=False, indent=2, sort_keys=True)
        f.write("\n")


__all__ = [
    "Counter",
    "defaultdict",
    "PACKAGE_ID_RE",
    "read_bytes",
    "strip_bom",
    "load_json",
    "load_json_with_duplicates",
    "describe",
    "percentile",
    "tarjan_scc",
    "cyclic_components",
    "norm_id",
    "eprint",
    "write_json",
    "fast_loads",
]
