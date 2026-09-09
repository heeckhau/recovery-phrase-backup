// Data-driven test against the official SLIP-39 test vectors. Source:
// https://github.com/trezor/python-shamir-mnemonic/blob/master/vectors.json
// (MIT license). This is the primary correctness net for this crate,
// since — unlike BIP39 — there is no audited Rust SLIP-39 crate to lean
// on for confidence (see PLAN.md).
//
// Format: [description, mnemonics, expected_secret_hex, expected_bip32_xprv].
// An empty expected_secret_hex means combining the mnemonics MUST fail.
// The passphrase "TREZOR" is used for all valid vector sets, per the
// spec's own "Test vectors" section. expected_bip32_xprv is unused here —
// BIP32 derivation is out of scope for this crate.
//
// The suite includes both 128-bit and 256-bit vectors; this crate's public
// API only supports 256-bit secrets (see PLAN.md — matches this tool's
// secp256k1 keys). For the 128-bit *valid* vectors, the correct, intended
// behavior is a clean `Error::UnsupportedSecretLength` rejection rather
// than a secret match — checked explicitly below, not skipped.

use serde::Deserialize;

#[derive(Deserialize)]
struct Vector(String, Vec<String>, String, String);

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

#[test]
fn official_test_vectors() {
    let raw = include_str!("vectors.json");
    let vectors: Vec<Vector> = serde_json::from_str(raw).expect("vectors.json must parse");
    assert_eq!(vectors.len(), 45, "expected all 45 official vectors to be present");

    let mut failures = Vec::new();
    for Vector(description, mnemonics, expected_secret_hex, _expected_xprv) in vectors {
        let refs: Vec<&str> = mnemonics.iter().map(String::as_str).collect();
        let result = pkc_slip39::combine(&refs, "TREZOR");

        if expected_secret_hex.is_empty() {
            if let Ok(actual) = result {
                failures.push(format!("{description}: expected an error, got Ok({actual:02x?})"));
            }
            continue;
        }

        let expected = hex_decode(&expected_secret_hex);
        if expected.len() != 32 {
            match result {
                Err(pkc_slip39::Error::UnsupportedSecretLength(bits)) if bits == expected.len() * 8 => {}
                other => failures.push(format!(
                    "{description}: expected Err(UnsupportedSecretLength({} bits)) since this is a {}-bit vector, got {other:?}",
                    expected.len() * 8,
                    expected.len() * 8
                )),
            }
            continue;
        }

        match result {
            Ok(actual) if actual.to_vec() == expected => {}
            Ok(actual) => failures.push(format!("{description}: got {actual:02x?}, expected {expected:02x?}")),
            Err(e) => failures.push(format!("{description}: expected Ok({expected_secret_hex}), got Err({e})")),
        }
    }

    assert!(failures.is_empty(), "{} of 45 official vector(s) failed:\n{}", failures.len(), failures.join("\n"));
}
