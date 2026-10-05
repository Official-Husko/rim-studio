// compiled as a second binary: compares mini parser vs keyvalues-parser on real files and edge cases
mod mini;
use keyvalues_parser as kv;
use std::time::Instant;

fn kv_canon(v: &kv::Value<'_>, out: &mut String) {
    match v {
        kv::Value::Str(s) => { out.push('"'); out.push_str(s); out.push('"'); }
        kv::Value::Obj(o) => {
            out.push('{');
            for (k, vs) in o.iter() { for v in vs { out.push('"'); out.push_str(k); out.push_str("\":"); kv_canon(v, out); out.push(','); } }
            out.push('}');
        }
    }
}
fn kv_doc(text: &str, literal: bool) -> Result<String, String> {
    let p = kv::Parser::new().literal_special_chars(literal);
    let d = p.parse(text).map_err(|e| format!("{e}").lines().next().unwrap_or("").trim().to_string())?;
    let mut s = String::new();
    s.push('"'); s.push_str(&d.key); s.push_str("\":"); kv_canon(&d.value, &mut s);
    Ok(s)
}
fn mini_doc(text: &str, esc: mini::Esc) -> Result<String, String> {
    let d = mini::parse(text, esc)?;
    let mut s = String::new();
    // mimic: one root pair (take first) for comparison with keyvalues-parser
    let mut sorted: Vec<&(std::borrow::Cow<str>, mini::Val)> = d.root.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, v) in sorted { s.push('"'); s.push_str(k); s.push_str("\":"); mini::canon(v, &mut s); }
    Ok(s)
}

