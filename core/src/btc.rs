use k256::elliptic_curve::sec1::ToSec1Point;
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};

use crate::key::{KeyFormat, Network, PrivateKey};

const MAINNET_P2PKH_VERSION: u8 = 0x00;
const TESTNET_P2PKH_VERSION: u8 = 0x6f;

fn hash160(data: &[u8]) -> [u8; 20] {
    let sha256 = Sha256::digest(data);
    Ripemd160::digest(sha256).into()
}

/// Derives the Bitcoin P2PKH address for a private key on the given
/// network, using the compressed or uncompressed public key encoding.
pub fn btc_p2pkh_address(key: &PrivateKey, network: Network, format: KeyFormat) -> String {
    let compressed = format == KeyFormat::Compressed;
    let public_key = key.to_secret_key().public_key();
    let encoded_point = public_key.to_sec1_point(compressed);

    let version = match network {
        Network::Mainnet => MAINNET_P2PKH_VERSION,
        Network::Testnet => TESTNET_P2PKH_VERSION,
    };

    let mut payload = Vec::with_capacity(21);
    payload.push(version);
    payload.extend_from_slice(&hash160(encoded_point.as_bytes()));

    bs58::encode(payload).with_check().into_string()
}

/// Derives the Bitcoin native SegWit (P2WPKH, BIP-84 style `bc1q…`) address
/// for a private key: witness version 0 over hash160 of the compressed
/// public key, bech32-encoded.
pub fn btc_p2wpkh_address(key: &PrivateKey, network: Network) -> String {
    let public_key = key.to_secret_key().public_key();
    let program = hash160(public_key.to_sec1_point(true).as_bytes());
    let hrp = match network {
        Network::Mainnet => bech32::hrp::BC,
        Network::Testnet => bech32::hrp::TB,
    };
    bech32::segwit::encode_v0(hrp, &program).expect("a 20-byte v0 witness program always encodes")
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
    fn uncompressed_mainnet_address_for_key_one() {
        let addr = btc_p2pkh_address(&key_one(), Network::Mainnet, KeyFormat::Uncompressed);
        assert_eq!(addr, "1EHNa6Q4Jz2uvNExL497mE43ikXhwF6kZm");
    }

    #[test]
    fn compressed_mainnet_address_for_key_one() {
        let addr = btc_p2pkh_address(&key_one(), Network::Mainnet, KeyFormat::Compressed);
        assert_eq!(addr, "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH");
    }

    #[test]
    fn compressed_testnet_address_for_key_one() {
        let addr = btc_p2pkh_address(&key_one(), Network::Testnet, KeyFormat::Compressed);
        assert_eq!(addr, "mrCDrCybB6J1vRfbwM5hemdJz73FwDBC8r");
    }

    #[test]
    fn native_segwit_address_for_key_one() {
        // BIP-173's own P2WPKH example is the key-one public key.
        let addr = btc_p2wpkh_address(&key_one(), Network::Mainnet);
        assert_eq!(addr, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");
    }
}
