using System.Reflection;
using System.Text.Json;

// Feasibility probe: how much of the XML schema can be derived from the game assembly by reflection alone?
// Reads Assembly-CSharp.dll via MetadataLoadContext (no code is executed). Output is statistics only.
var managed = args.Length > 0 ? args[0] : "/home/pawbeans/.steam/steam/steamapps/common/RimWorld/RimWorldLinux_Data/Managed";
var paths = Directory.GetFiles(managed, "*.dll");
var resolver = new PathAssemblyResolver(paths);
using var mlc = new MetadataLoadContext(resolver, "mscorlib");
var asm = mlc.LoadFromAssemblyPath(Path.Combine(managed, "Assembly-CSharp.dll"));

Type[] types;
try { types = asm.GetTypes(); }
catch (ReflectionTypeLoadException ex) { types = ex.Types.Where(t => t != null).ToArray()!; Console.WriteLine("partial type load: " + ex.Types.Count(t => t == null) + " failed"); }
Console.WriteLine($"types in Assembly-CSharp: {types.Length}");

var defBase = asm.GetType("Verse.Def")!;
var defTypes = types.Where(t => t != defBase && t.IsClass && IsSub(t, defBase)).ToList();
Console.WriteLine($"Def subclasses: {defTypes.Count} (abstract: {defTypes.Count(t => t.IsAbstract)})");
var byNs = defTypes.GroupBy(t => t.Namespace ?? "(none)").OrderByDescending(g => g.Count()).Take(8).Select(g => $"{g.Key}={g.Count()}");
Console.WriteLine("by namespace: " + string.Join(", ", byNs));

const BindingFlags All = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.DeclaredOnly;
static bool IsSub(Type t, Type b) { for (var c = t.BaseType; c != null; c = c.BaseType) if (c.FullName == b.FullName) return true; return false; }

string Cat(Type ft)
{
    if (ft.IsEnum) return "enum";
    if (ft.IsArray) return "array";
    if (ft.IsGenericType)
    {
        var n = ft.GetGenericTypeDefinition().FullName!;
        if (n.StartsWith("System.Collections.Generic.List")) return "list";
        if (n.StartsWith("System.Collections.Generic.Dictionary")) return "dict";
        if (n.StartsWith("System.Collections.Generic.HashSet")) return "hashset";
        if (n.StartsWith("System.Nullable")) return "nullable";
        return "generic-other";
    }
    if (ft.FullName == "System.String") return "string";
    if (ft.IsPrimitive) return "primitive";
    if (IsSub(ft, defBase) || ft.FullName == "Verse.Def") return "def-ref";
    if (ft.IsValueType) return "struct";
    if (ft.IsClass) return "class";
    return "other";
}

