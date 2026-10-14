# Rust/WASM Private Key Converter: Phase 1 Plan

## Context

`bitcoin-ethereum-private-key-converter.html` in this directory is a legacy single-file JS tool: it takes a raw hex private key or a Bitcoin WIF key, normalizes between them, and derives a Bitcoin P2PKH address and an Ethereum address. It works, but as a learning exercise in Rust/WASM and as a base to later add Shamir Secret Sharing (SLIP-0039) for pen-and-paper key backup, we're rewriting it as a real, cloneable, auditable Rust project instead of one big vendored-JS file. Auditability is the top priority throughout: this handles real private keys, so dependencies and architecture are chosen to keep the trust chain short.

This phase covers **only** the key/address converter (feature parity with the legacy tool, plus fixing known gaps: no WIF checksum validation, uncompressed-only, no testnet, no EIP-55 casing on ETH addresses). SLIP-0039 and printable backup cards are deliberately out of scope here, but the structure below is chosen so neither requires reworking this phase's code. (Also considered `iancoleman.io/shamir39` as prior art: it's a single-author, single-threshold, self-described prototype with no independent implementations, not the SLIP-0039 standard, so it's UX reference only, not the scheme to build Phase 2 on.)

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

**Dependencies** (chosen for wasm-compatibility and audit weight: all pure-Rust, no C toolchain needed for the wasm target):
- `k256`: secp256k1 EC math, pure Rust (RustCrypto). Rejected `secp256k1` (rust-bitcoin's libsecp256k1 wrapper) for this project despite it being the Bitcoin Core gold standard, because it needs a C compiler for the wasm target: extra build fragility and a C audit surface we don't need here.
- `sha3`: `Keccak256` specifically (Ethereum's original, pre-NIST-finalization padding; not `Sha3_256`, which is a different hash).
- `sha2` + `ripemd`: Bitcoin hash160 (SHA-256 then RIPEMD160).
- `bs58` with the `check` feature: Base58Check encode/decode; decoding fails on a bad checksum instead of silently returning garbage, which directly fixes the legacy tool's biggest gap.
- `zeroize`: scrub private key bytes on drop, consistent with the stated security priority even though not explicitly requested.
- `thiserror`: error type in `core`.
- `wasm-bindgen` (pin version to match installed `wasm-pack` 0.15.0 at scaffold time) + `console_error_panic_hook` so unexpected panics show up in the browser console instead of vanishing.
- `clap` (derive): CLI arg parsing, only in `cli`.

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

**`wasm` crate**: one exported function, `convert(input: &str, network: &str, compressed: bool) -> Result<JsConversionResult, JsValue>`, where `JsConversionResult` is a `#[wasm_bindgen]` struct with plain field getters (no `serde-wasm-bindgen` needed for a shape this simple). `core::Error` maps to `JsValue::from_str(&e.to_string())`: never a panic across the boundary. An `init()` export calls `console_error_panic_hook::set_once()`.

**`web/`**: hand-written `index.html` + `app.js` (vanilla JS, `autocomplete="off"` on the key input), loading `web/pkg/pkc_wasm.js`, the output of `wasm-pack build --target web --out-dir web/pkg` (gitignored; built on demand, documented in a README).

**Testing**: entirely in `core`, plain `cargo test`, no browser needed:
- Well-known private-key-`1` vectors: uncompressed WIF `5HpHagT65TZzG1PH3CSu63k8DbpvD8s5ip4nEB3kEsreAnchuDf` → address `1EHNa6Q4Jz2uvNExL497mE43ikXhwF6kZm`; compressed WIF `KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn` → address `1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH`.
- A testnet compressed vector: WIF `cMahea7zqjxrtgAbB7LSGbcQUr1uX1ojuat9jZodMN87JcbXMTcA` → address `mrCDrCybB6J1vRfbwM5hemdJz73FwDBC8r`.
- EIP-55 casing vector: `0x7e5f4552091a69125d5dfcb7b8c2659029395bdf` → `0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf`.
- Negative tests: a flipped WIF character → `Err(InvalidWifChecksum)`; malformed hex length/characters → `Err(...)`.
- Treat these as starting vectors, not gospel: re-derive/cross-check anything surprising against a second independent source while implementing, standard practice for crypto test vectors.

## Milestones
1. Scaffold the workspace (three empty crates, `.gitignore` for `target/` and `web/pkg/`, `git init`, move the two existing HTML files into `reference/`, copy this plan into the repo as `PLAN.md` so it's tracked alongside the code).
2. `core`: input parsing + WIF encode/decode + checksum validation + tests.
3. `core`: BTC P2PKH address derivation (mainnet+testnet, compressed+uncompressed) + tests.
4. `core`: ETH address derivation + EIP-55 casing + tests.
5. `wasm` crate bindings + panic hook.
6. `web/` HTML/JS UI wired to the wasm build.
7. `cli` crate (cheap, last, reuses a finished `core`).

## Verification
- `cargo test --workspace`: all `core` unit tests above pass without touching wasm or a browser.
- `cd wasm && wasm-pack build --target web --out-dir ../web/pkg` builds cleanly.
- Serve `web/` over HTTP (e.g. `python3 -m http.server` from that directory: opening `index.html` directly via `file://` will not work, browsers block wasm fetches over that protocol) and manually run each test vector above through the page, confirming the displayed WIF/addresses match.
- `cargo run -p pkc-cli -- <same inputs>` matches the browser output.

## Review: 2026-09-14

State of the code as reviewed (every crate, the web page, the CLI), after Phase 1 and the Phase 2
crate/wasm/cli/web work. `cargo test --workspace` passes (25 `core`, 52 `slip39` unit, 1 official
vectors), clippy is clean, `wasm-pack build --target web` builds (383 KB `.wasm`).

### Holds up

- **Architecture.** `core` has no UI dependency; `cli` and `wasm` are thin adapters; the page is
  vanilla JS. This is what lets the page, the build and the distribution change shape (see
  ROADMAP) without touching the code that matters.
- **Secret handling.** `PrivateKey` is zeroized with a redacting `Debug`; the scalar range is
  validated through `k256`; WIF and mnemonic checksums are enforced; SLIP-39 keeps share values,
  group secrets, the EMS and the Feistel state in `Zeroizing`, down to the `bitvec` buffer
  (`SecretBits`); the share digest is compared in constant time; the CLI warns on argv secrets and
  prompts with hidden input; the page carries a CSP and never sets HTML from a string.
- **`pkc-slip39`.** All 45 official Trezor vectors pass; the GF(256) tables are checked
  exhaustively against the direct multiplication; `ext=0/1` handling (empty salt vs
  `"shamir" || id`; `shamir_extendable` vs `shamir` in RS1024) matches the reference
  implementation. There is still no audited, permissively licensed Rust SLIP-39 crate, so this one
  has value on its own, independent of the rest of the tool.

### Findings

1. **The raw-entropy "mnemonic" collides with seed phrases.** `to_mnemonic` encodes the 32 key
   bytes as 24 BIP39 words: reversible, but indistinguishable from a wallet seed phrase, and the
   page's warning is the only thing separating the two. Once real seed-phrase inputs exist (BIP44
   addresses, where the words *are* the seed) one input box carrying both meanings is a
   loss-of-funds UX. Needs a semantic decision before any seed-phrase feature lands (ROADMAP,
   Decision A, which also lays out the replacements: hex/WIF stay the interchange formats for a
   raw key, and a 1-of-1 SLIP-39 mnemonic (different wordlist, stronger checksum, a standard)
   takes over the "key as checksummed words on paper" job).
2. **SLIP-39 splits a secp256k1 key, not a seed.** Splitting a seed phrase's entropy makes the
   shares recover the 24 words and therefore every key derived from them; splitting one raw key
   covers only that key. Same decision as (1).
3. **No BIP32/BIP44 derivation.** Seed phrase → `m/44'/60'/0'/0/i` Ethereum addresses is the most
   common thing people open iancoleman's tool for, and is absent. Bitcoin is legacy P2PKH only
   (no P2WPKH / Taproot).
4. **The trust story is not built, and for this tool it is the product.** No README, no LICENSE
   file (declared `MIT OR Apache-2.0`; `wasm-pack` warns), no CI, no reproducible single-file
   build, no signed release. `web/` needs a local HTTP server (module script + wasm fetch fail on
   `file://`): a step back from the one-file-opens-from-disk model of
   `reference/bip39-standalone.html` + `.asc`, which is the bar to meet.
5. **No tests on `wasm`, `web/app.js` or `cli`.** Acceptable for now: the risk sits in `core` and
   `slip39`, which are the tested parts.

## Cross-tool recovery test suite (built 2026-10-12)

The official vectors prove `combine` decodes what the reference implementation encoded. They say
nothing about the other direction (whether *our* shares recover elsewhere) and nothing at the
BIP39-phrase level, which is where the product lives. The suite closes both gaps, and is the
concrete form of the "no lock-in" promise (ROADMAP, "Distribution"): a backup made here must be
recoverable with tools that have nothing to do with this project, and vice versa.

**Run it**: `scripts/interop/run.sh` (regenerates the fixtures) or `run.sh --check-only`. It
enters a nix shell pinned to one nixpkgs revision for Python and Go; needs nix and network on
first run. Current result: 266/266 checks across 19 cases.

**Oracles** (pinned; the version is recorded in each fixture):
- `trezor/python-shamir-mnemonic` 0.3.0: the reference implementation (MIT), through its Python
  API. Byte level: every length 128 to 512 bits, both extendable values, iteration exponents 0 to 2,
  with and without a passphrase.
- `pjkundert/python-slip39` 14.1.0: its BIP39 mode (12- and 24-word phrases) and its raw mode (a
  512-bit seed). GPL-3.0: run only as an external program from the harness, in its own venv; never
  linked, vendored or shipped. The fixtures are its output data.
- `gavincarr/seedkit` v0.3.3: Go CLI, BIP39 phrase → shares → phrase, at all five phrase
  lengths, with and without a SLIP-39 passphrase. Its GitHub repository is no longer reachable;
  the Go module proxy still serves v0.3.3, verified against Go's checksum database on install.
- `iancoleman/slip39`: not included, since it is browser-only and adds no parameter the three above
  don't cover.

**Directions.** Theirs → ours: every oracle's shares recover in `pkc-slip39` to the same bytes,
both live in the harness and from the committed fixtures in a plain `cargo test`
(`slip39/tests/interop.rs`). Ours → theirs: shares from `pkc-slip39` recover in every oracle:
live only, since it needs the oracles. For the phrase-level oracles the recovered BIP39 phrase is
checked word for word, through each tool's own conversion.

**Checks** per case, in both directions where they apply: two different minimal recovery sets;
a set with surplus groups and members; a wrong passphrase gives the *same* wrong bytes in both
tools (pins our Feistel/PBKDF2 layer against independent implementations); a tampered share (two
adjacent words swapped) is rejected by both; `inspect` reads back the tool's extendable flag and
iteration exponent. Planting a bug in our salt derivation made 16 checks fail in each half.

**Findings** (recorded in the fixtures):
- *Defaults*: all three tools default to `extendable = true` and **iteration exponent 1**; ours
  (CLI and page) defaults to 0. Input for Decision D's defaults.
- *Passphrase*: python-slip39's BIP39 mode always uses an empty SLIP-39 passphrase (a passphrase
  there is the BIP39 one, for addresses only); seedkit's `-p` is the SLIP-39 passphrase.
- *Lengths*: python-slip39 refuses to create shares for 160-, 192- and 224-bit secrets (15-, 18-
  and 21-word phrases), but recovers ours at those lengths.
- *Surplus shares*: the reference implementation and seedkit reject more groups or members than
  the thresholds need, as the spec says ("GM MUST be equal to GT"); python-slip39 and
  `pkc-slip39` ignore the surplus. Ours is deliberate (commit `228e746`): the surplus is checked
  to belong to the same backup first. Each fixture records its tool's behaviour, and the harness
  checks it, so a change on either side shows up.

**Mechanics**:
- `scripts/interop/run.py` drives everything: `oracle_shamir.py` and `oracle_slip39.py` (one
  venv each: python-slip39 ships a fork under the reference's module name), the seedkit binary,
  and `slip39/examples/interop.rs`, a small example binary exposing `split`/`combine`/`inspect`
  over JSON lines. Examples are not part of the library.
- Fixtures: `slip39/tests/interop/<tool>.json`. Secrets are derived from the case names and stay
  fixed; shares are random, so regenerating rewrites them. Regenerating is a reviewed change,
  never automatic.
- Python dependencies are pinned as full transitive freezes (`requirements-*.txt`, installed with
  `--no-deps`); versions, not hashes, since wheel hashes differ per platform.

**Phrase level** (added 2026-10-14, with ROADMAP step 4): for python-slip39's BIP39 mode and
seedkit, our side now works from the words too: their shares recover to the original phrase
through `pkc_core::Phrase`, and ours are split from the phrase itself, both live and in
`cargo test`. The live half runs in CI (`.github/workflows/ci.yml`).
