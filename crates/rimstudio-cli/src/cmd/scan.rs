//! `scan`: the library scan with counts and timings.

use std::fmt::Write as _;

use serde_json::json;

use crate::cli::ScanArgs;
use crate::error::CliResult;
use crate::fmt::{arr_at, bool_at, str_at, u64_at};
use crate::session::Session;

/// Runs the library scan job and prints its result.
///
/// # Errors
/// The envelope of the scan (`library.scan-failed` when no source is known).
pub(crate) fn run(s: &Session, args: &ScanArgs) -> CliResult {
    let reply = s.call("library_scan", json!({"full": args.full}))?;
    let v = &reply.value;
    s.emit(v, || {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "scan {} (revision {})",
            if bool_at(v, "/cancelled") {
                "cancelled"
            } else {
                "finished"
            },
            u64_at(v, "/rev")
        );
        let _ = writeln!(
            out,
            "  folders probed: {}",
            u64_at(v, "/stats/foldersProbed")
        );
        let _ = writeln!(out, "  mods found: {}", u64_at(v, "/stats/modsFound"));
        let _ = writeln!(out, "  mods indexed: {}", u64_at(v, "/stats/modsIndexed"));
        let _ = writeln!(out, "  definitions: {}", u64_at(v, "/stats/defs"));
        let _ = writeln!(
            out,
            "  files parsed or reused: {} / {}",
            u64_at(v, "/stats/defFilesParsed"),
            u64_at(v, "/stats/defFilesReused")
        );
        let _ = writeln!(
            out,
            "  time: {} ms total (discover {}, metadata {}, definitions {})",
            u64_at(v, "/timings/totalMs"),
            u64_at(v, "/timings/discoverMs"),
            u64_at(v, "/timings/metadataMs"),
            u64_at(v, "/timings/definitionsMs"),
        );
        for src in arr_at(v, "/sources") {
            let _ = writeln!(
                out,
                "  source {} ({}): {} mods, {}",
                str_at(src, "/id"),
                str_at(src, "/path"),
                u64_at(src, "/mods"),
                str_at(src, "/status")
            );
        }
        if let Some(counts) = v["diagnostics"]["counts"].as_object() {
            for (code, n) in counts {
                let _ = writeln!(out, "  diagnostic {code}: {}", n.as_u64().unwrap_or(0));
            }
        }
        out
    });
    let d = &v["diagnostics"];
    if u64_at(d, "/warnings") > 0 || u64_at(d, "/errors") > 0 || bool_at(v, "/cancelled") {
        s.note_warning();
    }
    Ok(())
}