// walk the closure of types reachable from Def fields
var seen = new HashSet<string>();
var queue = new Queue<Type>();
var fieldCats = new Dictionary<string, int>();
var attrHist = new Dictionary<string, int>();
int totalFields = 0;
var customLoaders = new List<string>();
void Enqueue(Type t)
{
    if (t == null) return;
    if (t.IsGenericParameter || t.IsPointer || t.IsByRef) return;
    if (t.IsArray) { Enqueue(t.GetElementType()!); return; }
    if (t.IsGenericType) { foreach (var a in t.GetGenericArguments()) Enqueue(a); }
    var key = t.IsGenericType ? t.GetGenericTypeDefinition().FullName! : t.FullName!;
    if (key == null) return;
    if (t.Assembly.GetName().Name != "Assembly-CSharp") return;
    if (!seen.Add(key)) return;
    queue.Enqueue(t);
}
foreach (var t in defTypes) Enqueue(t);
// polymorphism: the game lets XML say Class="X" for any assignable subclass, so include subclasses of reachable abstract/base types.
var allByBase = new Dictionary<string, List<Type>>();
foreach (var t in types) for (var b = t.BaseType; b != null; b = b.BaseType) { var k = b.IsGenericType ? b.GetGenericTypeDefinition().FullName! : b.FullName!; if (k == null) continue; if (!allByBase.TryGetValue(k, out var l)) allByBase[k] = l = new(); l.Add(t); }
int processed = 0;
var reachableClasses = 0; var reachableStructs = 0; var reachableEnums = 0;
while (queue.Count > 0)
{
    var t = queue.Dequeue();
    processed++;
    if (t.BaseType != null) Enqueue(t.BaseType);
    if (t.IsEnum) { reachableEnums++; continue; }
    if (t.IsValueType) reachableStructs++; else reachableClasses++;
    if (t.GetMethods(All | BindingFlags.Static).Any(m => m.Name == "LoadDataFromXmlCustom")) customLoaders.Add(t.FullName!);
    // subclasses (only for classes that are not Def types themselves, to keep the closure honest)
    var k = t.IsGenericType ? t.GetGenericTypeDefinition().FullName! : t.FullName!;
    if (!defTypes.Contains(t) && allByBase.TryGetValue(k, out var subs)) foreach (var s in subs) Enqueue(s);
    foreach (var f in t.GetFields(All))
    {
        if (f.IsStatic || f.IsLiteral) continue;
        totalFields++;
        var c = Cat(f.FieldType);
        fieldCats[c] = fieldCats.GetValueOrDefault(c) + 1;
        foreach (var ad in f.GetCustomAttributesData())
        {
            var an = ad.AttributeType.Name;
            attrHist[an] = attrHist.GetValueOrDefault(an) + 1;
        }
        Enqueue(f.FieldType);
    }
}
Console.WriteLine($"closure: {seen.Count} types (classes {reachableClasses}, structs {reachableStructs}, enums {reachableEnums}), instance fields: {totalFields}");
Console.WriteLine("field type categories: " + string.Join(", ", fieldCats.OrderByDescending(kv => -kv.Value).Select(kv => $"{kv.Key}={kv.Value}")));
var interesting = new[] { "UnsavedAttribute", "LoadAliasAttribute", "NoTranslateAttribute", "MustTranslateAttribute", "TranslationHandleAttribute", "DefaultValueAttribute", "ObsoleteAttribute", "IgnoreSavedElementAttribute", "CaseInsensitiveXMLParsing", "XmlInheritanceAllowDuplicateNodes", "DescriptionAttribute", "TooltipAttribute", "MayRequireAttribute", "MayRequireRoyalty", "MayRequireIdeology", "MayRequireBiotech", "MayRequireAnomaly", "MayRequireOdyssey", "EditorHidden", "UnsavedAttribute", "TweakValue", "IgnoreInDefInjectionAttribute" };
Console.WriteLine("field attribute histogram (selected): " + string.Join(", ", attrHist.Where(kv => interesting.Contains(kv.Key) || kv.Key.StartsWith("MayRequire")).OrderByDescending(kv => kv.Value).Select(kv => $"{kv.Key}={kv.Value}")));
Console.WriteLine("top attributes overall: " + string.Join(", ", attrHist.OrderByDescending(kv => kv.Value).Take(12).Select(kv => $"{kv.Key}={kv.Value}")));
Console.WriteLine($"types with LoadDataFromXmlCustom in closure: {customLoaders.Count}: {string.Join(", ", customLoaders.Take(40))}");

// type-level attributes
int ci = 0, ign = 0;
foreach (var t in types.Where(x => seen.Contains(x.IsGenericType ? x.GetGenericTypeDefinition().FullName! : x.FullName!)))
{
    foreach (var ad in t.GetCustomAttributesData())
    {
        if (ad.AttributeType.Name == "CaseInsensitiveXMLParsing") ci++;
        if (ad.AttributeType.Name == "IgnoreSavedElementAttribute") ign++;
    }
}
Console.WriteLine($"type-level: CaseInsensitiveXMLParsing={ci}, IgnoreSavedElement={ign}");

// ThingDef sample
var thingDef = asm.GetType("Verse.ThingDef")!;
var chain = new List<Type>();
for (var c = thingDef; c != null && c.FullName != "System.Object"; c = c.BaseType) chain.Add(c);
Console.WriteLine("ThingDef chain: " + string.Join(" -> ", chain.Select(c => c.Name)));
int tdFields = 0, tdPublic = 0, tdNonPublic = 0, tdUnsaved = 0, tdAlias = 0, tdDefRef = 0;
var sample = new List<string>();
foreach (var c in chain)
{
    foreach (var f in c.GetFields(All))
    {
        if (f.IsStatic || f.IsLiteral) continue;
        tdFields++;
        if (f.IsPublic) tdPublic++; else tdNonPublic++;
        var names = f.GetCustomAttributesData().Select(a => a.AttributeType.Name).ToList();
        if (names.Contains("UnsavedAttribute")) tdUnsaved++;
        if (names.Contains("LoadAliasAttribute")) tdAlias++;
        if (Cat(f.FieldType) == "def-ref" || (f.FieldType.IsGenericType && f.FieldType.GetGenericArguments().Any(a => Cat(a) == "def-ref"))) tdDefRef++;
        if (sample.Count < 30 && f.IsPublic && !names.Contains("UnsavedAttribute")) sample.Add($"{f.Name}:{(f.FieldType.IsGenericType ? f.FieldType.Name + "<" + string.Join(",", f.FieldType.GetGenericArguments().Select(a => a.Name)) + ">" : f.FieldType.Name)}");
    }
}
Console.WriteLine($"ThingDef (with inherited): fields={tdFields} public={tdPublic} nonpublic={tdNonPublic} unsaved={tdUnsaved} loadAlias={tdAlias} defRefLike={tdDefRef}");
Console.WriteLine("ThingDef sample fields: " + string.Join(", ", sample));

