import os
import sys
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from lxml import etree                                           # noqa: E402

from defengine.mayrequire import prune_li                        # noqa: E402
from defengine.mods import ActiveSet, parse_load_folders, version_from_string  # noqa: E402
from defengine.xmlnet import XmlParseError, parse_xml_bytes, to_canonical     # noqa: E402
from defengine import xpath_net                                   # noqa: E402


class XmlLayer(unittest.TestCase):
    def test_bom_comments_and_whitespace(self):
        el = parse_xml_bytes(b"\xef\xbb\xbf<Defs>\n <!-- c -->\n <A> x </A>\n <B>\n</B></Defs>")
        self.assertEqual(to_canonical(el), {"tag": "Defs", "attrs": [], "children": [
            {"tag": "A", "attrs": [], "children": [" x "]}, {"tag": "B", "attrs": [], "children": []}]})

    def test_dtd_is_rejected(self):
        with self.assertRaises(XmlParseError):
            parse_xml_bytes(b'<!DOCTYPE a [<!ENTITY e "x">]><a>&e;</a>')

    def test_encoding_declaration_is_ignored(self):
        el = parse_xml_bytes('<?xml version="1.0" encoding="ISO-8859-1"?><a>é</a>'.encode("utf-8"))
        self.assertEqual(el.text, "é")


class XPathContext(unittest.TestCase):
    def setUp(self):
        self.root = parse_xml_bytes(b'<Defs><A><n>1</n></A><A><n>2</n></A></Defs>')

    def test_relative_path_starts_at_the_document(self):
        self.assertEqual(len(xpath_net.select_nodes(self.root, "Defs/A")), 2)
        self.assertEqual(len(xpath_net.select_nodes(self.root, "/Defs/A")), 2)

    def test_predicates_keep_their_own_context(self):
        self.assertEqual(len(xpath_net.select_nodes(self.root, 'Defs/A[n="2"]')), 1)
        self.assertEqual(len(xpath_net.select_nodes(self.root, "//n[text()='1']")), 1)

    def test_non_node_set_is_an_error(self):
        with self.assertRaises(xpath_net.XPathError):
            xpath_net.select_nodes(self.root, "count(//A)")
        with self.assertRaises(xpath_net.XPathError):
            xpath_net.select_nodes(self.root, "")


class ModsAndVersions(unittest.TestCase):
    def test_load_folders_keys(self):
        lf = parse_load_folders(parse_xml_bytes(
            b'<loadFolders><v1.5><li>1.5</li></v1.5><V1.6><li IfModActive="A.b, c.d">X</li><li>/</li></V1.6><default><li>Y\\Z</li></default></loadFolders>'))
        self.assertEqual(lf.defined_versions(), ["1.5", "1.6", "default"])
        x, root = lf.folders_for_version("1.6")
        self.assertEqual((x.folder_name, x.required_any_of, root.folder_name), ("X", ["A.b", "c.d"], ""))
        self.assertEqual(lf.folders_for_version("default")[0].folder_name, "Y/Z")

    def test_active_set(self):
        a = ActiveSet(["Ludeon.RimWorld", " Foo.Bar "], ["Core"])
        self.assertTrue(a.all_mods_active_no_suffix(["ludeon.rimworld", "FOO.BAR"]))
        self.assertFalse(a.any_mod_active_no_suffix(["", "nope"]))
        self.assertTrue(a.has_active_mod_with_name("Core"))
        self.assertFalse(a.has_active_mod_with_name("core"))

    def test_version_ordering(self):
        self.assertLess(version_from_string("1.5"), version_from_string("1.6"))


class MayRequireLists(unittest.TestCase):
    def test_prune_li(self):
        a = ActiveSet(["x.y"])
        n = parse_xml_bytes(b'<d><l><li>keep</li><li MayRequire="a.b">drop</li><li MayRequire="X.Y">keep2</li>'
                            b'<li MayRequireAnyOf="a.b,c.d"><li>nested</li></li><li MayRequire="">empty-keeps</li></l></d>')
        self.assertEqual(prune_li(n, a), 2)
        self.assertEqual([e.text for e in n.iter("li")], ["keep", "keep2", "empty-keeps"])


if __name__ == "__main__":
    unittest.main()
