use bip39::{Language, Mnemonic};

use crate::error::Error;
use crate::key::PrivateKey;

/// Encodes a private key's raw bytes directly as BIP39 entropy, producing a
/// 24-word mnemonic. This is a direct, reversible re-encoding of the key
/// bytes themselves — distinct from (and not compatible with) the
/// "mnemonic -> PBKDF2 seed -> BIP32 HD wallet" pipeline most wallets use,
/// which would derive a *different* key from the same words.
pub fn to_mnemonic(key: &PrivateKey) -> String {
    Mnemonic::from_entropy_in(Language::English, &key.to_bytes())
        .expect("32 bytes is always valid BIP39 entropy (256 bits, in range)")
        .to_string()
}

/// Parses a 24-word BIP39 mnemonic back into a private key, validating its
/// checksum.
pub fn from_mnemonic(phrase: &str) -> Result<PrivateKey, Error> {
    // BIP39 wordlists are all lowercase, but plenty of text fields
    // (mobile keyboards, Safari's autocapitalize) capitalize the first
    // letter of what looks like the start of a sentence — normalize
    // rather than reject what's otherwise a perfectly typed phrase.
    let normalized = phrase.to_lowercase();
    let parsed = Mnemonic::parse_in(Language::English, normalized.as_str())
        .map_err(|e| Error::InvalidMnemonic(e.to_string()))?;

    let word_count = parsed.word_count();
    if word_count != 24 {
        return Err(Error::InvalidMnemonicWordCount(word_count));
    }

    let entropy = parsed.to_entropy();
    let bytes: [u8; 32] = entropy
        .as_slice()
        .try_into()
        .expect("a 24-word mnemonic always decodes to exactly 32 bytes of entropy");

    PrivateKey::from_bytes(bytes)
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
    fn round_trips_through_mnemonic() {
        let phrase = to_mnemonic(&key_one());
        assert_eq!(phrase.split_whitespace().count(), 24);
        let recovered = from_mnemonic(&phrase).unwrap();
        assert_eq!(recovered.to_bytes(), key_one().to_bytes());
    }

    #[test]
    fn tolerates_autocapitalized_first_word() {
        // Safari/mobile keyboards commonly capitalize the first letter of
        // a text field's contents by default (autocapitalize="sentences").
        let phrase = to_mnemonic(&key_one());
        let mut words: Vec<String> = phrase.split_whitespace().map(str::to_string).collect();
        let mut chars: Vec<char> = words[0].chars().collect();
        chars[0] = chars[0].to_ascii_uppercase();
        words[0] = chars.into_iter().collect();
        let capitalized = words.join(" ");

        let recovered = from_mnemonic(&capitalized).unwrap();
        assert_eq!(recovered.to_bytes(), key_one().to_bytes());
    }

    #[test]
    fn rejects_wrong_word_count() {
        // The canonical all-zero-entropy 12-word BIP39 test vector.
        let twelve = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        assert_eq!(from_mnemonic(twelve).unwrap_err(), Error::InvalidMnemonicWordCount(12));
    }

    #[test]
    fn rejects_all_zero_entropy_as_an_invalid_key_even_though_the_mnemonic_itself_is_valid() {
        // The canonical all-zero-entropy 24-word BIP39 test vector. Valid
        // BIP39, but zero isn't a valid secp256k1 scalar, so this must
        // still fail — just at key validation, not mnemonic parsing.
        let all_zero = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon \
                         abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon \
                         abandon art";
        assert_eq!(from_mnemonic(all_zero).unwrap_err(), Error::InvalidScalarRange);
    }

    #[test]
    fn rejects_mutated_word() {
        let phrase = to_mnemonic(&key_one());
        let mut words: Vec<&str> = phrase.split_whitespace().collect();
        words[0] = if words[0] == "zoo" { "abandon" } else { "zoo" };
        let mutated = words.join(" ");
        assert!(from_mnemonic(&mutated).is_err());
    }
}
