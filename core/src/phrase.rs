// BIP39 recovery phrases (the words a wallet shows as its backup) at
// every standard length (12, 15, 18, 21, 24 words), English wordlist only.
//
// A phrase's *entropy* is what SLIP-39 backs up: shares recover the
// entropy, and the entropy gives back the exact words. Wallets don't use
// the entropy directly; they stretch the phrase (plus an optional BIP39
// passphrase, the "25th word") into a 512-bit seed and derive keys from it
// with BIP32. That path is only used here to *recognise* a phrase: its
// BIP32 fingerprint and its first Ethereum and Bitcoin addresses.
use bip32::{ChildNumber, XPrv};
use bip39::{Language, Mnemonic};
use zeroize::Zeroizing;

use crate::btc::btc_p2wpkh_address;
use crate::error::Error;
use crate::eth::eth_address;
use crate::key::{Network, PrivateKey};

/// A checksum-valid English BIP39 phrase.
pub struct Phrase(Mnemonic);

/// The first receiving addresses a phrase's wallet would show, for
/// recognising it on screen. Never printed on a share card: an address
/// links a card to funds anyone can look up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Addresses {
    /// m/44'/60'/0'/0/0: the first account in MetaMask and most Ethereum wallets.
    pub eth: String,
    /// m/84'/0'/0'/0/0: the first native SegWit (`bc1q…`) receiving address.
    pub btc: String,
}

/// Why a phrase deserves a second look before it is backed up. Not errors:
/// these phrases are valid, and backing one up is the user's call: but
/// each almost always means a test or toy phrase, which anyone can sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weakness {
    /// One of the official BIP39 test vectors (trezor/python-mnemonic
    /// vectors.json), published in every wallet's test suite.
    KnownTestPhrase,
    /// The entropy is a single byte repeated ("abandon … about",
    /// "zoo … wrong", "legal winner …"), never the output of a real
    /// random generator.
    RepeatedByte,
    /// Fewer than half the words are distinct, e.g. Hardhat's
    /// "test test … junk".
    FewDistinctWords,
}

/// Entropy of the official BIP39 test vectors that aren't a single
/// repeated byte (those are caught by [`Weakness::RepeatedByte`]).
const KNOWN_TEST_ENTROPY: [&str; 12] = [
    "9e885d952ad362caeb4efe34a8e91bd2",
    "6610b25967cdcca9d59875f5cb50b0ea75433311869e930b",
    "68a79eaca2324873eacc50cb9c6eca8cc68ea5d936f98787c60c7ebc74e6ce7c",
    "c0ba5a8e914111210f2bd131f3d5e08d",
    "6d9be1ee6ebd27a258115aad99b7317b9c8d28b6d76431c3",
    "9f6a2878b2520799a44ef18bc7df394e7061a224d2c33cd015b157d746869863",
    "23db8160a31d3e0dca3688ed941adbf3",
    "8197a4a47f0425faeaa69deebc05ca29c0a5b5cc76ceacc0",
    "066dca1a2bb7e8a1db2832148ce9933eea0f3ac9548d793112d9a95c9407efad",
    "f30f8c1da665478f49b001d94c5fc452",
    "c10ec20dc3cd9f652c7fac2f1230f7a3c828389a14392f05",
    "f585c11aec520db57dd353c69554b21a89b20fb0650966fa0a9d6f74fd989d8f",
];

const HARDENED: bool = true;
const ETH_PATH: [(u32, bool); 5] = [(44, HARDENED), (60, HARDENED), (0, HARDENED), (0, false), (0, false)];
const BTC_PATH: [(u32, bool); 5] = [(84, HARDENED), (0, HARDENED), (0, HARDENED), (0, false), (0, false)];

/// The 2048-word English BIP39 wordlist, in order: e.g. for autocomplete.
pub fn bip39_wordlist() -> &'static [&'static str] {
    Language::English.word_list()
}

