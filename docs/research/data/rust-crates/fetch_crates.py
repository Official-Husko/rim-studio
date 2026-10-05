#!/usr/bin/env python3
"""Fetch crates.io metadata for a list of crates (1 request/second, generic User-Agent).
Writes crates_meta.json: {crate: {...}} with only public package data."""
import json, sys, time, urllib.request, urllib.error, datetime, os

UA = "rimstudio-research"
OUT = os.path.join(os.path.dirname(__file__), "crates_meta.json")

CRATES = {
 "tauri": ["tauri","tauri-build","tauri-plugin","tauri-utils","tauri-runtime","tauri-runtime-wry","wry","tao","tauri-cli","tauri-driver",
           "tauri-plugin-dialog","tauri-plugin-opener","tauri-plugin-shell","tauri-plugin-process","tauri-plugin-updater","tauri-plugin-log",
           "tauri-plugin-single-instance","tauri-plugin-window-state","tauri-plugin-global-shortcut","tauri-plugin-notification",
           "tauri-plugin-clipboard-manager","tauri-plugin-os","tauri-plugin-deep-link","tauri-plugin-fs","tauri-plugin-persisted-scope",
           "tauri-plugin-store","tauri-plugin-http","tauri-plugin-autostart","tauri-plugin-positioner","tauri-plugin-localhost",
           "tauri-plugin-upload","tauri-plugin-sql","tauri-plugin-stronghold","tauri-plugin-cli","tauri-plugin-websocket",
           "tauri-plugin-prevent-default","tauri-plugin-opener","rfd","arboard","dioxus"],
 "xml": ["quick-xml","roxmltree","xmltree","xml-rs","xml","sxd-document","sxd-xpath","xee-xpath","xee-xpath-compiler","xee-xpath-ast","xot","skyscraper",
         "libxml","xrust","xpath_reader","amxml","minidom","rxml","xmlparser","serde-xml-rs","xmlwriter","xml-builder","elementtree","treexml",
         "encoding_rs","encoding_rs_io","chardetng","strong-xml","xmltree-rs","xmlem","kuchiki","scraper","tl","html5ever","xpath-reader","fast-xml","xml5ever",
         "unxml","xmlsafe","simple-xml","xmlserde","quick-xml-to-json","xpath-rs","xpath","sxd_xpath","xee-interpreter","xpath_parser"],
 "parsing": ["winnow","nom","logos","chumsky","pest","lalrpop","combine"],
 "fs": ["walkdir","ignore","jwalk","rayon","tokio","crossbeam-channel","crossbeam-deque","crossbeam","flume","async-channel","memmap2","notify",
        "notify-debouncer-mini","notify-debouncer-full","file-id","fs-err","same-file","dunce","camino","junction","trash","globset","fs4","fd-lock",
        "filetime","normpath","path-slash","opener","open","which","fs_extra","symlink","reflink-copy","sysinfo","winreg","windows","windows-sys","dashmap",
        "parking_lot","arc-swap","tokio-util","futures","rustix","inotify","remove_dir_all","path-clean","pathdiff","relative-path","typed-path","faccess",
        "std-fs-cap","cap-std","tempfile","scopeguard","ctrlc","signal-hook","fs-set-times","fastrand","bstr","memchr","regex","aho-corasick","wild","glob","wax"],
 "persist": ["rusqlite","sqlx","diesel","redb","fjall","sled","heed","rocksdb","native_db","turso","limbo","serde","serde_json","simd-json","sonic-rs","rkyv","bincode",
             "postcard","rmp-serde","ciborium","bitcode","speedy","borsh","serde_yaml","serde_norway","zstd","lz4_flex","snap","blake3","xxhash-rust","twox-hash",
             "rustc-hash","foldhash","ahash","fxhash","seahash","gxhash","highway","siphasher","sha2","md-5","crc32fast","hex","base64","bs58",
             "atomic-write-file","atomicwrites","jsonc-parser","json5","serde_jsonc","json_comments","serde_json_lenient","jsonc","toml","toml_edit","basic-toml",
             "serde_with","keyring","keyring-core","secret-service","oo7","security-framework","directories","etcetera","dirs","platform-dirs","app_dirs2",
             "directories-next","xdg","known-folders","home","tempfile","serde_path_to_error","json-patch","serde_ignored","figment","config","confy","rmpv","jiff","chrono","time"],
 "graph_text": ["petgraph","daggy","topological-sort","pathfinding","nucleo","nucleo-matcher","fuzzy-matcher","strsim","rapidfuzz","sublime_fuzzy","tantivy",
                "grep-searcher","grep-regex","grep-matcher","regex-automata","fancy-regex","natord","alphanumeric-sort","lexical-sort","human-sort","natural-sort-rs",
                "unicode-normalization","caseless","unicase","icu_casemap","icu_normalizer","unicode-segmentation","unicode-width","deunicode","semver",
                "lenient_semver","versions","version-compare","smol_str","compact_str","ecow","arcstr","lasso","string-interner","ustr","internment","indexmap",
                "smallvec","arrayvec","thin-vec","bumpalo","typed-arena","slotmap","hashbrown","bitflags","itertools","ordered-float"],
 "net_steam": ["reqwest","ureq","hyper","rustls","native-tls","rustls-platform-verifier","rustls-native-certs","webpki-roots","aws-lc-rs","ring","http-cache-reqwest",
               "http-cache-semantics","reqwest-middleware","reqwest-retry","retry-policies","backon","backoff","governor","url","percent-encoding","mime","futures-util",
               "bytes","async-compression","steamlocate","keyvalues-parser","keyvalues-serde","vdf-reader","steam-vdf-parser","steamy-vdf","vdf-parser","steam-vdf",
               "steamworks","steamworks-sys","steam-vent","steam-rs","steamcmd","portable-pty","pty-process","expectrl","conpty","octocrab","steam-workshop","steamid-ng",
               "isahc","surf","attohttpc","minreq","httpmock","wiremock","tokio-tungstenite","tower","hyper-util","http","tokio-rustls","rustls-pemfile","webpki-root-certs",
               "tokio-stream","async-openai","genai","rig-core","sentry"],
 "images": ["image","fast_image_resize","resvg","usvg","tiny-skia","ddsfile","image_dds","intel_tex_2","texpresso","bcndecode","squish","block_compression",
            "basis-universal","png","oxipng","webp","zune-image","zune-png","imagesize","imageproc","turbojpeg","mozjpeg","jpeg-decoder","qoi","ravif","rgb","half",
            "lodepng","spng","fdeflate","miniz_oxide","zlib-rs","flate2","libdeflater","image-compare","texture-synthesis","bc7enc","bc7e","ispc_rt","ispc","cmake","cc",
            "tinydds","dds-rs","directxtex","compressonator","wgpu","sixel-image","color-quant","exr","tiff","gif"],
 "archives_git": ["zip","sevenz-rust2","sevenz-rust","tar","flate2","zstd","ruzstd","lzma-rs","xz2","liblzma","async_zip","bzip2","unrar","compress-tools","gix","git2",
                  "gix-url","gix-transport","gix-protocol","gix-features","git-url-parse","self_update","cargo-dist","zip-extract","unarc-rs","lzma-sys","zstd-safe","zstd-sys","libz-sys","cargo-packager","cargo-bundle","nsis"],
 "errors_logging": ["thiserror","anyhow","miette","snafu","eyre","color-eyre","error-stack","displaydoc","tracing","tracing-subscriber","tracing-appender","tracing-log",
                    "tracing-error","tracing-tracy","tracing-chrome","tracing-flame","log","fern","env_logger","flexi_logger","sentry","sentry-tauri","minidumper","crash-handler",
                    "human-panic","color-backtrace","backtrace","better-panic","panic-message","tracing-test","tracing-opentelemetry","tracing-indicatif","tracing-journald","tracing-oslog","tracing-android","tracing-bunyan-formatter","tracing-serde"],
 "testing": ["insta","proptest","rstest","assert_fs","assert_cmd","predicates","criterion","divan","cargo-nextest","cargo-llvm-cov","cargo-deny","cargo-audit",
             "cargo-machete","cargo-udeps","cargo-shear","cargo-hakari","just","mise","cargo-vet","cargo-semver-checks","cargo-bloat","cargo-about","cargo-license","cargo-release",
             "typos-cli","taplo-cli","cargo-mutants","cargo-fuzz","arbitrary","libfuzzer-sys","quickcheck","test-case","pretty_assertions","similar","similar-asserts",
             "serial_test","mockall","fake","trybuild","bolero","cargo-xtask","xtask","cargo-sort","cargo-edit","cargo-outdated","cargo-watch","bacon","sccache","cargo-tarpaulin","cargo-careful","cargo-geiger","cargo-cranky","cargo-binstall","cargo-auditable","cargo-cyclonedx","cargo-sbom","cargo-msrv","cargo-minimal-versions","cargo-hack","nextest-runner","iai-callgrind","gungraun","tango-bench","insta-cmd","expect-test","goldenfile","snapbox","trycmd","assert-json-diff","jsonschema","schemars"],
 "ipc": ["specta","specta-typescript","tauri-specta","specta-util","specta-serde","ts-rs","schemars","typeshare","typeshare-cli","utoipa","rspc","ts-rs-macros","serde_json","serde","tauri-typegen","taurpc","tauri-bindgen","wasm-bindgen","tsify","tsify-next","typescript-type-def","oxidize","typify","rust-to-ts","specta-macros"],
 "tooling": ["mimalloc","tikv-jemallocator","snmalloc-rs","clap","clap_complete","cargo-zigbuild","cargo-xwin","cross","mold","lld","wild-linker","wild","rust-lld","cargo-bundle","cargo-wix","cargo-appimage","cargo-deb","cargo-generate-rpm","linuxdeploy","cargo-dylint","dylint","cargo-chef","cargo-cache","cargo-sweep","cargo-expand","cargo-show-asm","cargo-flamegraph","samply","dhat","puffin","tracy-client","hotpath","profiling","coz","inferno","divan-macros"],
 "misc_checklist": ["docx-rs","printpdf","genpdf","typst","lopdf","scraper","bs58","base58","rust_decimal","humantime","bytesize","indicatif","console","dialoguer","ratatui","crossterm","chardet","charset-normalizer-rs","detect-lang","whatlang","lingua","unic-langid","fluent","fluent-bundle","fluent-templates","rust-i18n","gettext-rs","sys-locale","unicode-bidi","dotenvy","once_cell","lazy_static","futures-lite","smol","async-std","glommio","monoio","tokio-uring","io-uring","uuid","ulid","rand","nanoid","getrandom","hostname","whoami","machine-uid","os_info","sys-info","num_cpus","raw-cpuid","semver-parser","cargo_metadata","cargo_toml","toml-rs","git-version","vergen","built","shadow-rs","rustc_version","embed-resource","winresource","winres","windows-registry","windows-service","is-terminal","supports-color","owo-colors","colored","yansi","anstyle","termcolor","tabled","comfy-table","prettytable-rs","term-table"],
}

