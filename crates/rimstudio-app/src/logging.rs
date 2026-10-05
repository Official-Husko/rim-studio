//! Logging: `tracing` with a rolling JSON lines file, no telemetry and redaction of private text.
//!
//! [`init`] is the only place that installs a global subscriber (the permitted exception to I-16). The
//! file lives in the logs root as `rimstudio.<yyyy-mm-dd>.jsonl` (daily rotation, the last 14 files
//! kept by the appender), one JSON object per line. Every line passes through the redactor of
//! `rimstudio-core` before it is written, so home folders, user names, Steam ids and secrets never
//! reach the file or the console. Libraries only emit events; nothing here sends data anywhere.
//!
//! Installing a global subscriber can happen once per process. A second call (tests start many
//! application contexts) leaves the first subscriber in place and returns a guard with
//! [`LogGuard::installed`] false.

use std::io::{self, Write};
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::redact::Redactor;
use rimstudio_core::settings::LogLevel;
use rimstudio_io::atomic::atomic_write;
use rimstudio_io::roots::DataRoots;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// How many daily files the appender keeps.
pub const MAX_LOG_FILES: usize = 14;

/// The name stem of the log files.
pub const FILE_PREFIX: &str = "rimstudio";

/// The name suffix of the log files.
pub const FILE_SUFFIX: &str = "jsonl";

/// The environment variable that overrides the filter (same syntax as `RUST_LOG`).
pub const FILTER_VAR: &str = "RIMSTUDIO_LOG";

/// The targets of our own crates; the configured level applies to these and `warn` to the rest.
const OUR_CRATES: [&str; 14] = [
    "rimstudio_app",
    "rimstudio_core",
    "rimstudio_defs",
    "rimstudio_design",
    "rimstudio_io",
    "rimstudio_library",
    "rimstudio_manager",
    "rimstudio_platform",
    "rimstudio_steam",
    "rimstudio_toolkit",
    "rimstudio_workspace",
    "rimstudio_xml",
    "rimstudio_xpath",
    "panic",
];

/// What to set up.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Write the rolling JSON lines file in the logs root.
    pub file: bool,
    /// Also write human readable lines to standard error.
    pub stderr: bool,
    /// The level for `rimstudio_*` targets (the settings value); `None` means `info`.
    pub level: Option<LogLevel>,
    /// An explicit filter that wins over `level` (the value of [`FILTER_VAR`]).
    pub filter: Option<String>,
    /// Log panics and write `crash-<session>.json` beside the crash marker.
    pub panic_hook: bool,
    /// The random id of this run, written into crash files.
    pub session: String,
}

impl LogConfig {
    /// No output at all and no global state: for tests and tools that bring their own subscriber.
    #[must_use]
    pub fn off() -> Self {
        Self {
            file: false,
            stderr: false,
            level: None,
            filter: None,
            panic_hook: false,
            session: String::new(),
        }
    }

    /// The file layer and the panic hook, which is what the desktop shell uses.
    #[must_use]
    pub fn file_only() -> Self {
        Self {
            file: true,
            panic_hook: true,
            ..Self::off()
        }
    }

    /// True when nothing would be installed.
    #[must_use]
    pub fn is_off(&self) -> bool {
        !self.file && !self.stderr && !self.panic_hook
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self::file_only()
    }
}

/// Keeps the non-blocking writer alive; dropping it flushes the file.
#[derive(Debug)]
pub struct LogGuard {
    worker: Option<WorkerGuard>,
    installed: bool,
    dir: Utf8PathBuf,
}

impl LogGuard {
    /// A guard for a context that logs nothing.
    #[must_use]
    pub fn none(dir: &Utf8Path) -> Self {
        Self {
            worker: None,
            installed: false,
            dir: dir.to_owned(),
        }
    }

    /// True when this call installed the global subscriber.
    #[must_use]
    pub fn installed(&self) -> bool {
        self.installed
    }

    /// True when a file writer is attached.
    #[must_use]
    pub fn has_file(&self) -> bool {
        self.worker.is_some()
    }

    /// The folder of the log files.
    #[must_use]
    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }
}

/// The filter text for a configuration: the explicit filter, else `warn` for dependencies and the
/// configured level for `rimstudio_*`.
#[must_use]
pub fn filter_text(config: &LogConfig) -> String {
    if let Some(explicit) = config.filter.as_ref().filter(|f| !f.trim().is_empty()) {
        return explicit.clone();
    }
    let level = match config.level.unwrap_or(LogLevel::Info) {
        LogLevel::Error => "error",
        LogLevel::Warn => "warn",
        LogLevel::Info => "info",
        LogLevel::Debug => "debug",
        LogLevel::Trace => "trace",
    };
    let mut text = String::from("warn");
    for name in OUR_CRATES {
        text.push_str(&format!(",{name}={level}"));
    }
    text
}