impl Phrase {
    /// Parses and validates a phrase: known words, a standard length, and
    /// a matching checksum. Case and extra whitespace are ignored: phones
    /// and Safari capitalise the first word of a text field.
    pub fn parse(phrase: &str) -> Result<Self, Error> {
        let normalized = phrase.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        let normalized = Zeroizing::new(normalized);
        Mnemonic::parse_in(Language::English, normalized.as_str())
            .map(Phrase)
            .map_err(|e| Error::InvalidPhrase(e.to_string()))
    }

    /// The phrase for 16, 20, 24, 28 or 32 bytes of entropy.
    pub fn from_entropy(entropy: &[u8]) -> Result<Self, Error> {
        Mnemonic::from_entropy_in(Language::English, entropy)
            .map(Phrase)
            .map_err(|_| Error::InvalidPhraseEntropyLength(entropy.len()))
    }

    pub fn word_count(&self) -> usize {
        self.0.word_count()
    }

    /// The words, space-separated.
    pub fn to_words(&self) -> Zeroizing<String> {
        Zeroizing::new(self.0.to_string())
    }

    /// The entropy the words encode: what a SLIP-39 backup of this phrase
    /// splits.
    pub fn entropy(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(self.0.to_entropy())
    }

    /// Reasons to question this phrase before backing it up; empty for a
    /// phrase that looks like real wallet output.
    pub fn weaknesses(&self) -> Vec<Weakness> {
        let entropy = self.entropy();
        let mut found = Vec::new();
        if KNOWN_TEST_ENTROPY.contains(&hex::encode(entropy.as_slice()).as_str()) {
            found.push(Weakness::KnownTestPhrase);
        }
        if entropy.iter().all(|&b| b == entropy[0]) {
            found.push(Weakness::RepeatedByte);
        }
        let words = self.to_words();
        let all: Vec<&str> = words.split(' ').collect();
        let mut distinct = all.clone();
        distinct.sort_unstable();
        distinct.dedup();
        if distinct.len() * 2 < all.len() {
            found.push(Weakness::FewDistinctWords);
        }
        found
    }

    fn master_key(&self, bip39_passphrase: &str) -> XPrv {
        let seed = Zeroizing::new(self.0.to_seed(bip39_passphrase));
        XPrv::new(&*seed).expect("a 64-byte seed is always a valid BIP32 master key input")
    }

    /// The BIP32 master-key fingerprint (first 4 bytes of hash160 of the
    /// master public key), with an empty BIP39 passphrase: the value printed
    /// on every share card. Wallets such as Sparrow and Coldcard display the
    /// same value.
    pub fn fingerprint(&self) -> [u8; 4] {
        self.master_key("").public_key().fingerprint()
    }

    /// The first Ethereum and Bitcoin addresses for this phrase, under
    /// `bip39_passphrase` ("" for none). The passphrase changes every
    /// derived key, so a user who has one must enter it to see their real
    /// addresses: it is not part of any SLIP-39 backup of the phrase.
    pub fn addresses(&self, bip39_passphrase: &str) -> Addresses {
        let master = self.master_key(bip39_passphrase);
        let eth_key = derive(&master, &ETH_PATH);
        let btc_key = derive(&master, &BTC_PATH);
        Addresses { eth: eth_address(&eth_key), btc: btc_p2wpkh_address(&btc_key, Network::Mainnet) }
    }
}

fn derive(master: &XPrv, path: &[(u32, bool)]) -> PrivateKey {
    let mut key = master.clone();
    for &(index, hardened) in path {
        let child = ChildNumber::new(index, hardened).expect("path indices are below 2^31");
        // Fails only with probability ~2^-127 per step (BIP32: the derived
        // scalar is zero or ≥ the curve order); real wallets don't handle it
        // either.
        key = key.derive_child(child).expect("BIP32 child derivation");
    }
    PrivateKey::from_bytes(key.to_bytes()).expect("a BIP32-derived key is a valid secp256k1 scalar")
}

#[cfg(test)]
mod tests {
    use super::*;