fn main() {
    let home = std::env::var("HOME").unwrap();
    let steam = format!("{home}/.local/share/Steam");
    let mut files: Vec<String> = vec![
        format!("{steam}/steamapps/libraryfolders.vdf"),
        format!("{steam}/config/config.vdf"),
        format!("{steam}/steamapps/workshop/appworkshop_294100.acf"),
        format!("{steam}/steamapps/appmanifest_294100.acf"),
        format!("{home}/.steam/registry.vdf"),
    ];
    for e in std::fs::read_dir(format!("{steam}/steamapps")).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n.starts_with("appmanifest_") && n.ends_with(".acf") { files.push(e.path().to_string_lossy().to_string()); }
    }
    files.sort(); files.dedup();
    println!("== real files: identical tree from both parsers? ==");
    let (mut same, mut diff, mut kv_fail, mut mini_fail) = (0, 0, 0, 0);
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        let a = kv_doc(&text, false);
        let b = mini_doc(&text, mini::Esc::Process);
        let name = f.rsplit('/').next().unwrap();
        match (&a, &b) {
            (Ok(x), Ok(y)) => { if x == y { same += 1 } else { diff += 1; println!("DIFF {name}"); } }
            (Err(e), Ok(_)) => { kv_fail += 1; println!("keyvalues-parser(escaped) FAILS, mini OK: {name}: {e}"); 
                 let lit = kv_doc(&text, true); println!("   keyvalues-parser(literal_special_chars) => {}", if lit.is_ok() {"OK"} else {"FAIL"}); }
            (Ok(_), Err(e)) => { mini_fail += 1; println!("mini FAILS, kv OK: {name}: {e}") }
            (Err(_), Err(_)) => { println!("both fail: {name}") }
        }
    }
    println!("same={same} diff={diff} kv_only_fail={kv_fail} mini_only_fail={mini_fail}");

    println!("== timing: appworkshop_294100.acf (parse only, 50 iterations, release) ==");
    let text = std::fs::read_to_string(format!("{steam}/steamapps/workshop/appworkshop_294100.acf")).unwrap();
    let t0 = Instant::now(); for _ in 0..50 { let d = kv::parse(&text).unwrap(); std::hint::black_box(&d); } let a = t0.elapsed() / 50;
    let t0 = Instant::now(); for _ in 0..50 { let d = mini::parse(&text, mini::Esc::Process).unwrap(); std::hint::black_box(&d); } let b = t0.elapsed() / 50;
    println!("keyvalues-parser: {:?} per parse ({} bytes) | mini: {:?} per parse | ratio {:.1}x", a, text.len(), b, a.as_secs_f64() / b.as_secs_f64());
    let cfg = std::fs::read_to_string(format!("{steam}/config/config.vdf")).unwrap();
    let t0 = Instant::now(); for _ in 0..50 { let d = kv::parse(&cfg).unwrap(); std::hint::black_box(&d); } let a = t0.elapsed() / 50;
    let t0 = Instant::now(); for _ in 0..50 { let d = mini::parse(&cfg, mini::Esc::Process).unwrap(); std::hint::black_box(&d); } let b = t0.elapsed() / 50;
    println!("config.vdf: keyvalues-parser {:?} | mini {:?} | ratio {:.1}x", a, b, a.as_secs_f64() / b.as_secs_f64());

    println!("== edge cases: [keyvalues-parser escaped | keyvalues-parser literal | mini escaped] ==");
    let cases: Vec<(&str, String)> = vec![
        ("windows path escaped \\\\ (Steam libraryfolders style)", "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}".to_string()),
        ("single backslash path (registry.vdf style)", "\"Registry\"\n{\n\t\"p\"\t\"C:\\Steam\\steamapps\\sourcemods\"\n}".to_string()),
        ("UTF-8 BOM prefix", "\u{feff}\"a\"\n{\n\"k\" \"v\"\n}".to_string()),
        ("CRLF line endings", "\"a\"\r\n{\r\n\t\"k\"\t\"v\"\r\n}\r\n".to_string()),
        ("unquoted tokens", "a\n{\n k v\n path /home/x/Steam\n}".to_string()),
        ("duplicate keys", "\"a\"\n{\n\"k\" \"1\"\n\"k\" \"2\"\n\"o\" { \"x\" \"1\" }\n\"o\" { \"x\" \"2\" }\n}".to_string()),
        ("line comment", "// hi\n\"a\" // c\n{\n// c2\n\"k\" \"v\"\n}\n// end".to_string()),
        ("block comment /* */ (not in Valve parser)", "\"a\"\n{\n/* c */ \"k\" \"v\"\n}".to_string()),
        ("conditional [$WIN32] after value", "\"a\"\n{\n\"k\" \"v\" [$WIN32]\n\"k2\" \"v2\" [!$WIN32]\n}".to_string()),
        ("conditional before brace", "\"a\" [$LINUX]\n{\n\"k\" \"v\"\n}".to_string()),
        ("#base macro", "#base \"other.vdf\"\n\"a\"\n{\n\"k\" \"v\"\n}".to_string()),
        ("#include macro", "#include \"other.vdf\"\n\"a\"\n{\n\"k\" \"v\"\n}".to_string()),
        ("two top-level pairs", "\"a\" { \"k\" \"v\" }\n\"b\" { \"k\" \"w\" }".to_string()),
        ("truncated (missing closing brace)", "\"a\"\n{\n\"k\" \"v\"\n".to_string()),
        ("empty file", "".to_string()),
        ("whitespace only", "  \n\t\n".to_string()),
        ("trailing NUL byte", "\"a\" { \"k\" \"v\" }\n\u{0}".to_string()),
        ("newline inside quoted value", "\"a\"\n{\n\"k\" \"line1\nline2\"\n}".to_string()),
        ("escaped quote and tab", "\"a\"\n{\n\"k\" \"say \\\"hi\\\"\\t!\"\n}".to_string()),
        ("empty string value and empty object", "\"a\"\n{\n\"k\" \"\"\n\"o\" { }\n}".to_string()),
    ];
    for (name, text) in &cases {
        let a = kv_doc(text, false); let b = kv_doc(text, true); let c = mini_doc(text, mini::Esc::Process);
        let f = |r: &Result<String, String>| match r { Ok(s) => { let t: String = s.chars().take(46).collect(); format!("OK {t}") }, Err(e) => format!("ERR {}", e.chars().take(30).collect::<String>()) };
        println!("- {name}\n    kv-escaped : {}\n    kv-literal : {}\n    mini       : {}", f(&a), f(&b), f(&c));
    }
}
