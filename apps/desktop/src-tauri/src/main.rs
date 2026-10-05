//! The `rimstudio` binary: the desktop application, or a headless self test with `--smoke`.

// A release build on Windows is a GUI program without a console; `--smoke` then reports only through
// its exit code.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    rimstudio_shell::app::run(std::env::args().skip(1))
}
