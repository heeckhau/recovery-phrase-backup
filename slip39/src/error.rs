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
}

impl From<getrandom::Error> for Error {
    fn from(_: getrandom::Error) -> Self {
        Error::Random
    }
}
