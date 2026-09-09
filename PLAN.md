# Rust/WASM Private Key Converter — Phase 1 Plan

## Context

`bitcoin-ethereum-private-key-converter.html` in this directory is a legacy single-file JS tool: it takes a raw hex private key or a Bitcoin WIF key, normalizes between them, and derives a Bitcoin P2PKH address and an Ethereum address. It works, but as a learning exercise in Rust/WASM and as a base to later add Shamir Secret Sharing (SLIP-0039) for pen-and-paper key backup, we're rewriting it as a real, cloneable, auditable Rust project instead of one big vendored-JS file. Auditability is the top priority throughout — this handles real private keys, so dependencies and architecture are chosen to keep the trust chain short.

This phase covers **only** the key/address converter (feature parity with the legacy tool, plus fixing known gaps: no WIF checksum validation, uncompressed-only, no testnet, no EIP-55 casing on ETH addresses). SLIP-0039 and printable backup cards are deliberately out of scope here, but the structure below is chosen so neither requires reworking this phase's code. (Also considered `iancoleman.io/shamir39` as prior art — it's a single-author, single-threshold, self-described prototype with no independent implementations, not the SLIP-0039 standard, so it's UX reference only, not the scheme to build Phase 2 on.)

## Approach

**Cargo workspace**, three crates + a static site:
```
private-key-convertor/
├── Cargo.toml            # [workspace] members = ["core", "cli", "wasm"]
├── core/                 # pkc-core: pure Rust, no wasm dependency, fully unit-testable
├── cli/                  # pkc-cli: bin crate, core + clap, for offline/air-gapped use
├── wasm/                 # pkc-wasm: cdylib, core + wasm-bindgen, browser glue only
├── web/                  # index.html + app.js (vanilla JS), loads web/pkg/ (wasm-pack output, gitignored)
└── reference/            # existing bitcoin-ethereum-private-key-converter.html + bip39-standalone.html(+.asc), moved here for context, not modified
```
`core` has zero UI/browser concerns; `wasm` and `cli` are both thin adapters over it. A later `slip39` crate can join the workspace as a sibling of `core` without touching this API, since SLIP-0039 is independent GF(256) math with different inputs (entropy/mnemonics, not secp256k1 keys).

