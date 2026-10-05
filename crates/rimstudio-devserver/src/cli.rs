//! The command line of `rimstudio-devserver`: flags, defaults and the help text.

use crate::server::{DEFAULT_ORIGINS, DEFAULT_PORT};

/// The environment variable that sets the port when `--port` is absent.
pub const PORT_VAR: &str = "RIMSTUDIO_BRIDGE_PORT";

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The port; 0 picks a free one.
    pub port: u16,
    /// The data folder; `None` means the default under the home folder.
    pub data_dir: Option<String>,
    /// The token file; `None` means the default in the repository.
    pub token_file: Option<String>,
    /// The allowed origins (the defaults when no flag is given).
    pub allow_origins: Vec<String>,
}

/// The result of reading the command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    /// Print the help and exit.
    Help,
    /// Run with this configuration.
    Run(Config),
}

/// The help text.
#[must_use]
pub fn help_text() -> String {
    format!(
        "rimstudio-devserver {version}\n\
\n\
Development bridge: serves the RimStudio command registry over HTTP on 127.0.0.1 so that the browser\n\
UI can work with the real backend. For development only; it is never packaged.\n\
\n\
USAGE:\n\
    rimstudio-devserver [FLAGS]\n\
\n\
FLAGS:\n\
    --port N              Port to listen on, 0 picks a free one (default {port}, or ${port_var}).\n\
    --data-dir PATH       Data folder with config, data, cache and logs inside (default\n\
                          $HOME/.local/share/rimstudio-dev). Kept apart from the CLI and the app.\n\
    --token-file PATH     Where to write {{\"port\":N,\"token\":\"...\"}} (default\n\
                          node_modules/.cache/rimstudio-bridge.json in the repository root).\n\
    --allow-origin URL    Origin a page may call from; repeat for several. Replaces the defaults\n\
                          {origins}.\n\
    -h, --help            Show this text.\n\
\n\
Every request except GET /dev/health needs the header x-rimstudio-token with the token printed at\n\
start. Stop the server by typing quit and pressing Enter in its terminal; Ctrl+C also ends it but\n\
skips the clean shutdown mark (the standard library cannot catch the signal).\n",
        version = env!("CARGO_PKG_VERSION"),
        port = DEFAULT_PORT,
        port_var = PORT_VAR,
        origins = DEFAULT_ORIGINS.join(" and "),
    )
}

fn valid_origin(origin: &str) -> bool {
    let Some((scheme, rest)) = origin.split_once("://") else {
        return false;
    };
    (scheme == "http" || scheme == "https")
        && !rest.is_empty()
        && !rest.contains(['/', '?', '#', ' '])
}

fn parse_port(text: &str, what: &str) -> Result<u16, String> {
    text.parse::<u16>()
        .map_err(|_| format!("{what} must be a number from 0 to 65535, got {text:?}"))
}

/// Reads the arguments (without the program name). `env_port` is the value of [`PORT_VAR`].
///
/// # Errors
/// A message for an unknown flag, a missing value or a value that does not parse.
pub fn parse(
    args: impl IntoIterator<Item = String>,
    env_port: Option<&str>,
) -> Result<Parsed, String> {
    let mut port: Option<u16> = None;
    let mut data_dir = None;
    let mut token_file = None;
    let mut origins: Vec<String> = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        if arg == "-h" || arg == "--help" {
            return Ok(Parsed::Help);
        }
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
            _ => (arg.clone(), None),
        };
        if !matches!(
            flag.as_str(),
            "--port" | "--data-dir" | "--token-file" | "--allow-origin"
        ) {
            return Err(format!("unknown argument {arg:?}; try --help"));
        }
        let value = match inline {
            Some(v) => v,
            None => iter.next().ok_or_else(|| format!("{flag} needs a value"))?,
        };
        match flag.as_str() {
            "--port" => port = Some(parse_port(&value, "--port")?),
            "--data-dir" => data_dir = Some(value),
            "--token-file" => token_file = Some(value),
            _ => {
                if !valid_origin(&value) {
                    return Err(format!(
                        "--allow-origin must look like http://localhost:5173, got {value:?}"
                    ));
                }
                origins.push(value);
            }
        }
    }
    let port = match (port, env_port.filter(|v| !v.is_empty())) {
        (Some(p), _) => p,
        (None, Some(v)) => parse_port(v, PORT_VAR)?,
        (None, None) => DEFAULT_PORT,
    };
    if origins.is_empty() {
        origins = DEFAULT_ORIGINS.iter().map(|s| (*s).to_owned()).collect();
    }
    Ok(Parsed::Run(Config {
        port,
        data_dir,
        token_file,
        allow_origins: origins,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str], env: Option<&str>) -> Result<Parsed, String> {
        parse(args.iter().map(|s| (*s).to_owned()), env)
    }

    fn config(args: &[&str], env: Option<&str>) -> Config {
        match run(args, env).unwrap() {
            Parsed::Run(c) => c,
            Parsed::Help => panic!("help"),
        }
    }

    #[test]
    fn defaults() {
        let c = config(&[], None);
        assert_eq!(c.port, 7878);
        assert_eq!(c.data_dir, None);
        assert_eq!(c.token_file, None);
        assert_eq!(c.allow_origins.len(), 2);
    }

    #[test]
    fn flags_and_equals_form() {
        let c = config(
            &[
                "--port",
                "0",
                "--data-dir=/x/y",
                "--token-file",
                "/t.json",
                "--allow-origin",
                "http://localhost:3000",
                "--allow-origin=https://a.test",
            ],
            Some("9999"),
        );
        assert_eq!(c.port, 0);
        assert_eq!(c.data_dir.as_deref(), Some("/x/y"));
        assert_eq!(c.token_file.as_deref(), Some("/t.json"));
        assert_eq!(c.allow_origins, ["http://localhost:3000", "https://a.test"]);
    }

    #[test]
    fn the_environment_port_is_used_when_the_flag_is_absent() {
        assert_eq!(config(&[], Some("9001")).port, 9001);
        assert_eq!(config(&["--port", "1"], Some("9001")).port, 1);
        assert_eq!(config(&[], Some("")).port, 7878);
        assert!(run(&[], Some("abc")).is_err());
    }

    #[test]
    fn help_wins() {
        assert_eq!(run(&["--help"], None).unwrap(), Parsed::Help);
        assert_eq!(run(&["--port", "1", "-h"], None).unwrap(), Parsed::Help);
        assert!(help_text().contains("--allow-origin"));
    }

    #[test]
    fn bad_input_is_an_error() {
        assert!(run(&["--port"], None).is_err());
        assert!(run(&["--port", "70000"], None).is_err());
        assert!(run(&["--port", "x"], None).is_err());
        assert!(run(&["--nope"], None).is_err());
        assert!(run(&["stray"], None).is_err());
        assert!(run(&["--allow-origin", "localhost:5173"], None).is_err());
        assert!(run(&["--allow-origin", "http://a/b"], None).is_err());
    }
}