/// A writer that redacts every chunk before it reaches the inner writer.
///
/// The formatter hands over one complete line per write, so the redactor sees whole paths.
#[derive(Debug)]
pub struct RedactingWriter<W> {
    inner: W,
    redactor: Arc<Redactor>,
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let clean = self.redactor.redact(&text);
        self.inner.write_all(clean.as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// A [`MakeWriter`] that wraps another one in a [`RedactingWriter`].
#[derive(Debug, Clone)]
pub struct RedactingMakeWriter<M> {
    inner: M,
    redactor: Arc<Redactor>,
}

impl<M> RedactingMakeWriter<M> {
    /// Wraps `inner`.
    #[must_use]
    pub fn new(inner: M, redactor: Arc<Redactor>) -> Self {
        Self { inner, redactor }
    }
}

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for RedactingMakeWriter<M> {
    type Writer = RedactingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter {
            inner: self.inner.make_writer(),
            redactor: Arc::clone(&self.redactor),
        }
    }
}

/// Installs the subscriber described by `config` and, when asked, the panic hook.
///
/// A file that cannot be opened degrades to console only logging with a warning on standard error
/// through the subscriber; logging never stops the application.
pub fn init(roots: &DataRoots, config: &LogConfig, redactor: Arc<Redactor>) -> LogGuard {
    let dir = roots.logs.clone();
    if config.is_off() {
        return LogGuard::none(&dir);
    }
    let filter = EnvFilter::try_new(filter_text(config)).unwrap_or_else(|_| EnvFilter::new("info"));
    let mut worker = None;
    let file_layer = if config.file {
        match Builder::new()
            .rotation(Rotation::DAILY)
            .filename_prefix(FILE_PREFIX)
            .filename_suffix(FILE_SUFFIX)
            .max_log_files(MAX_LOG_FILES)
            .build(dir.as_std_path())
        {
            Ok(appender) => {
                let (writer, guard) = tracing_appender::non_blocking(appender);
                worker = Some(guard);
                Some(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_ansi(false)
                        .with_current_span(false)
                        .with_span_list(true)
                        .with_writer(RedactingMakeWriter::new(writer, Arc::clone(&redactor))),
                )
            }
            Err(_) => None,
        }
    } else {
        None
    };
    let stderr_layer = config.stderr.then(|| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(RedactingMakeWriter::new(io::stderr, Arc::clone(&redactor)))
    });
    let installed = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .try_init()
        .is_ok();
    if installed && config.panic_hook {
        install_panic_hook(&dir, &config.session, redactor);
    }
    LogGuard {
        worker,
        installed,
        dir,
    }
}

/// Logs a panic as an error line and writes `crash-<session>.json` into the logs folder, then lets
/// the previous hook run.
fn install_panic_hook(dir: &Utf8Path, session: &str, redactor: Arc<Redactor>) {
    let dir = dir.to_owned();
    let session = session.to_owned();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = crate::error::panic_message(info.payload());
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_owned();
        tracing::error!(target: "panic", thread = %thread, location = %location, "{message}");
        let report = serde_json::json!({
            "schemaVersion": 1,
            "session": session,
            "message": redactor.redact(&message),
            "location": redactor.redact(&location),
            "thread": thread,
        });
        if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
            let path = dir.join(format!("crash-{session}.json"));
            let _ = atomic_write(&path, &bytes);
        }
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filter_names_the_configured_level_for_our_crates() {
        let config = LogConfig {
            level: Some(LogLevel::Debug),
            ..LogConfig::off()
        };
        let text = filter_text(&config);
        assert!(text.starts_with("warn,"));
        assert!(text.contains("rimstudio_app=debug"));
        assert!(EnvFilter::try_new(&text).is_ok());
    }

    #[test]
    fn an_explicit_filter_wins() {
        let config = LogConfig {
            filter: Some("rimstudio_io=trace".to_owned()),
            level: Some(LogLevel::Error),
            ..LogConfig::off()
        };
        assert_eq!(filter_text(&config), "rimstudio_io=trace");
    }

    #[test]
    fn a_blank_explicit_filter_is_ignored() {
        let config = LogConfig {
            filter: Some("  ".to_owned()),
            ..LogConfig::off()
        };
        assert!(filter_text(&config).starts_with("warn,"));
    }

    #[test]
    fn an_off_config_installs_nothing() {
        let roots = DataRoots::under_base(Utf8Path::new("/nonexistent/rs_base"));
        let guard = init(&roots, &LogConfig::off(), Arc::new(Redactor::new()));
        assert!(!guard.installed());
        assert!(!guard.has_file());
    }

    #[test]
    fn the_redacting_writer_removes_the_home_folder_before_writing() {
        let redactor = Arc::new(Redactor::new().with_home("/home/rs_person"));
        let make = RedactingMakeWriter::new(SharedBuf::default(), redactor);
        let mut w = make.make_writer();
        let line = br#"{"msg":"opened /home/rs_person/mods/RS_Mod"}"#;
        assert_eq!(w.write(line).unwrap_or(0), line.len());
        let text = make.inner.text();
        assert!(!text.contains("rs_person"), "{text}");
        assert!(text.contains("RS_Mod"));
    }

    #[derive(Debug, Clone, Default)]
    struct SharedBuf(Arc<std::sync::Mutex<Vec<u8>>>);

    impl SharedBuf {
        fn text(&self) -> String {
            let guard = self.0.lock().unwrap_or_else(|p| p.into_inner());
            String::from_utf8_lossy(&guard).into_owned()
        }
    }

    impl Write for SharedBuf {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut guard = self.0.lock().unwrap_or_else(|p| p.into_inner());
            guard.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for SharedBuf {
        type Writer = SharedBuf;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }
}
