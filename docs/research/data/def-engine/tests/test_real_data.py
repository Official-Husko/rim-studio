"""Re-computes golden/real_data_summary.json from the user's own installs and compares counts and hashes.

Environment: RIMWORLD_DIR (default: Linux Steam path), CE_DIR (CombatExtended source/mod folder) and CE_DLL
(compiled CombatExtended.dll, needs `monodis`). Each part is skipped when its inputs are missing.
"""
import json
import os
import subprocess
import sys
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
GAME = Path(os.environ.get("RIMWORLD_DIR", "/home/pawbeans/.steam/steam/steamapps/common/RimWorld"))
CE = os.environ.get("CE_DIR")
CE_DLL = os.environ.get("CE_DLL")


def run(*extra):
    out = subprocess.run([sys.executable, str(ROOT / "validate_real.py"), "--game", str(GAME), *extra],
                         capture_output=True, text=True, check=True, env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"))
    return json.loads(out.stdout)


@unittest.skipUnless((GAME / "Version.txt").is_file(), "RimWorld install not found")
class RealData(unittest.TestCase):
    golden = json.loads((ROOT / "golden" / "real_data_summary.json").read_text(encoding="utf-8"))

    def test_vanilla_matches_golden(self):
        got = run()["vanilla"]
        self.assertEqual(got, self.golden["vanilla"])
        self.assertEqual(got["diagnostics"], {})                 # the game logs no error for a vanilla load
        self.assertEqual(got["patch_ops_applied"], got["patch_ops_total"])

    @unittest.skipUnless(CE and CE_DLL, "set CE_DIR and CE_DLL to run the Combat Extended part")
    def test_ce_matches_golden(self):
        got = run("--ce", CE, "--ce-dll", CE_DLL)["vanilla_plus_ce"]
        self.assertEqual(got, self.golden["vanilla_plus_ce"])


if __name__ == "__main__":
    unittest.main()
