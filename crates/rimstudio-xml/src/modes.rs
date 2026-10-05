//! The two parse modes of the boundary.

/// How strictly a document is read.
///
/// [`ParseMode::Game`] reproduces the game's own loader for Defs and Patches files: the UTF-8 byte
/// order mark is removed, the bytes are decoded as UTF-8 whatever the declaration says, comments
/// and whitespace only text are dropped (except below `xml:space="preserve"`), a document type
/// declaration is refused, illegal characters pass, and any well-formedness error fails the whole
/// file. [`ParseMode::Tolerant`] keeps the same tree shape but recovers from errors and returns a
/// partial tree plus warnings, so a broken file can still be shown and fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ParseMode {
    /// The game's loader: fail where the game fails.
    #[default]
    Game,
    /// Best effort recovery with warnings.
    Tolerant,
}

impl ParseMode {
    /// True for [`ParseMode::Tolerant`].
    #[must_use]
    pub fn is_tolerant(self) -> bool {
        matches!(self, ParseMode::Tolerant)
    }
}
