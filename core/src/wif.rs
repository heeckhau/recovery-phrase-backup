use crate::error::Error;
use crate::key::{KeyFormat, Network, PrivateKey};

const MAINNET_VERSION: u8 = 0x80;
const TESTNET_VERSION: u8 = 0xef;
const COMPRESSED_FLAG: u8 = 0x01;

/// A private key together with the network/format a WIF encoding told us
/// about it, if the input was a WIF. Raw hex input carries neither.
#[derive(Debug)]
pub struct ParsedKey {
    pub key: PrivateKey,
    pub network: Option<Network>,
    pub format: Option<KeyFormat>,
}

/// Parses a raw 32-byte hex private key (optionally `0x`-prefixed) or a
/// Base58Check-encoded WIF private key, validating the WIF checksum (the
/// legacy tool this replaces didn't). Words are rejected as a recovery
/// phrase: see [`crate::Phrase`].
pub fn parse_private_key(input: &str) -> Result<ParsedKey, Error> {
    let trimmed = input.trim();

    // Words are a recovery phrase, never a raw key: a phrase derives many
    // keys through BIP32, so treating its words as one key would be wrong
    // in a way that looks plausible.
    if trimmed.split_whitespace().count() > 1 {
        return Err(Error::InputIsAPhrase);
    }

    let hex_candidate = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);

    if !hex_candidate.is_empty() && hex_candidate.chars().all(|c| c.is_ascii_hexdigit()) {
        if hex_candidate.len() != 64 {
            return Err(Error::InvalidHexLength(hex_candidate.len()));
        }
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(hex_candidate, &mut bytes).map_err(|_| Error::InvalidHexChar)?;
        let key = PrivateKey::from_bytes(bytes)?;
        return Ok(ParsedKey { key, network: None, format: None });
    }

    let decoded = bs58::decode(trimmed)
        .with_check(None)
        .into_vec()
        .map_err(|e| match e {
            bs58::decode::Error::InvalidChecksum { .. } => Error::InvalidWifChecksum,
            _ => Error::UnrecognizedFormat,
        })?;

    let (format, key_bytes): (KeyFormat, &[u8]) = match decoded.len() {
        33 => (KeyFormat::Uncompressed, &decoded[1..33]),
        34 if decoded[33] == COMPRESSED_FLAG => (KeyFormat::Compressed, &decoded[1..33]),
        _ => return Err(Error::InvalidWifLength),
    };

    let network = match decoded[0] {
        MAINNET_VERSION => Network::Mainnet,
        TESTNET_VERSION => Network::Testnet,
        other => return Err(Error::UnknownWifVersionByte(other)),
    };

    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(key_bytes);
    let key = PrivateKey::from_bytes(bytes)?;

    Ok(ParsedKey { key, network: Some(network), format: Some(format) })
}

/// Encodes a private key as a Base58Check WIF for the given network/format.
pub fn encode_wif(key: &PrivateKey, network: Network, format: KeyFormat) -> String {
    let version = match network {
        Network::Mainnet => MAINNET_VERSION,
        Network::Testnet => TESTNET_VERSION,
    };

    let mut payload = Vec::with_capacity(34);
    payload.push(version);
    payload.extend_from_slice(&key.to_bytes());
    if format == KeyFormat::Compressed {
        payload.push(COMPRESSED_FLAG);
    }

    bs58::encode(payload).with_check().into_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Private key = 1, the standard textbook test vector.
    const WIF_UNCOMPRESSED: &str = "5HpHagT65TZzG1PH3CSu63k8DbpvD8s5ip4nEB3kEsreAnchuDf";
    const WIF_COMPRESSED: &str = "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn";
    const WIF_TESTNET_COMPRESSED: &str = "cMahea7zqjxrtgAbB7LSGbcQUr1uX1ojuat9jZodMN87JcbXMTcA";

    fn key_one_bytes() -> [u8; 32] {
        let mut b = [0u8; 32];
        b[31] = 1;
        b
    }

    #[test]
    fn parses_raw_hex() {
        let hex = hex::encode(key_one_bytes());
        let parsed = parse_private_key(&hex).unwrap();
        assert_eq!(parsed.key.to_bytes(), key_one_bytes());
        assert_eq!(parsed.network, None);
        assert_eq!(parsed.format, None);
    }

    #[test]
    fn parses_raw_hex_with_0x_prefix() {
        let hex = format!("0x{}", hex::encode(key_one_bytes()));
        let parsed = parse_private_key(&hex).unwrap();
        assert_eq!(parsed.key.to_bytes(), key_one_bytes());
    }

    #[test]
    fn rejects_wrong_length_hex() {
        let err = parse_private_key("abcdef").unwrap_err();
        assert_eq!(err, Error::InvalidHexLength(6));
    }

    #[test]
    fn parses_uncompressed_wif() {
        let parsed = parse_private_key(WIF_UNCOMPRESSED).unwrap();
        assert_eq!(parsed.key.to_bytes(), key_one_bytes());
        assert_eq!(parsed.network, Some(Network::Mainnet));
        assert_eq!(parsed.format, Some(KeyFormat::Uncompressed));
    }

    #[test]
    fn parses_compressed_wif() {
        let parsed = parse_private_key(WIF_COMPRESSED).unwrap();
        assert_eq!(parsed.key.to_bytes(), key_one_bytes());
        assert_eq!(parsed.network, Some(Network::Mainnet));
        assert_eq!(parsed.format, Some(KeyFormat::Compressed));
    }

    #[test]
    fn parses_testnet_compressed_wif() {
        let parsed = parse_private_key(WIF_TESTNET_COMPRESSED).unwrap();
        assert_eq!(parsed.key.to_bytes(), key_one_bytes());
        assert_eq!(parsed.network, Some(Network::Testnet));
        assert_eq!(parsed.format, Some(KeyFormat::Compressed));
    }

    #[test]
    fn rejects_flipped_checksum_character() {
        let mut chars: Vec<char> = WIF_COMPRESSED.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == 'n' { 'm' } else { 'n' };
        let tampered: String = chars.into_iter().collect();
        assert_eq!(parse_private_key(&tampered).unwrap_err(), Error::InvalidWifChecksum);
    }

    #[test]
    fn round_trips_through_encode_wif() {
        let key = PrivateKey::from_bytes(key_one_bytes()).unwrap();
        assert_eq!(encode_wif(&key, Network::Mainnet, KeyFormat::Uncompressed), WIF_UNCOMPRESSED);
        assert_eq!(encode_wif(&key, Network::Mainnet, KeyFormat::Compressed), WIF_COMPRESSED);
        assert_eq!(encode_wif(&key, Network::Testnet, KeyFormat::Compressed), WIF_TESTNET_COMPRESSED);
    }
}