**Dependencies** (chosen for wasm-compatibility and audit weight — all pure-Rust, no C toolchain needed for the wasm target):
- `k256` — secp256k1 EC math, pure Rust (RustCrypto). Rejected `secp256k1` (rust-bitcoin's libsecp256k1 wrapper) for this project despite it being the Bitcoin Core gold standard, because it needs a C compiler for the wasm target — extra build fragility and a C audit surface we don't need here.
- `sha3` — `Keccak256` specifically (Ethereum's original, pre-NIST-finalization padding — not `Sha3_256`, which is a different hash).
- `sha2` + `ripemd` — Bitcoin hash160 (SHA-256 then RIPEMD160).
- `bs58` with the `check` feature — Base58Check encode/decode; decoding fails on a bad checksum instead of silently returning garbage, which directly fixes the legacy tool's biggest gap.
- `zeroize` — scrub private key bytes on drop, consistent with the stated security priority even though not explicitly requested.
- `thiserror` — error type in `core`.
- `wasm-bindgen` (pin version to match installed `wasm-pack` 0.15.0 at scaffold time) + `console_error_panic_hook` so unexpected panics show up in the browser console instead of vanishing.
- `clap` (derive) — CLI arg parsing, only in `cli`.

Local toolchain already has everything needed: `rustc`/`cargo` 1.98.0-nightly, `wasm32-unknown-unknown` target installed, `wasm-pack` 0.15.0, `git` 2.54.0. No install steps needed.

**`core` public API** (`core/src/lib.rs`):
```rust
pub enum Network { Mainnet, Testnet }
pub enum KeyFormat { Compressed, Uncompressed }
pub struct PrivateKey([u8; 32]); // Zeroize on drop

pub enum Error { InvalidHexLength, InvalidHexChar, InvalidWifChecksum,
                 InvalidWifLength, UnknownWifVersionByte /* thiserror */ }

pub struct ParsedKey { pub key: PrivateKey, pub network: Option<Network>, pub format: Option<KeyFormat> }
pub fn parse_private_key(input: &str) -> Result<ParsedKey, Error>;

pub fn encode_wif(key: &PrivateKey, network: Network, format: KeyFormat) -> String;
pub fn btc_p2pkh_address(key: &PrivateKey, network: Network, format: KeyFormat) -> Result<String, Error>;
pub fn eth_address(key: &PrivateKey) -> Result<String, Error>; // uncompressed pubkey, EIP-55 cased

pub struct ConversionResult { pub network: Network, pub compressed: bool,
    pub private_key_hex: String, pub wif: String, pub btc_address: String, pub eth_address: String }
pub fn convert(input: &str, network: Network, format: KeyFormat) -> Result<ConversionResult, Error>;
```
`parse_private_key` recovers network/format from a WIF's version byte and trailing `0x01` flag when the input is a WIF; for raw hex input (no such metadata), `convert`'s explicit `Network`/`KeyFormat` arguments decide what to encode.

**`wasm` crate**: one exported function, `convert(input: &str, network: &str, compressed: bool) -> Result<JsConversionResult, JsValue>`, where `JsConversionResult` is a `#[wasm_bindgen]` struct with plain field getters (no `serde-wasm-bindgen` needed for a shape this simple). `core::Error` maps to `JsValue::from_str(&e.to_string())` — never a panic across the boundary. An `init()` export calls `console_error_panic_hook::set_once()`.

**`web/`**: hand-written `index.html` + `app.js` (vanilla JS, `autocomplete="off"` on the key input), loading `web/pkg/pkc_wasm.js` — the output of `wasm-pack build --target web --out-dir web/pkg` (gitignored; built on demand, documented in a README).

**Testing** — entirely in `core`, plain `cargo test`, no browser needed:
- Well-known private-key-`1` vectors: uncompressed WIF `5HpHagT65TZzG1PH3CSu63k8DbpvD8s5ip4nEB3kEsreAnchuDf` → address `1EHNa6Q4Jz2uvNExL497mE43ikXhwF6kZm`; compressed WIF `KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn` → address `1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH`.
- A testnet compressed vector: WIF `cMahea7zqjxrtgAbB7LSGbcQUr1uX1ojuat9jZodMN87JcbXMTcA` → address `mrCDrCybB6J1vRfbwM5hemdJz73FwDBC8r`.
- EIP-55 casing vector: `0x7e5f4552091a69125d5dfcb7b8c2659029395bdf` → `0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf`.
- Negative tests: a flipped WIF character → `Err(InvalidWifChecksum)`; malformed hex length/characters → `Err(...)`.
- Treat these as starting vectors, not gospel — re-derive/cross-check anything surprising against a second independent source while implementing, standard practice for crypto test vectors.

## Milestones
1. Scaffold the workspace (three empty crates, `.gitignore` for `target/` and `web/pkg/`, `git init`, move the two existing HTML files into `reference/`, copy this plan into the repo as `PLAN.md` so it's tracked alongside the code).
2. `core`: input parsing + WIF encode/decode + checksum validation + tests.
3. `core`: BTC P2PKH address derivation (mainnet+testnet, compressed+uncompressed) + tests.
4. `core`: ETH address derivation + EIP-55 casing + tests.
5. `wasm` crate bindings + panic hook.
6. `web/` HTML/JS UI wired to the wasm build.
7. `cli` crate (cheap, last, reuses a finished `core`).

## Verification
- `cargo test --workspace` — all `core` unit tests above pass without touching wasm or a browser.
- `cd wasm && wasm-pack build --target web --out-dir ../web/pkg` builds cleanly.
- Serve `web/` over HTTP (e.g. `python3 -m http.server` from that directory — opening `index.html` directly via `file://` will not work, browsers block wasm fetches over that protocol) and manually run each test vector above through the page, confirming the displayed WIF/addresses match.
- `cargo run -p pkc-cli -- <same inputs>` matches the browser output.
