use std::time::Instant;

fn main() {
    // ---- steamlocate on this machine (paths only; never print last_user / ids) ----
    println!("== steamlocate::locate_all() ==");
    match steamlocate::locate_all() {
        Ok(dirs) => {
            for d in &dirs {
                println!("steam dir: {}", d.path().display());
            }
            println!("count: {}", dirs.len());
        }
        Err(e) => println!("locate_all error: {e}"),
    }
    println!("== steamlocate::locate() ==");
    let steam = match steamlocate::locate() {
        Ok(s) => s,
        Err(e) => { println!("locate error: {e}"); return; }
    };
    println!("first: {}", steam.path().display());
    println!("== libraries ==");
    match steam.library_paths() {
        Ok(p) => for x in p { println!("library path: {}", x.display()); },
        Err(e) => println!("library_paths error: {e}"),
    }
    match steam.libraries() {
        Ok(it) => for lib in it {
            match lib {
                Ok(l) => println!("library ok: {} ({} appmanifests)", l.path().display(), l.app_ids().len()),
                Err(e) => println!("library error: {e}"),
            }
        },
        Err(e) => println!("libraries error: {e}"),
    }
    println!("== find_app(294100) ==");
    match steam.find_app(294100) {
        Ok(Some((app, lib))) => {
            println!("name={:?} install_dir={:?} build_id={:?} target_build_id={:?}", app.name, app.install_dir, app.build_id, app.target_build_id);
            println!("state_flags={:?} -> {:?}", app.state_flags, app.state_flags.map(|s| s.flags().collect::<Vec<_>>()));
            println!("last_updated={:?}", app.last_updated);
            println!("size_on_disk={:?} bytes_to_download={:?}", app.size_on_disk, app.bytes_to_download);
            println!("resolved app dir: {}", lib.resolve_app_dir(&app).display());
            println!("installed depots: {:?}", app.installed_depots.keys().collect::<Vec<_>>());
        }
        Ok(None) => println!("not found"),
        Err(e) => println!("find_app error: {e}"),
    }
    println!("== compat_tool_mapping() ==");
    match steam.compat_tool_mapping() {
        Ok(m) => {
            println!("entries: {}", m.len());
            println!("default (id 0): {:?}", m.get(&0).and_then(|c| c.name.clone()));
            println!("294100 override: {:?}", m.get(&294100).and_then(|c| c.name.clone()));
        }
        Err(e) => println!("compat_tool_mapping error: {e}"),
    }
    println!("== shortcuts ==");
    match steam.shortcuts() {
        Ok(it) => println!("shortcuts iterator ok, count={}", it.count()),
        Err(e) => println!("shortcuts error: {e}"),
    }

    // ---- keyvalues-parser on real files ----
    println!("== keyvalues-parser on real files ==");
    let root = steam.path();
    let mut files: Vec<std::path::PathBuf> = vec![
        root.join("steamapps/libraryfolders.vdf"),
        root.join("config/libraryfolders.vdf"),
        root.join("config/config.vdf"),
        root.join("steamapps/workshop/appworkshop_294100.acf"),
    ];
    if let Ok(rd) = std::fs::read_dir(root.join("steamapps")) {
        let mut v: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| {
            p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with("appmanifest_")).unwrap_or(false)
        }).collect();
        v.sort();
        files.extend(v);
    }
    let (mut ok, mut bad) = (0, 0);
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        let t0 = Instant::now();
        let r = keyvalues_parser::parse(&text);
        let dt = t0.elapsed();
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        match r {
            Ok(_) => { ok += 1; if name.contains("294100") || name.contains("libraryfolders") || name == "config.vdf" { println!("OK   {name} ({} bytes) in {:?}", text.len(), dt); } }
            Err(e) => { bad += 1; println!("FAIL {name} ({} bytes): {}", text.len(), format!("{e}").lines().next().unwrap_or("")); }
        }
    }
    println!("parsed ok={ok} failed={bad}");
}
