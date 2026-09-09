use k256::elliptic_curve::sec1::ToSec1Point;
use sha3::{Digest, Keccak256};

use crate::key::PrivateKey;

/// Derives the Ethereum address for a private key, EIP-55 checksum-cased.
pub fn eth_address(key: &PrivateKey) -> String {
    let public_key = key.to_secret_key().public_key();
    let uncompressed = public_key.to_sec1_point(false); // 0x04 || X(32) || Y(32)
    let hash = Keccak256::digest(&uncompressed.as_bytes()[1..]); // hash X||Y only
    let lower_hex = hex::encode(&hash[12..]); // last 20 bytes

    eip55_checksum(&lower_hex)
}

/// Applies EIP-55 mixed-case checksum encoding to a 40-char lowercase hex
/// address (no `0x` prefix): keccak256 the address *string* itself, and
/// uppercase each hex letter whose corresponding hash nibble is >= 8.
fn eip55_checksum(lower_hex: &str) -> String {
    let hash = Keccak256::digest(lower_hex.as_bytes());

    let mut out = String::with_capacity(2 + lower_hex.len());
    out.push_str("0x");
    for (i, c) in lower_hex.chars().enumerate() {
        if c.is_ascii_digit() {
            out.push(c);
            continue;
        }
        let byte = hash[i / 2];
        let nibble = if i % 2 == 0 { byte >> 4 } else { byte & 0x0f };
        out.push(if nibble >= 8 { c.to_ascii_uppercase() } else { c });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_one() -> PrivateKey {
        let mut bytes = [0u8; 32];
        bytes[31] = 1;
        PrivateKey::from_bytes(bytes).unwrap()
    }

    #[test]
    fn eip55_casing_matches_known_vector() {
        // Cross-checked against the EIP-55 spec's own worked example.
        assert_eq!(
            eip55_checksum("7e5f4552091a69125d5dfcb7b8c2659029395bdf"),
            "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf"
        );
    }

    #[test]
    fn eth_address_for_key_one() {
        // Raw address independently cross-checked against reference/bitcoin-ethereum-private-key-converter.html's
        // own keccak256 code (js-sha3), run standalone under Node: 0x7e5f4552091a69125d5dfcb7b8c2659029395bdf.
        assert_eq!(eth_address(&key_one()), "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf");
    }
}
