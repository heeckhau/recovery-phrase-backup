use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Plain-data mirror of `pkc_core::ConversionResult` for JS: network/format
/// are surfaced as strings rather than exposing the Rust enums to wasm-bindgen.
#[wasm_bindgen(getter_with_clone)]
pub struct JsConversionResult {
    pub network: String,
    pub format: String,
    pub private_key_hex: String,
    pub wif: String,
    pub btc_address: String,
    pub eth_address: String,
    pub mnemonic: String,
}

fn network_to_str(network: pkc_core::Network) -> &'static str {
    match network {
        pkc_core::Network::Mainnet => "mainnet",
        pkc_core::Network::Testnet => "testnet",
    }
}

fn format_to_str(format: pkc_core::KeyFormat) -> &'static str {
    match format {
        pkc_core::KeyFormat::Compressed => "compressed",
        pkc_core::KeyFormat::Uncompressed => "uncompressed",
    }
}

/// Converts `input` (raw hex, WIF, or a 24-word BIP39 mnemonic) to every
/// representation. `network` must be `"mainnet"` or `"testnet"`; both are
/// only defaults used when `input` is raw hex or a mnemonic — a WIF's own
/// network/format take precedence.
#[wasm_bindgen]
pub fn convert(input: &str, network: &str, compressed: bool) -> Result<JsConversionResult, JsValue> {
    let network = match network {
        "mainnet" => pkc_core::Network::Mainnet,
        "testnet" => pkc_core::Network::Testnet,
        other => return Err(JsValue::from_str(&format!("unknown network \"{other}\" (expected \"mainnet\" or \"testnet\")"))),
    };
    let format = if compressed { pkc_core::KeyFormat::Compressed } else { pkc_core::KeyFormat::Uncompressed };

    let result = pkc_core::convert(input, network, format).map_err(|e| JsValue::from_str(&e.to_string()))?;

    Ok(JsConversionResult {
        network: network_to_str(result.network).to_string(),
        format: format_to_str(result.format).to_string(),
        private_key_hex: result.private_key_hex,
        wif: result.wif,
        btc_address: result.btc_address,
        eth_address: result.eth_address,
        mnemonic: result.mnemonic,
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Splits a private key (hex, WIF, or BIP39 mnemonic — anything
/// `convert()` accepts) into SLIP-39 mnemonic shares. `group_specs` is a
/// flat array of alternating `[member_threshold, member_count, ...]`
/// pairs, one pair per group. Returns one string per group: that group's
/// member mnemonics joined by `"\n"` (a mnemonic itself is space-joined
/// words, so `\n` unambiguously separates members — split on it in JS to
/// get each group's individual mnemonics).
#[wasm_bindgen]
pub fn slip39_split(
    private_key: &str,
    group_threshold: u8,
    group_specs: &[u8],
    passphrase: &str,
    iteration_exponent: u8,
) -> Result<Vec<String>, JsValue> {
    let key = pkc_core::parse_private_key(private_key).map_err(|e| JsValue::from_str(&e.to_string()))?.key;

    if !group_specs.len().is_multiple_of(2) {
        return Err(JsValue::from_str("group_specs must be an even-length array of [threshold, count] pairs"));
    }
    let groups: Vec<pkc_slip39::GroupSpec> = group_specs
        .chunks(2)
        .map(|pair| pkc_slip39::GroupSpec { member_threshold: pair[0], member_count: pair[1] })
        .collect();

    let secret: [u8; 32] = key.to_bytes();
    let shares = pkc_slip39::split(&secret, group_threshold, &groups, passphrase, iteration_exponent)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    Ok(shares.into_iter().map(|members| members.join("\n")).collect())
}

/// Which group/member a single SLIP-39 share is, and the thresholds it
/// was split with — all 1-indexed for display. None of this is sensitive;
/// it's decoded straight from the mnemonic's own plaintext structure.
#[wasm_bindgen]
pub struct JsShareInfo {
    pub group_number: u32,
    pub group_count: u32,
    pub group_threshold: u32,
    pub member_number: u32,
    pub member_threshold: u32,
}

/// Identifies a single SLIP-39 mnemonic share — which group/member it is,
/// and the thresholds it needs — without requiring enough shares to
/// recover anything. Useful for labeling a share as it's entered.
#[wasm_bindgen]
pub fn slip39_inspect(mnemonic: &str) -> Result<JsShareInfo, JsValue> {
    let info = pkc_slip39::inspect(mnemonic).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(JsShareInfo {
        group_number: info.group_index as u32 + 1,
        group_count: info.group_count as u32,
        group_threshold: info.group_threshold as u32,
        member_number: info.member_index as u32 + 1,
        member_threshold: info.member_threshold as u32,
    })
}

/// Recombines SLIP-39 mnemonic shares into the original secret's hex, at
/// whatever length it was split with — 32 bytes for a private key, other
/// lengths for e.g. BIP39 entropy from another tool. A wrong `passphrase`
/// is not detected as an error here — SLIP-39 decryption is intentionally
/// unauthenticated, so it silently returns different, wrong bytes instead.
/// Callers should show the derived addresses so the user can visually
/// confirm they recognize them.
#[wasm_bindgen]
pub fn slip39_combine(mnemonics: Vec<String>, passphrase: &str) -> Result<String, JsValue> {
    let refs: Vec<&str> = mnemonics.iter().map(String::as_str).collect();
    let secret = pkc_slip39::combine(&refs, passphrase).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(hex_encode(&secret))
}
