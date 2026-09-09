use thiserror::Error as ThisError;

#[derive(Debug, ThisError, PartialEq, Eq)]
pub enum Error {
    #[error("threshold must be between 1 and {share_count} (the share count), got {threshold}")]
    InvalidThreshold { threshold: u8, share_count: u8 },
    #[error("share count must be between 1 and 16, got {0}")]
    InvalidShareCount(u8),
    #[error("secret must be at least 128 bits and a multiple of 16 bits, got {0} bits")]
    InvalidSecretLength(usize),
    #[error("a system random number generator call failed")]
    Random,
    #[error("shares do not reconstruct a consistent secret (wrong shares, wrong threshold, or corrupted data)")]
    ShareSetInconsistent,
    #[error("expected at least {MIN_WORD_COUNT} words, got {0}")]
    InvalidMnemonicWordCount(usize),
    #[error("\"{0}\" is not a word in the SLIP-39 wordlist")]
    UnknownWord(String),
    #[error("checksum does not match — check for a typo")]
    InvalidChecksum,
    #[error("padding bits are not all zero, or the padding is too long (tampered or corrupted share)")]
    InvalidPadding,
    #[error("decoded share value is only {0} bits, must be at least 128")]
    ShareValueTooShort(usize),
}

/// The shortest possible valid SLIP-39 mnemonic: a 128-bit share value
/// (20 words total; see the spec's share-length table).
pub const MIN_WORD_COUNT: usize = 20;

impl From<getrandom::Error> for Error {
    fn from(_: getrandom::Error) -> Self {
        Error::Random
    }
}
