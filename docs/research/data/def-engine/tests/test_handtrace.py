"""Hand-traced expectations for vanilla ThingDefs (see handtrace_vanilla.json) against the engine.

Needs a RimWorld install: set RIMWORLD_DIR (default: the Linux Steam path). Skipped when absent.
Every expectation was derived by hand from the raw XML of the def and its parent chain (the merge rules of
section 6 of the semantics note), not copied from engine output.
"""
import json
import os
import sys
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from defengine.engine import LoadConfig, load_game          # noqa: E402

GAME = Path(os.environ.get("RIMWORLD_DIR", "/home/pawbeans/.steam/steam/steamapps/common/RimWorld"))
CORE_DIRS = "Core Royalty Ideology Biotech Anomaly Odyssey".split()


@unittest.skipUnless((GAME / "Version.txt").is_file(), "RimWorld install not found (set RIMWORLD_DIR)")
class HandTrace(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.res = load_game(LoadConfig(mods=[GAME / "Data" / d for d in CORE_DIRS], game_dir=GAME,
                                       types=HERE.parent / "data" / "def_types_vanilla.json"))

    def test_all_hand_traces(self):
        traces = json.loads((HERE / "handtrace_vanilla.json").read_text(encoding="utf-8"))
        self.assertGreaterEqual(len(traces), 12)
        for t in traces:
            typ, name = t["def"].split("/", 1)
            d = self.res.get(typ, name)
            self.assertIsNotNone(d, t["def"])
            self.assertEqual(d.mod_id, t["mod"], t["def"])
            if t.get("parents") is not None:
                self.assertEqual(d.parents, t["parents"], t["def"])
            for c in t["checks"]:
                with self.subTest(t["def"], check=c):
                    if "text" in c:
                        el = d.node.find(c["text"])
                        self.assertIsNotNone(el)
                        self.assertEqual("".join(el.itertext()), c["expect"])
                    elif "count" in c:
                        self.assertEqual(int(d.node.xpath("count(%s)" % c["count"])), c["expect"])
                    elif "texts" in c:
                        self.assertEqual(["".join(e.itertext()) for e in d.node.xpath(c["texts"])], c["expect"])
                    elif "names" in c:
                        self.assertEqual([e.tag for e in d.node.xpath(c["names"])], c["expect"])
                    elif "attrs" in c:
                        for k, v in c["attrs"].items():
                            self.assertEqual(d.node.get(k), v)


if __name__ == "__main__":
    unittest.main()
