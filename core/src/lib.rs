mod btc;
mod error;
mod eth;
mod key;
mod phrase;
mod wif;

pub use btc::{btc_p2pkh_address, btc_p2wpkh_address};
pub use error::Error;
pub use eth::eth_address;
pub use key::{KeyFormat, Network, PrivateKey};
pub use phrase::{Addresses, Phrase};
pub use wif::{encode_wif, parse_private_key, ParsedKey};

/// Every representation of a private key, derived in one call.
#[derive(Debug)]
pub struct ConversionResult {
    pub network: Network,
    pub format: KeyFormat,
    pub private_key_hex: String,
    pub wif: String,
    pub btc_address: String,
    pub eth_address: String,
}

/// Parses `input` (raw hex or WIF) and derives
/// every representation.
///
/// `network`/`format` are only defaults: if `input` is a WIF, the
/// network/format it encodes take precedence, since re-encoding a WIF the
/// user actually gave us under different settings than they specified
/// would be surprising. They only apply when `input` is raw hex, which
/// carries no such metadata.
pub fn convert(input: &str, network: Network, format: KeyFormat) -> Result<ConversionResult, Error> {
    let parsed = parse_private_key(input)?;
    let network = parsed.network.unwrap_or(network);
    let format = parsed.format.unwrap_or(format);

    Ok(ConversionResult {
        network,
        format,
        private_key_hex: parsed.key.to_hex(),
        wif: encode_wif(&parsed.key, network, format),
        btc_address: btc_p2pkh_address(&parsed.key, network, format),
        eth_address: eth_address(&parsed.key),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_from_raw_hex_uses_requested_network_and_format() {
        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let result = convert(hex, Network::Mainnet, KeyFormat::Compressed).unwrap();
        assert_eq!(result.network, Network::Mainnet);
        assert_eq!(result.format, KeyFormat::Compressed);
        assert_eq!(result.private_key_hex, hex);
        assert_eq!(result.wif, "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn");
        assert_eq!(result.btc_address, "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH");
        assert_eq!(result.eth_address, "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf");
    }

    #[test]
    fn convert_from_wif_ignores_requested_network_and_format() {
        // Testnet compressed WIF for key = 1; requested args are the opposite on both axes.
        let wif = "cMahea7zqjxrtgAbB7LSGbcQUr1uX1ojuat9jZodMN87JcbXMTcA";
        let result = convert(wif, Network::Mainnet, KeyFormat::Uncompressed).unwrap();
        assert_eq!(result.network, Network::Testnet);
        assert_eq!(result.format, KeyFormat::Compressed);
        assert_eq!(result.btc_address, "mrCDrCybB6J1vRfbwM5hemdJz73FwDBC8r");
        assert_eq!(result.wif, wif);
    }

    #[test]
    fn convert_refuses_a_recovery_phrase() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        assert!(matches!(convert(phrase, Network::Mainnet, KeyFormat::Compressed), Err(Error::InputIsAPhrase)));
    }

    #[test]
    fn convert_rejects_invalid_input() {
        assert!(convert("not a key", Network::Mainnet, KeyFormat::Compressed).is_err());
    }
}
