//! File watching (deferred, not part of 0.1.0).
//!
//! The design calls for `watch::Watcher`: roots only, debounced through `notify` and
//! `notify-debouncer-mini`, with a polling fallback when the operating system runs out of watches
//! (`ENOSPC`). The 0.1.0 slice (a weapons editor and the optional Combat Extended patch generator,
//! driven from the headless CLI) rescans on demand and never watches, so this module is
//! intentionally empty. The dependencies stay declared so the manifest does not change when the
//! watcher lands with the mod manager milestone.
//!
//! Nothing here may be relied upon; the first public item will arrive together with its tests and
//! an update of `docs/architecture/data-and-persistence.md` section 11.
