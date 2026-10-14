use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("expected 64 hex characters (32 bytes), got {0}")]
    InvalidHexLength(usize),
    #[error("input contains a character that is not valid hex")]
    InvalidHexChar,
    #[error("WIF checksum does not match; check for a typo")]
    InvalidWifChecksum,
    #[error("decoded WIF payload has an unexpected length")]
    InvalidWifLength,
    #[error("unrecognized WIF version byte: 0x{0:02x}")]
    UnknownWifVersionByte(u8),
    #[error("input is not valid hex or a valid Base58Check-encoded WIF key")]
    UnrecognizedFormat,
    #[error("private key value is not a valid secp256k1 scalar (zero or ≥ curve order)")]
    InvalidScalarRange,
    #[error("invalid recovery phrase: {0}")]
    InvalidPhrase(String),
    #[error("a BIP39 phrase encodes 16, 20, 24, 28 or 32 bytes of entropy, got {0}")]
    InvalidPhraseEntropyLength(usize),
    #[error("this looks like a recovery phrase, not a private key. A phrase is backed up as a whole: see `pkc slip39 split`")]
    InputIsAPhrase,
}
