// Drives pkc-slip39 for the cross-tool recovery suite (scripts/interop/).
// Not part of the library: reads one JSON request per line on stdin and
// writes one JSON response per line on stdout.
//
//   {"op":"split","secret_hex":..,"group_threshold":..,"groups":[[t,n],..],
//    "passphrase":..,"iteration_exponent":..}      -> {"mnemonics":[[..],..]}
//   {"op":"combine","mnemonics":[..],"passphrase":..} -> {"secret_hex":..}
//   {"op":"inspect","mnemonic":..}                   -> {"extendable":..,"iteration_exponent":..,..}
//   {"op":"split_phrase","phrase":..,...as split}    -> {"mnemonics":[[..],..]}   (BIP39 phrase in)
//   {"op":"combine_phrase","mnemonics":[..],"passphrase":..} -> {"phrase":..,"fingerprint":..}
//
// Any library error comes back as {"error": "..."}.
use std::io::{BufRead, Write};

use serde_json::{json, Value};

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("odd-length hex".into());
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string())).collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn str_field<'a>(req: &'a Value, key: &str) -> Result<&'a str, String> {
    req[key].as_str().ok_or_else(|| format!("missing string field {key}"))
}

fn u8_field(req: &Value, key: &str) -> Result<u8, String> {
    req[key].as_u64().and_then(|v| u8::try_from(v).ok()).ok_or_else(|| format!("missing u8 field {key}"))
}

fn handle(req: &Value) -> Result<Value, String> {
    match str_field(req, "op")? {
        op @ ("split" | "split_phrase") => {
            let secret = if op == "split_phrase" {
                pkc_core::Phrase::parse(str_field(req, "phrase")?).map_err(|e| e.to_string())?.entropy().to_vec()
            } else {
                hex_decode(str_field(req, "secret_hex")?)?
            };
            let groups: Vec<pkc_slip39::GroupSpec> = req["groups"]
                .as_array()
                .ok_or("missing groups")?
                .iter()
                .map(|g| {
                    let t = g[0].as_u64().ok_or("bad group")? as u8;
                    let n = g[1].as_u64().ok_or("bad group")? as u8;
                    Ok(pkc_slip39::GroupSpec { member_threshold: t, member_count: n })
                })
                .collect::<Result<_, String>>()?;
            let mnemonics = pkc_slip39::split(
                &secret,
                u8_field(req, "group_threshold")?,
                &groups,
                str_field(req, "passphrase")?,
                u8_field(req, "iteration_exponent")?,
            )
            .map_err(|e| e.to_string())?;
            Ok(json!({ "mnemonics": mnemonics }))
        }
        "combine" => {
            let mnemonics: Vec<&str> =
                req["mnemonics"].as_array().ok_or("missing mnemonics")?.iter().filter_map(Value::as_str).collect();
            let secret = pkc_slip39::combine(&mnemonics, str_field(req, "passphrase")?).map_err(|e| e.to_string())?;
            Ok(json!({ "secret_hex": hex_encode(&secret) }))
        }
        "combine_phrase" => {
            let mnemonics: Vec<&str> =
                req["mnemonics"].as_array().ok_or("missing mnemonics")?.iter().filter_map(Value::as_str).collect();
            let secret = pkc_slip39::combine(&mnemonics, str_field(req, "passphrase")?).map_err(|e| e.to_string())?;
            let phrase = pkc_core::Phrase::from_entropy(&secret).map_err(|e| e.to_string())?;
            Ok(json!({ "phrase": phrase.to_words().as_str(), "fingerprint": hex_encode(&phrase.fingerprint()) }))
        }
        "inspect" => {
            let info = pkc_slip39::inspect(str_field(req, "mnemonic")?).map_err(|e| e.to_string())?;
            Ok(json!({
                "id": info.id,
                "extendable": info.extendable,
                "iteration_exponent": info.iteration_exponent,
                "group_threshold": info.group_threshold,
                "group_count": info.group_count,
                "group_index": info.group_index,
                "member_index": info.member_index,
            }))
        }
        other => Err(format!("unknown op {other}")),
    }
}

fn main() {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in std::io::stdin().lock().lines() {
        let line = line.expect("stdin is readable");
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(req) => handle(&req).unwrap_or_else(|e| json!({ "error": e })),
            Err(e) => json!({ "error": format!("bad request: {e}") }),
        };
        writeln!(out, "{response}").expect("stdout is writable");
        out.flush().expect("stdout is writable");
    }
}
