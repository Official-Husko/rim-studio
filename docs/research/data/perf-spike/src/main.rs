//! rimstudio-perf-spike: throwaway benchmark for scan and index speed of a RimWorld mod library.
//! Usage: spike <stage> [--roots a,b] [--runs N] [--threads 1,2,4] [--out results.jsonl] ...

mod about;
mod defs;
mod modscan;
mod roots;
mod stage_s1;
mod stage_s2;
mod stage_s3;
mod stage_s456;
mod util;
mod walk;

fn main() {
    let a = util::Args::parse();
    let stage = a.pos.first().map(|s| s.as_str()).unwrap_or("help");
    match stage {
        "s1" => stage_s1::run(&a),
        "s2" => stage_s2::run(&a),
        "s2-robust" => stage_s2::robust(&a),
        "s3" => stage_s3::run(&a),
        "s4" => stage_s456::run_s4(&a),
        "s4-load" => stage_s456::run_s4_load(&a),
        "s5" => stage_s456::run_s5(&a),
        "s5-child" => stage_s456::run_s5_child(&a),
        "s6" => stage_s456::run_s6(&a),
        "s6-about" => stage_s456::run_about_cold(&a),
        _ => {
            eprintln!("usage: spike s1|s2|s3|s4|s5|s7|pitfalls ...");
            std::process::exit(2);
        }
    }
}
