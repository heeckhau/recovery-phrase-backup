use zeroize::Zeroize;

use crate::error::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    Mainnet,
    Testnet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyFormat {
    Compressed,
    Uncompressed,
}

/// A raw secp256k1 private key. Zeroized on drop; `Debug` never prints the
/// bytes, so it can't leak into logs by accident.
#[derive(Zeroize)]
#[zeroize(drop)]
pub struct PrivateKey([u8; 32]);

impl PrivateKey {
    /// Validates that `bytes` is a valid secp256k1 scalar (nonzero, less
    /// than the curve order) before accepting it as a private key.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, Error> {
        k256::SecretKey::from_slice(&bytes).map_err(|_| Error::InvalidScalarRange)?;
        Ok(Self(bytes))
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub(crate) fn to_secret_key(&self) -> k256::SecretKey {
        // Already validated in `from_bytes`, so this cannot fail.
        k256::SecretKey::from_slice(&self.0).expect("validated in PrivateKey::from_bytes")
    }
}

impl std::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrivateKey(REDACTED)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_key() {
        assert_eq!(PrivateKey::from_bytes([0u8; 32]).unwrap_err(), Error::InvalidScalarRange);
    }

    #[test]
    fn accepts_key_of_value_one() {
        let mut bytes = [0u8; 32];
        bytes[31] = 1;
        assert!(PrivateKey::from_bytes(bytes).is_ok());
    }

    #[test]
    fn debug_does_not_print_bytes() {
        let mut bytes = [0u8; 32];
        bytes[31] = 1;
        let key = PrivateKey::from_bytes(bytes).unwrap();
        assert_eq!(format!("{key:?}"), "PrivateKey(REDACTED)");
    }
}
