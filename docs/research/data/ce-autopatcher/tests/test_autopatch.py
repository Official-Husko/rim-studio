import json, sys, unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import autopatch as AP

V = json.loads((HERE / "vectors.json").read_text())


class T(unittest.TestCase):
    def test_curves(self):
        for c in V["curves"]:
            self.assertAlmostEqual(AP.curve_eval(c["points"], c["x"]), c["expect"], places=9, msg=c["how"])

    def test_ranges(self):
        for r in V["ranges"]:
            self.assertEqual(AP.inc(r["range"], r["x"]), r["expect"], r["how"])

    def test_tools(self):
        for t in V["tools"]:
            got = AP.convert_tool(t["tool"])
            for k, v in t["expect"].items():
                self.assertAlmostEqual(got[k], v, msg=t["how"])

    def test_toughness(self):
        for t in V["toughness"]:
            got = AP.stuff_toughness_multiplier(t["bulk"], t["tech"], t["ranged"], t["capacities"])
            if t["expect"] is None:
                self.assertIsNone(got)
            else:
                self.assertAlmostEqual(got, t["expect"], msg=t["how"])

    def test_race(self):
        for r in V["race"]:
            s, b = AP.race_armor(r["sharp"], r["blunt"])
            self.assertAlmostEqual(s, r["expect"][0])
            self.assertAlmostEqual(b, r["expect"][1])

    def test_gun_classify(self):
        presets = V["gun_presets"]
        for c in V["gun_classify"]:
            p, why = AP.classify_gun(c["gun"], presets)
            self.assertEqual((p["defName"], why), (c["preset"], c["reason"]), c["how"])

    def test_gun_patch(self):
        presets = {p["defName"]: p for p in V["gun_presets"]}
        for c in V["gun_patch"]:
            got = AP.patch_gun(c["gun"], presets[c["preset"]])
            for k, v in c["expect"].items():
                if v is None:
                    self.assertIsNone(got[k], c["how"])
                elif isinstance(v, str):
                    self.assertEqual(got[k], v, c["how"])
                else:
                    self.assertAlmostEqual(got[k], v, places=9, msg=c["how"] + " " + k)

    def test_apparel(self):
        presets = V["apparel_presets"]
        for c in V["apparel"]:
            p = AP.classify_apparel(c["a"], presets)
            self.assertEqual(p["defName"] if p else None, c["preset"], c["how"])
            if p and "expect" in c:
                got = AP.patch_apparel(c["a"], p)
                for k, v in c["expect"].items():
                    self.assertAlmostEqual(got[k], v, places=9, msg=c["how"] + " " + k)


if __name__ == "__main__":
    unittest.main()
