using System;
using System.IO;
using System.Linq;
using System.Xml;
using System.Collections.Generic;

class Probe1
{
    static XmlDocument Load(string xml, bool ignoreWs = true)
    {
        var settings = new XmlReaderSettings { IgnoreComments = true, IgnoreWhitespace = ignoreWs, CheckCharacters = false };
        using (var sr = new StringReader(xml))
        using (var r = XmlReader.Create(sr, settings))
        {
            var d = new XmlDocument();
            d.Load(r);
            return d;
        }
    }
    static string Describe(XmlNode n)
    {
        var parts = new List<string>();
        foreach (XmlNode c in n.ChildNodes) parts.Add(c.NodeType + ":" + (c.NodeType == XmlNodeType.Element ? c.Name : "'" + c.Value + "'"));
        return string.Join(", ", parts);
    }
    static void Main()
    {
        Console.WriteLine("runtime: " + Environment.Version + " mono? " + (Type.GetType("Mono.Runtime") != null));
        // 1. whitespace handling
        string[] ws = {
            "<a> </a>",
            "<a>  <b/> </a>",
            "<a>x  <b/>  y</a>",
            "<a>&#32;</a>",
            "<a><![CDATA[ ]]></a>",
            "<a xml:space=\"preserve\"> <b/> </a>",
            "<a>\n\t  \n</a>",
            "<a> text </a>",
            "<a>one<!-- c -->two</a>",
            "<a><![CDATA[x]]></a>",
            "<a>x<![CDATA[y]]>z</a>",
        };
        foreach (var s in ws)
        {
            var d = Load(s);
            Console.WriteLine("WS " + s.Replace("\n", "\\n").Replace("\t", "\\t") + "  =>  [" + Describe(d.DocumentElement) + "]  HasChildNodes=" + d.DocumentElement.HasChildNodes);
        }
        // 2. Remove while enumerating
        {
            var d = Load("<r><a><x/><y/><z/></a></r>");
            var a = d.DocumentElement["a"];
            foreach (XmlNode c in a.ChildNodes) { if (c.NodeType != XmlNodeType.Attribute) a.RemoveChild(c); }
            Console.WriteLine("REMOVE-WHILE-ENUM leaves: [" + Describe(a) + "]");
            var d2 = Load("<r><a>hello</a></r>");
            var a2 = d2.DocumentElement["a"];
            foreach (XmlNode c in a2.ChildNodes) { a2.RemoveChild(c); }
            Console.WriteLine("REMOVE-WHILE-ENUM text only leaves: [" + Describe(a2) + "]");
        }
        // 3. Attributes.Append duplicates
        {
            var d = Load("<r a=\"1\" b=\"2\"/>");
            var at = d.CreateAttribute("a"); at.Value = "9";
            d.DocumentElement.Attributes.Append(at);
            Console.WriteLine("ATTR-APPEND-DUP: " + d.DocumentElement.OuterXml);
            var at2 = d.CreateAttribute("c"); at2.Value = "3";
            d.DocumentElement.Attributes.Append(at2);
            Console.WriteLine("ATTR-APPEND-NEW: " + d.DocumentElement.OuterXml);
            d.DocumentElement.Attributes.RemoveAll();
            Console.WriteLine("ATTR-REMOVEALL: " + d.DocumentElement.OuterXml);
        }
        // 4. indexer
        {
            var d = Load("<r><b>1</b><a>2</a><b>3</b></r>");
            Console.WriteLine("INDEXER first b: " + d.DocumentElement["b"].InnerText + "; missing: " + (d.DocumentElement["zzz"] == null));
        }
        // 5. xpath context + relative
        {
            var d = Load("<Defs><A id=\"1\"/><A id=\"2\"/></Defs>");
            Console.WriteLine("XPATH relative 'Defs/A' on doc: " + d.SelectNodes("Defs/A").Count + "; '/Defs/A': " + d.SelectNodes("/Defs/A").Count + "; 'A' on doc: " + d.SelectNodes("A").Count + "; on root element 'A': " + d.DocumentElement.SelectNodes("A").Count);
        }
        // 6. Insert append/prepend ordering with multiple children
        {
            var d = Load("<r><x/></r>");
            var x = d.DocumentElement["x"];
            var v = Load("<v><c1/><c2/><c3/></v>").DocumentElement;
            var parent = x.ParentNode;
            foreach (XmlNode c in v.ChildNodes) parent.InsertAfter(parent.OwnerDocument.ImportNode(c, true), x);
            Console.WriteLine("INSERT-AFTER multi: " + d.DocumentElement.OuterXml);
            var d3 = Load("<r><x/></r>");
            var x3 = d3.DocumentElement["x"];
            for (int i = v.ChildNodes.Count - 1; i >= 0; i--) x3.ParentNode.InsertBefore(d3.ImportNode(v.ChildNodes[i], true), x3);
            Console.WriteLine("INSERT-BEFORE multi (reverse loop): " + d3.DocumentElement.OuterXml);
        }
        // 7. lazy SelectNodes with mutation
        {
            var d = Load("<r><a/><a/><a/></r>");
            int n = 0;
            foreach (XmlNode a in d.SelectNodes("/r/a"))
            {
                n++;
                if (n > 20) break;
                a.ParentNode.AppendChild(d.CreateElement("a"));
            }
            Console.WriteLine("LAZY-SELECT appending <a/> while iterating /r/a: visited " + n + " final count " + d.SelectNodes("/r/a").Count);
        }
        // 8. SetName InnerXml roundtrip
        {
            var d = Load("<r><x Name=\"n\" a=\"1\">text<y z=\"2\"/>tail</x></r>");
            var x = d.DocumentElement["x"];
            var nn = d.CreateElement("w");
            nn.InnerXml = x.InnerXml;
            Console.WriteLine("SETNAME: " + nn.OuterXml);
        }
        // 9. text node xpath + Normalize
        {
            var d = Load("<r><x>aa</x></r>");
            foreach (XmlNode t in d.SelectNodes("/r/x/text()"))
            {
                var p = t.ParentNode;
                p.InsertBefore(d.CreateTextNode("PRE"), t);
                p.InsertAfter(d.CreateTextNode("POST"), t);
                Console.WriteLine("TEXT insert: nodes before normalize = " + p.ChildNodes.Count);
                p.Normalize();
                Console.WriteLine("TEXT insert: nodes after normalize = " + p.ChildNodes.Count + " -> " + p.OuterXml);
            }
        }
        // 10. Node.Name on text/xpath results with string() => exception?
        try { var d = Load("<r/>"); var l = d.SelectNodes("string(/r)"); Console.WriteLine("string() select ok " + l.Count); }
        catch (Exception e) { Console.WriteLine("SelectNodes(string(...)) throws " + e.GetType().Name + ": " + e.Message); }
        try { var d = Load("<r/>"); var l = d.SelectNodes(""); Console.WriteLine("empty select ok " + l.Count); }
        catch (Exception e) { Console.WriteLine("SelectNodes('') throws " + e.GetType().Name + ": " + e.Message); }
        try { var d = Load("<r a='1'/>"); foreach (XmlNode n in d.SelectNodes("/r/@a")) Console.WriteLine("attr node parent null? " + (n.ParentNode == null)); }
        catch (Exception e) { Console.WriteLine("attr select throws " + e.GetType().Name); }
    }
}