    // The canonical all-zero-entropy test phrase, used across BIP32/39/44/84
    // test vectors and wallet test suites.
    const ABANDON_ABOUT: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    #[test]
    fn known_phrase_gives_known_fingerprint_and_addresses() {
        let phrase = Phrase::parse(ABANDON_ABOUT).unwrap();
        // Master fingerprint as used in BIP-84 / Bitcoin Core descriptor docs.
        assert_eq!(hex::encode(phrase.fingerprint()), "73c5da0a");
        let addresses = phrase.addresses("");
        // BIP-84's own test vector for m/84'/0'/0'/0/0.
        assert_eq!(addresses.btc, "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu");
        // The first MetaMask/ethers account for this phrase.
        assert_eq!(addresses.eth, "0x9858EfFD232B4033E47d90003D41EC34EcaEda94");
    }

    #[test]
    fn bip39_passphrase_changes_the_addresses_but_not_the_fingerprint() {
        let phrase = Phrase::parse(ABANDON_ABOUT).unwrap();
        assert_ne!(phrase.addresses("TREZOR"), phrase.addresses(""));
        // The fingerprint is defined with an empty passphrase, so it
        // identifies the words alone.
        assert_eq!(hex::encode(phrase.fingerprint()), "73c5da0a");
    }

    #[test]
    fn every_standard_length_round_trips_through_entropy() {
        for len in [16, 20, 24, 28, 32] {
            let entropy: Vec<u8> = (0..len as u8).map(|i| i.wrapping_mul(29).wrapping_add(7)).collect();
            let phrase = Phrase::from_entropy(&entropy).unwrap();
            assert_eq!(phrase.word_count(), len * 3 / 4);
            let reparsed = Phrase::parse(&phrase.to_words()).unwrap();
            assert_eq!(*reparsed.entropy(), entropy);
        }
    }

    #[test]
    fn flags_test_and_toy_phrases_but_not_real_looking_ones() {
        let weak = |p: &str| Phrase::parse(p).unwrap().weaknesses();
        assert!(weak(ABANDON_ABOUT).contains(&Weakness::RepeatedByte));
        assert!(weak(ABANDON_ABOUT).contains(&Weakness::FewDistinctWords));
        // Official vector 9e885d95…, and Hardhat's default development phrase.
        assert_eq!(
            weak("ozone drill grab fiber curtain grace pudding thank cruise elder eight picnic"),
            vec![Weakness::KnownTestPhrase]
        );
        assert_eq!(weak("test test test test test test test test test test test junk"), vec![Weakness::FewDistinctWords]);
        // A phrase from random entropy (entropy bytes chosen by hand, not a published vector).
        let random_looking = Phrase::from_entropy(&[
            0x3c, 0x91, 0x0e, 0xd7, 0x52, 0xa8, 0x6f, 0x14, 0xb3, 0x29, 0xe0, 0x7d, 0x45, 0x9a, 0xc6, 0x18,
        ])
        .unwrap();
        assert!(random_looking.weaknesses().is_empty());
    }

    #[test]
    fn every_official_non_pattern_vector_is_flagged() {
        for hex_entropy in KNOWN_TEST_ENTROPY {
            let phrase = Phrase::from_entropy(&hex::decode(hex_entropy).unwrap()).unwrap();
            assert!(phrase.weaknesses().contains(&Weakness::KnownTestPhrase), "{hex_entropy}");
        }
    }

    #[test]
    fn tolerates_capitalisation_and_extra_whitespace() {
        let messy = format!("  Abandon {}\n", ABANDON_ABOUT["abandon ".len()..].replace(' ', "   "));
        assert_eq!(*Phrase::parse(&messy).unwrap().to_words(), ABANDON_ABOUT);
    }

    #[test]
    fn rejects_bad_checksum_unknown_words_and_odd_lengths() {
        let bad_checksum = ABANDON_ABOUT.replace("about", "abandon");
        assert!(matches!(Phrase::parse(&bad_checksum), Err(Error::InvalidPhrase(_))));
        assert!(matches!(Phrase::parse("abandon notaword"), Err(Error::InvalidPhrase(_))));
        let eleven = ABANDON_ABOUT.rsplit_once(' ').unwrap().0;
        assert!(matches!(Phrase::parse(eleven), Err(Error::InvalidPhrase(_))));
        assert_eq!(Phrase::from_entropy(&[0u8; 17]).err(), Some(Error::InvalidPhraseEntropyLength(17)));
    }
}
