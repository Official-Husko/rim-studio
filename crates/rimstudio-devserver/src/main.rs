#![allow(clippy::print_stdout, clippy::print_stderr)]
//! `rimstudio-devserver`: the development bridge. Never packaged.
//!
//! Boots the application with its own data roots, binds the loopback port, prints the token, writes the
//! token file and serves until `quit` is typed. See `rimstudio_devserver` for the protocol.

use std::io::{BufRead as _, IsTerminal as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use rimstudio_app::logging::LogConfig;
use rimstudio_app::{BootInput, Platform, boot};
use rimstudio_devserver::cli::{self, Parsed};
use rimstudio_devserver::{ServerOptions, start};

/// The repository root: the nearest folder above the working directory that holds this crate, else
/// the one this binary was built from.
fn repo_root() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        for dir in cwd.ancestors() {
            if dir.join("crates").join("rimstudio-devserver").is_dir() {
                return dir.to_path_buf();
            }
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn main() -> ExitCode {
    let env_port = std::env::var(cli::PORT_VAR).ok();
    let config = match cli::parse(std::env::args().skip(1), env_port.as_deref()) {
        Ok(Parsed::Help) => {
            print!("{}", cli::help_text());
            return ExitCode::SUCCESS;
        }
        Ok(Parsed::Run(config)) => config,
        Err(message) => {
            eprintln!("rimstudio-devserver: {message}");
            return ExitCode::from(2);
        }
    };

    let platform = Platform::system();
    let Some(home) = platform.env.home_dir() else {
        eprintln!("rimstudio-devserver: the home folder is unknown; pass --data-dir");
        return ExitCode::from(1);
    };
    let data_dir = config.data_dir.clone().unwrap_or_else(|| {
        home.join(".local")
            .join("share")
            .join("rimstudio-dev")
            .as_str()
            .to_owned()
    });
    let token_file = config.token_file.as_ref().map_or_else(
        || {
            repo_root()
                .join("node_modules")
                .join(".cache")
                .join("rimstudio-bridge.json")
        },
        PathBuf::from,
    );

    let mut log = LogConfig::file_only();
    log.stderr = std::env::var_os("RIMSTUDIO_LOG").is_some();
    let input = BootInput::new(platform)
        .with_data_base(&data_dir)
        .with_log(log);
    let app = match boot(input) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("rimstudio-devserver: the application could not start: {e}");
            return ExitCode::from(1);
        }
    };

    let options = ServerOptions {
        port: config.port,
        token: None,
        allow_origins: config.allow_origins.clone(),
        token_file: Some(token_file.clone()),
        data_dir: data_dir.clone(),
        ..ServerOptions::default()
    };
    let server = match start(app.clone(), options) {
        Ok(server) => server,
        Err(e) => {
            eprintln!(
                "rimstudio-devserver: cannot start on port {}: {e}",
                config.port
            );
            return ExitCode::from(1);
        }
    };

    println!(
        "rimstudio-devserver {} (development only, never packaged)",
        rimstudio_devserver::BRIDGE_VERSION
    );
    println!("listening        http://{}", server.addr());
    println!("token            {}", server.token());
    println!("token file       {}", token_file.display());
    println!("data dir         {data_dir}");
    println!("allowed origins  {}", server.allow_origins().join(", "));
    println!("commands         {}", rimstudio_app::registry::ROUTES.len());

    if std::io::stdin().is_terminal() {
        println!(
            "Type quit and press Enter to stop (Ctrl+C ends the process without the clean mark)."
        );
        let stopper = server.stopper();
        let spawned = std::thread::Builder::new()
            .name("bridge-stdin".to_owned())
            .spawn(move || {
                for line in std::io::stdin().lock().lines() {
                    let Ok(line) = line else { break };
                    if matches!(line.trim(), "q" | "quit" | "exit" | "stop") {
                        break;
                    }
                }
                stopper.stop();
            });
        if let Err(e) = spawned {
            eprintln!("rimstudio-devserver: no stdin control: {e}");
        }
    } else {
        println!("Stop with Ctrl+C or by ending the process.");
    }

    server.wait();
    app.shutdown(Duration::from_secs(5));
    println!("stopped");
    ExitCode::SUCCESS
}
