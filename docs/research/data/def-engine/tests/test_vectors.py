import os
import sys
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import harness  # noqa: E402


class VectorTests(unittest.TestCase):
    pass


def _make(vec):
    def t(self):
        fails = harness.check_vector(vec)
        self.assertEqual(fails, [], "\n".join(fails))
    return t


for _v in harness.load_vectors():
    setattr(VectorTests, "test_" + _v["name"].replace("-", "_"), _make(_v))

if __name__ == "__main__":
    unittest.main()
