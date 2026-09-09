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
