//! Deployment to the game (deferred beyond release 0.1.0).
//!
//! Planned: a link farm with an ownership manifest (unlink only, never recursive delete), junctions
//! on Windows and symbolic links elsewhere, a copy fallback with a `.rimstudio.json` marker,
//! dry run plans, a pre launch check and the `ModsConfig.xml` write through the game write fence of
//! `rimstudio-io`. Nothing is implemented here on purpose: the library never writes into the game
//! folders in 0.1.0, and a mod found only in a custom folder is reported as
//! [`Loadability::NeedsLink`](crate::index::Loadability::NeedsLink) until a later release links it.
