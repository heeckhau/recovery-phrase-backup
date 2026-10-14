use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

fn js_error(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// What identifies a recovery phrase on screen: its fingerprint (the value
/// printed on every share card) and its first addresses (never printed).
#[wasm_bindgen]
#[derive(Clone)]
pub struct JsPhraseCheck {
    pub word_count: u32,
    /// Why this phrase deserves a second look, as plain-language
    /// sentences; empty for a phrase that looks like real wallet output.
    #[wasm_bindgen(getter_with_clone)]
    pub warnings: Vec<String>,
    #[wasm_bindgen(getter_with_clone)]
    pub fingerprint: String,
    #[wasm_bindgen(getter_with_clone)]
    pub eth_address: String,
    #[wasm_bindgen(getter_with_clone)]
    pub btc_address: String,
}

fn weakness_text(weakness: pkc_core::Weakness) -> &'static str {
    match weakness {
        pkc_core::Weakness::KnownTestPhrase => {
            "This is a published BIP39 test phrase. Anyone can use it, so never keep funds on it."
        }
        pkc_core::Weakness::RepeatedByte => {
            "This phrase follows a simple pattern no real wallet would generate. Anyone can guess it."
        }
        pkc_core::Weakness::FewDistinctWords => {
            "This phrase repeats the same few words. Real wallet phrases almost never do: check you typed your own phrase."
        }
    }
}

/// The 2048-word English BIP39 wordlist, newline-separated, for autocomplete.
#[wasm_bindgen]
pub fn bip39_wordlist() -> String {
    pkc_core::bip39_wordlist().join("\n")
}

/// The 1024-word SLIP-39 wordlist, newline-separated, for autocomplete.
#[wasm_bindgen]
pub fn slip39_wordlist() -> String {
    pkc_slip39::wordlist().join("\n")
}

fn check(phrase: &pkc_core::Phrase, bip39_passphrase: &str) -> JsPhraseCheck {
    let addresses = phrase.addresses(bip39_passphrase);
    JsPhraseCheck {
        word_count: phrase.word_count() as u32,
        warnings: phrase.weaknesses().into_iter().map(weakness_text).map(String::from).collect(),
        fingerprint: hex_encode(&phrase.fingerprint()),
        eth_address: addresses.eth,
        btc_address: addresses.btc,
    }
}

/// Validates a BIP39 recovery phrase and returns what identifies it.
/// `bip39_passphrase` (the "25th word", "" for none) only affects the
/// addresses: it is not part of any backup.
#[wasm_bindgen]
pub fn phrase_check(phrase: &str, bip39_passphrase: &str) -> Result<JsPhraseCheck, JsValue> {
    let phrase = pkc_core::Phrase::parse(phrase).map_err(js_error)?;
    Ok(check(&phrase, bip39_passphrase))
}

/// Splits a BIP39 recovery phrase into SLIP-39 shares of its entropy.
/// `group_specs` is a flat array of alternating `[member_threshold,
/// member_count, ...]` pairs, one pair per group. Returns one string per
/// group: that group's shares joined by `"\n"` (a share itself is
/// space-separated words, so `\n` unambiguously separates them).
#[wasm_bindgen]
pub fn slip39_split(
    phrase: &str,
    group_threshold: u8,
    group_specs: &[u8],
    passphrase: &str,
    iteration_exponent: u8,
) -> Result<Vec<String>, JsValue> {
    let phrase = pkc_core::Phrase::parse(phrase).map_err(js_error)?;

    if !group_specs.len().is_multiple_of(2) {
        return Err(JsValue::from_str("group_specs must be an even-length array of [threshold, count] pairs"));
    }
    let groups: Vec<pkc_slip39::GroupSpec> = group_specs
        .chunks(2)
        .map(|pair| pkc_slip39::GroupSpec { member_threshold: pair[0], member_count: pair[1] })
        .collect();

    let shares = pkc_slip39::split(&phrase.entropy(), group_threshold, &groups, passphrase, iteration_exponent)
        .map_err(js_error)?;

    Ok(shares.into_iter().map(|members| members.join("\n")).collect())
}

/// Which group/member a single SLIP-39 share is, and the thresholds it
/// was split with: all 1-indexed for display. None of this is sensitive;
/// it's decoded straight from the mnemonic's own plaintext structure.
#[wasm_bindgen]
pub struct JsShareInfo {
    pub group_number: u32,
    pub group_count: u32,
    pub group_threshold: u32,
    pub member_number: u32,
    pub member_threshold: u32,
}

/// Identifies a single SLIP-39 mnemonic share (which group/member it is,
/// and the thresholds it needs) without requiring enough shares to
/// recover anything. Useful for labeling a share as it's entered.
#[wasm_bindgen]
pub fn slip39_inspect(mnemonic: &str) -> Result<JsShareInfo, JsValue> {
    let info = pkc_slip39::inspect(mnemonic).map_err(js_error)?;
    Ok(JsShareInfo {
        group_number: info.group_index as u32 + 1,
        group_count: info.group_count as u32,
        group_threshold: info.group_threshold as u32,
        member_number: info.member_index as u32 + 1,
        member_threshold: info.member_threshold as u32,
    })
}

/// What a set of shares recovered. A backup doesn't record what it holds:
/// a phrase-sized secret is returned as a phrase (`words` and `check`), and
/// 32 bytes might instead be a raw private key backed up with the CLI, so
/// that reading is offered too (`raw_key_*`). The fingerprint or address
/// the user recognises tells which one they backed up. `secret_hex` is set
/// only when neither reading applies (e.g. a 512-bit seed from another tool).
#[wasm_bindgen(getter_with_clone)]
pub struct JsRecovered {
    pub words: Option<String>,
    pub check: Option<JsPhraseCheck>,
    pub raw_key_hex: Option<String>,
    pub raw_key_eth_address: Option<String>,
    pub secret_hex: Option<String>,
}

/// Recombines SLIP-39 shares. A wrong `passphrase` is not an error:
/// SLIP-39 decryption is unauthenticated, so it silently returns different
/// bytes. That is why the result carries the fingerprint and addresses
/// for the user to check. `bip39_passphrase` only affects those addresses.
#[wasm_bindgen]
pub fn slip39_combine(mnemonics: Vec<String>, passphrase: &str, bip39_passphrase: &str) -> Result<JsRecovered, JsValue> {
    let refs: Vec<&str> = mnemonics.iter().map(String::as_str).collect();
    let secret = pkc_slip39::combine(&refs, passphrase).map_err(js_error)?;

    let phrase = pkc_core::Phrase::from_entropy(&secret).ok();
    let raw_key = <[u8; 32]>::try_from(secret.as_slice()).ok().and_then(|b| pkc_core::PrivateKey::from_bytes(b).ok());
    let neither = phrase.is_none() && raw_key.is_none();

    Ok(JsRecovered {
        words: phrase.as_ref().map(|p| p.to_words().to_string()),
        check: phrase.as_ref().map(|p| check(p, bip39_passphrase)),
        raw_key_hex: raw_key.as_ref().map(|k| k.to_hex()),
        raw_key_eth_address: raw_key.as_ref().map(pkc_core::eth_address),
        secret_hex: neither.then(|| hex_encode(&secret)),
    })
}
