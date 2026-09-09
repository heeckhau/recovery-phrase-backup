# Roadmap

## Motivation

This started as a `bitcoin-ethereum-private-key-converter.html` file (kept in
[`reference/`](reference/)) — a legacy single-file JS tool that converts
between a Bitcoin WIF private key and a raw Ethereum private key, and derives
addresses for both chains. It's being rewritten from scratch as a real,
cloneable Rust project instead of one big file of vendored JavaScript, both
as a Rust/WASM learning project and to add key-backup features the original
never had.

**Guiding principle: auditability first.** This handles real private keys,
so every choice — dependencies, architecture, UI — favors the shortest
possible trust chain over convenience or polish. Prefer minimal, well-known,
audited dependencies; prefer a plain static page over a framework; prefer
code someone can read end to end over code that's merely tested.

## Architecture (applies to every phase)

A Cargo workspace, not a single file:
- **`core`** — pure Rust, no wasm dependency, holds all crypto/encoding logic, fully unit-testable with plain `cargo test`.
- **`cli`** — thin binary wrapping `core`, for offline/air-gapped use and easy manual testing.
- **`wasm`** — thin `wasm-bindgen` layer wrapping `core` for the browser.
- **`web/`** — a hand-written static HTML page with vanilla JS, no frontend framework, loading the `wasm-pack --target web` output.

Each phase below adds a module/crate alongside this structure rather than reworking it.

## Phases

### Phase 1 — ETH-primary private key/address converter
Feature parity with the legacy tool, fixing its known gaps: no WIF checksum
validation, uncompressed-keys-only, no testnet, no EIP-55 checksum casing on
ETH addresses. Full detail (crate layout, dependency choices, API
signatures, test vectors, milestones) is in [`PLAN.md`](PLAN.md). Since
building it, the web UI was restructured Ethereum-first (Ethereum address
up top, Bitcoin's network/compressed options and WIF/address grouped into
their own tab), and plain BIP39 mnemonic support was added as a third input
format alongside hex/WIF — a private key's 32 bytes used directly as BIP39
entropy, which is reversible but is **not** the "mnemonic → PBKDF2 seed →
BIP32 HD wallet" pipeline actual wallets use, so it doesn't import into
MetaMask etc. as a recovery phrase. This is a plain, non-redundant single
phrase — see Phase 2 for redundant, split backup shares.
**Status: in progress.**

### Phase 2 — SLIP-0039 Shamir Secret Sharing
Split a key/seed into checksum-protected mnemonic shares so it can be
recovered from, e.g., "2 of 3 locations, each needing 2 of 3 people
present" — matching the original motivation of not trusting any single
copy of a key. Targeting the **SLIP-0039** standard specifically (spec +
independent implementations, incl. Trezor hardware wallets), not
`iancoleman.io/shamir39` — that tool is a useful UX reference but is a
single-author, single-threshold prototype with no independent
implementations and isn't the standard. SLIP-0039 is pure GF(256) math with
different inputs (entropy/mnemonics, not secp256k1 keys), so it lands as an
independent `slip39` crate alongside `core`, with its own thin `wasm`
bindings and a new section of the web UI — no changes to Phase 1's code.
**Status: not started.**

### Phase 3 — Printable backup cards
A pen-and-paper-friendly way to record and recover shares: a
`@media print` card layout in `web/`, exported via the browser's native
"Print to PDF" — no new dependencies, no secrets leaving the page. A
stretch goal (only after this works) is real programmatic PDF generation
client-side (e.g. `printpdf` compiled to wasm) for tighter layout control
and QR codes.
**Status: not started.**

## Prior art consulted
- [`reference/bitcoin-ethereum-private-key-converter.html`](reference/bitcoin-ethereum-private-key-converter.html) — the legacy tool being replaced (bitaddress.org + coinb.in `coinjs` + js-sha3 code, all BSD/MIT).
- [`reference/bip39-standalone.html`](reference/bip39-standalone.html) — `iancoleman/bip39` (MIT), UX reference for mnemonic/entropy input and multi-format output.
- `iancoleman.io/shamir39` — considered and rejected as the Phase 2 scheme (see above); UX reference only.
