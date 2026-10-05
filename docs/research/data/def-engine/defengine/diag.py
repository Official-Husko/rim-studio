"""Diagnostics: the equivalent of the lines the game writes to Player.log, but structured.

Every message has a severity (error / warning / info), a stable `code` and an optional
mod and file. Counts are kept per code; the first `keep` full messages per code are
retained so that a 15k-def load does not hold a million strings.
"""
from __future__ import annotations

from collections import Counter, defaultdict
from dataclasses import dataclass
from typing import Dict, List, Optional


@dataclass
class Diag:
    level: str
    code: str
    message: str
    mod: Optional[str] = None
    file: Optional[str] = None

    def __str__(self):
        where = ""
        if self.mod or self.file:
            where = " [%s%s]" % (self.mod or "", (":" + self.file) if self.file else "")
        return "%s %s%s: %s" % (self.level.upper(), self.code, where, self.message)


class Diagnostics:
    def __init__(self, keep: int = 100):
        self.keep = keep
        self.counts: Counter = Counter()
        self.levels: Dict[str, str] = {}
        self.samples: Dict[str, List[Diag]] = defaultdict(list)

    def add(self, level: str, code: str, message: str, mod: Optional[str] = None, file: Optional[str] = None) -> None:
        self.counts[code] += 1
        self.levels[code] = level
        lst = self.samples[code]
        if len(lst) < self.keep:
            lst.append(Diag(level, code, message, mod, file))

    def count(self, code: str) -> int:
        return self.counts.get(code, 0)

    def summary(self) -> Dict[str, Dict[str, object]]:
        return {c: {"level": self.levels[c], "count": n} for c, n in sorted(self.counts.items())}

    def merge(self, other: "Diagnostics") -> None:
        for c, n in other.counts.items():
            self.counts[c] += n
            self.levels[c] = other.levels[c]
            room = self.keep - len(self.samples[c])
            if room > 0:
                self.samples[c].extend(other.samples[c][:room])
