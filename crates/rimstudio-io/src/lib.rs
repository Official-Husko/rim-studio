//! JSON and JSONC storage, atomic writes, backups, migrations, data roots, walking and the write fence.
//!
//! Layer `l1-infra`. See docs/architecture/crate-catalog.md for the contract of this crate. This is
//! the only crate that writes app data, and the only one that may write under the game folders
//! (through [`fence::GameWriteFence`]).
//!
//! | Module | Purpose |
//! |---|---|
//! | [`error`] | [`IoError`] with [`StoreError`], [`MigrateError`], [`GuardError`] and stable codes |
//! | [`roots`] | [`DataRoots`] (config, data, cache, logs) and portable mode |
//! | [`atomic`] | atomic file replacement and the [`atomic::FsOps`] crash injection seam |
//! | [`backup`] | verified backups with retention under an injected clock |
//! | [`schema`], [`migrate`] | [`Versioned`] types, forward only migrations, load results |
//! | [`jsonc`] | JSONC reading, comment preserving CST edits (spike S-02 findings), new file writer |
//! | [`store`] | [`Store`]: one typed JSONC file (settings) with migration, backup and verified writes |
//! | [`collection`] | [`Collection`]: the in house JSON document database (no SQL) |
//! | [`walk`], [`statkey`] | bounded parallel directory scans and change detection keys |
//! | [`guard`], [`fence`] | path validation against roots and the game write fence (I-05) |
//! | [`real_fs`] | [`RealFs`], the real [`rimstudio_core::ports::FsProbe`] |
//! | [`watch`], [`ignore_set`] | deferred, not part of 0.1.0 |
//!
//! Operating system conditionals do not belong here (I-11): where an identity such as an inode is
//! needed, a hook ([`statkey::FileIdFn`]) is injected by `rimstudio-platform`.

pub mod atomic;
pub mod backup;
pub mod collection;
pub mod error;
pub mod fence;
pub mod guard;
pub mod ignore_set;
pub mod jsonc;
pub mod migrate;
pub mod real_fs;
pub mod roots;
pub mod schema;
pub mod statkey;
pub mod store;
pub mod walk;
pub mod watch;

pub use atomic::atomic_write;
pub use collection::Collection;
pub use error::{GuardError, IoError, MigrateError, StoreError};
pub use fence::GameWriteFence;
pub use guard::RootGuard;
pub use real_fs::RealFs;
pub use roots::{DataRoots, RootKind};
pub use schema::{Loaded, Versioned};
pub use statkey::StatKey;
pub use store::Store;