def get(url, tries=4):
    for i in range(tries):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "application/json"})
            with urllib.request.urlopen(req, timeout=40) as r:
                return r.status, json.loads(r.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return 404, None
            if e.code in (429, 500, 502, 503, 504):
                time.sleep(5 * (i + 1))
                continue
            return e.code, None
        except Exception:
            time.sleep(3 * (i + 1))
    return 0, None

def parse_dt(s):
    return datetime.datetime.fromisoformat(s.replace("Z", "+00:00"))

def main():
    names = []
    seen = set()
    for cat, lst in CRATES.items():
        for n in lst:
            if n not in seen:
                seen.add(n)
                names.append((cat, n))
    out = {}
    if os.path.exists(OUT):
        out = json.load(open(OUT))
    todo = [(c, n) for c, n in names if n not in out]
    print(f"{len(names)} crates, {len(todo)} to fetch", flush=True)
    for idx, (cat, n) in enumerate(todo):
        st, d = get(f"https://crates.io/api/v1/crates/{n}")
        time.sleep(1.05)
        if st == 404 or d is None:
            out[n] = {"category": cat, "status": st or "error", "exists": False}
        else:
            c = d["crate"]
            vers = d["versions"]
            stable = c.get("max_stable_version") or c.get("max_version")
            sv = next((v for v in vers if v["num"] == stable), None)
            newest = c.get("newest_version")
            nv = next((v for v in vers if v["num"] == newest), None)
            def vinfo(v):
                if not v: return None
                return {"num": v["num"], "created_at": v["created_at"], "yanked": v["yanked"], "license": v.get("license"),
                        "rust_version": v.get("rust_version"), "edition": v.get("edition"), "crate_size": v.get("crate_size"),
                        "features": sorted((v.get("features") or {}).keys())[:60], "has_lib": v.get("has_lib"), "bin_names": v.get("bin_names")}
            now = datetime.datetime(2026, 10, 4, tzinfo=datetime.timezone.utc)
            rel_dates = sorted(parse_dt(v["created_at"]) for v in vers if not v["yanked"] and "-" not in v["num"])
            last12 = sum(1 for t in rel_dates if (now - t).days <= 365)
            out[n] = {
                "category": cat, "exists": True,
                "description": (c.get("description") or "")[:200],
                "repository": c.get("repository"), "homepage": c.get("homepage"), "documentation": c.get("documentation"),
                "downloads": c.get("downloads"), "recent_downloads": c.get("recent_downloads"),
                "created_at": c.get("created_at"), "updated_at": c.get("updated_at"),
                "max_stable_version": c.get("max_stable_version"), "newest_version": newest, "max_version": c.get("max_version"),
                "stable": vinfo(sv), "newest": vinfo(nv) if newest != stable else None,
                "n_versions": len(vers), "n_yanked": sum(1 for v in vers if v["yanked"]),
                "releases_last_365d": last12,
                "first_release": rel_dates[0].isoformat() if rel_dates else None,
                "previous_stable": (sorted([v for v in vers if not v["yanked"] and "-" not in v["num"]], key=lambda v: v["created_at"])[-2]["num"]
                                    if len([v for v in vers if not v["yanked"] and "-" not in v["num"]]) >= 2 else None),
                "keywords": [k if isinstance(k, str) else k.get("id") for k in d.get("keywords", [])][:6],
            }
        if (idx + 1) % 10 == 0:
            json.dump(out, open(OUT, "w"), indent=1)
            print(f"{idx+1}/{len(todo)} {n}", flush=True)
    json.dump(out, open(OUT, "w"), indent=1)
    print("done", flush=True)

if __name__ == "__main__":
    main()