// size estimate for a compact schema: one record per type, one per field
var estimate = new
{
    types = seen.Count,
    fields = totalFields,
    approxJsonBytesMinified = seen.Count * 60 + totalFields * 38
};
Console.WriteLine("compact schema estimate: " + JsonSerializer.Serialize(estimate));


// ---- schema export for the validation experiment ----
string Fmt(Type t)
{
    if (t.IsArray) return "A<" + Fmt(t.GetElementType()!) + ">";
    if (t.IsGenericType)
    {
        var d = t.GetGenericTypeDefinition().FullName!;
        var args = string.Join(",", t.GetGenericArguments().Select(Fmt));
        if (d.StartsWith("System.Collections.Generic.List`1")) return "L<" + args + ">";
        if (d.StartsWith("System.Collections.Generic.Dictionary`2")) return "D<" + args + ">";
        if (d.StartsWith("System.Collections.Generic.HashSet`1")) return "H<" + args + ">";
        if (d.StartsWith("System.Nullable`1")) return "N<" + args + ">";
        return "G:" + d + "<" + args + ">";
    }
    return t.FullName ?? t.Name;
}
var export = new Dictionary<string, object>();
var closureTypes = types.Where(x => seen.Contains(x.IsGenericType ? x.GetGenericTypeDefinition().FullName! : x.FullName!)).ToList();
var extra = new List<Type>();
foreach (var t in closureTypes)
{
    var fl = new List<object>();
    foreach (var f in t.GetFields(All))
    {
        if (f.IsStatic || f.IsLiteral) continue;
        var attrs = f.GetCustomAttributesData();
        string? alias = null; bool unsaved = false; bool allowLoading = false;
        foreach (var a in attrs)
        {
            if (a.AttributeType.Name == "LoadAliasAttribute") alias = a.ConstructorArguments.Count > 0 ? a.ConstructorArguments[0].Value?.ToString() : null;
            if (a.AttributeType.Name == "UnsavedAttribute") { unsaved = true; if (a.ConstructorArguments.Count > 0 && a.ConstructorArguments[0].Value is bool b) allowLoading = b; }
        }
        var aliases = attrs.Where(a => a.AttributeType.Name == "LoadAliasAttribute").Select(a => a.ConstructorArguments[0].Value?.ToString()).ToList();
        fl.Add(new { n = f.Name, t = Fmt(f.FieldType), u = unsaved && !allowLoading ? 1 : 0, a = aliases, pub = f.IsPublic ? 1 : 0 });
    }
    var names = t.IsEnum ? t.GetFields(BindingFlags.Static | BindingFlags.Public).Select(f => f.Name).ToArray() : Array.Empty<string>();
    bool flags = t.GetCustomAttributesData().Any(a => a.AttributeType.Name == "FlagsAttribute");
    bool custom = t.GetMethods(All | BindingFlags.Static).Any(m => m.Name == "LoadDataFromXmlCustom");
    var ignEl = t.GetCustomAttributesData().Where(a => a.AttributeType.Name == "IgnoreSavedElementAttribute").Select(a => a.ConstructorArguments[0].Value?.ToString()).ToList();
    export[t.FullName!] = new { b = t.BaseType?.FullName, abs = t.IsAbstract ? 1 : 0, kind = t.IsEnum ? "enum" : t.IsValueType ? "struct" : "class", flags, custom, ign = ignEl, enumNames = names, f = fl, ns = t.Namespace, name = t.Name, def = defTypes.Contains(t) ? 1 : 0 };
}
File.WriteAllText("schema.json", JsonSerializer.Serialize(export));
Console.WriteLine("wrote schema.json: " + new FileInfo("schema.json").Length + " bytes, types: " + export.Count);
