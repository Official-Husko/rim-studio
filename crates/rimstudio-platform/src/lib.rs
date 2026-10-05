//! Operating system adapters behind core ports.
//!
//! Layer `l1-infra`. This is the only crate allowed to use `cfg(target_os)` and operating system
//! specific dependencies (invariant I-11). [`SystemPlatform`] implements the ports of
//! `rimstudio_core::ports` that touch the OS: environment, registry, processes, directory links,
//! launching, credentials, sandbox and install source detection. Decisions that can be made from
//! plain data (directory resolution, install source, sandbox, command construction, output parsing)
//! are pure functions so tests drive them with injected environments on every operating system.
//!
//! The folders `windows/`, `macos/`, `linux/`, `unix/` and `generic/` hold the code that is specific
//! to one family; each module compiles on every operating system where it can, and the calls that
//! cannot are guarded inside the module.

pub mod dirs;
pub mod error;
pub mod generic;
pub mod install;
pub mod launch;
pub mod linux;
pub mod macos;
pub mod process;
pub mod sandbox;
pub mod system;
pub mod unix;
pub mod windows;

pub use error::{PlatformError, PlatformResult};
pub use system::SystemPlatform;
