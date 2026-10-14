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
    #[error("word {0} is not in the SLIP-39 wordlist")]
    UnknownWord(usize),
    #[error("checksum does not match; check for a typo")]
    InvalidChecksum,
    #[error("padding bits are not all zero, or the padding is too long (tampered or corrupted share)")]
    InvalidPadding,
    #[error("decoded share value is only {0} bits, must be at least 128")]
    ShareValueTooShort(usize),
    #[error("group count must be between 1 and 16, got {0}")]
    InvalidGroupCount(usize),
    #[error("group threshold must be between 1 and {group_count} (the number of groups), got {group_threshold}")]
    InvalidGroupThreshold { group_threshold: u8, group_count: u8 },
    #[error("group {group_index}: member count must be between 1 and 16, got {count}")]
    InvalidMemberCount { group_index: u8, count: u8 },
    #[error("group {group_index}: member threshold must be between 1 and {member_count}, got {member_threshold} (a threshold of 1 also requires exactly 1 member)")]
    InvalidMemberThreshold { group_index: u8, member_threshold: u8, member_count: u8 },
    #[error("iteration exponent must be between 0 and 15, got {0}")]
    InvalidIterationExponent(u8),
    #[error("passphrase contains a character that is not printable ASCII (code point {0})")]
    InvalidPassphrase(u32),
    #[error("shares have different {field}s, so they aren't all from the same backup")]
    MismatchedShares { field: &'static str },
    #[error("group {group_index}: shares disagree on the member threshold")]
    MismatchedMemberThreshold { group_index: u8 },
    #[error("group {group_index}: two shares have the same member index {member_index}")]
    DuplicateMemberIndex { group_index: u8, member_index: u8 },
    #[error("need shares from at least {need} distinct groups, got {have}")]
    NotEnoughGroups { have: u8, need: u8 },
    #[error("group {group_index}: need at least {need} member shares, got {have}")]
    NotEnoughMemberShares { group_index: u8, have: u8, need: u8 },
    #[error("no shares provided")]
    NoShares,
}

/// The shortest possible valid SLIP-39 mnemonic: a 128-bit share value
/// (20 words total; see the spec's share-length table).
pub const MIN_WORD_COUNT: usize = 20;

impl From<getrandom::Error> for Error {
    fn from(_: getrandom::Error) -> Self {
        Error::Random
    }
}
