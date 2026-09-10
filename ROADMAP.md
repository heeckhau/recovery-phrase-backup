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
**Status: done.**

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

Includes, as a companion feature once splitting works: a pen-and-paper-friendly
way to record and recover shares — a `@media print` card layout in `web/`,
exported via the browser's native "Print to PDF" (no new dependencies, no
secrets leaving the page). Real programmatic PDF generation (e.g. `printpdf`
compiled to wasm, for QR codes/tighter layout) is a stretch goal after that.

Chosen as the next phase (over jumping straight to Phase 3) because it's a
bounded extension of the existing architecture — no new curve, no new
external ecosystem to learn — that directly serves this tool's existing
users before taking on something much bigger.

Turned out there's no audited Rust SLIP-39 crate to depend on at all
(unlike `k256`/`bip39`): the one candidate with the right license is
self-described by its own author as "untested work-in-progress," and the
only actively-maintained option is GPL-3.0 (license-incompatible). Hand-
implemented directly from the spec instead, verified against the official
45-vector reference test suite (trezor/python-shamir-mnemonic) — all 45
pass. Full detail in the plan history (`/plan` at the time; see commit
`484df7a` onward for the implementation).

The `pkc-slip39` crate, its `wasm` bindings, `cli slip39 split`/
`slip39 combine` subcommands, and the web UI (a "SLIP-39 Backup" page tab
with Split/Combine sub-tabs: dynamic group rows, a group-threshold picker
that only appears once there's more than one group, and a Combine view
that shows the recovered key's derived addresses so users can visually
catch a wrong passphrase) are all done, verified end-to-end in a real
browser.

Still open: the pen-and-paper print-card layout mentioned above — the
Split/Combine *forms* are built, but not yet a `@media print` card view
for writing down/recovering shares on paper. Small, self-contained,
doesn't block anything else.
**Status: crate/wasm/cli/web UI done; print cards not started.**

### Phase 3 — Validator key generation (wagyu-key-gen equivalent)
Add the functionality of [stake-house/wagyu-key-gen](https://github.com/stake-house/wagyu-key-gen)
(a React/Electron GUI wrapping the Python `ethstaker-deposit-cli`): generate
BLS12-381 validator keys via EIP-2333/2334 HD derivation from a mnemonic,
EIP-2335 encrypted keystores, and SSZ-encoded `deposit_data.json` for
Ethereum's 32 ETH validator deposit contract.

This is a different, higher-stakes domain from Phase 1/2 — a different
curve (BLS12-381, not secp256k1), a different derivation scheme, and real
irreversible-deposit/slashing consequences if something is subtly wrong
(wagyu-key-gen itself was paid-audited by HashCloak for exactly this
reason). The plan is to **reuse audited crates from Ethereum's Lighthouse
consensus client** (`sigp/lighthouse`) rather than hand-roll BLS/SSZ/keystore
logic — `blst`-based BLS12-381, `eth2_keystore`/`eth2_wallet` for
EIP-2333/2335, `ethereum_ssz` for deposit data — consistent with how this
project has always preferred audited dependencies (`k256`, `bip39`) over
reimplementing cryptography. Scope down to mnemonic → BLS keys → keystore
first; deposit-data/SSZ generation can follow once that's solid, with much
more test-vector rigor than Phase 1/2 needed given the stakes.
**Status: not started.**

### Phase 4 — Universal key-gen tool
Combine Phase 2 and Phase 3: one tool where SLIP-39 split/recover works for
*both* secp256k1 keys (BTC/ETH) and BLS12-381 validator keys, not just the
former — genuinely useful functionality wagyu-key-gen doesn't have on its
own, not merely a merge for its own sake. Likely also where packaging as a
proper cross-platform desktop app happens (Windows/macOS/Linux), reusing the
same `web/` HTML/JS/wasm frontend rather than rewriting it — **Tauri** is
the leading candidate (Rust backend, OS-native webview instead of bundled
Chromium, `tauri-action`/`cargo-dist` for cross-platform CI builds), over a
pure-native GUI toolkit (`egui`/`iced`) which would mean rebuilding the UI
instead of reusing it.
**Status: not started.**

## Prior art consulted
- [`reference/bitcoin-ethereum-private-key-converter.html`](reference/bitcoin-ethereum-private-key-converter.html) — the legacy tool being replaced (bitaddress.org + coinb.in `coinjs` + js-sha3 code, all BSD/MIT).
- [`reference/bip39-standalone.html`](reference/bip39-standalone.html) — `iancoleman/bip39` (MIT), UX reference for mnemonic/entropy input and multi-format output.
- `iancoleman.io/shamir39` — considered and rejected as the Phase 2 scheme (see above); UX reference only.
- [`stake-house/wagyu-key-gen`](https://github.com/stake-house/wagyu-key-gen) — target functionality for Phase 3 (see above); audited by HashCloak, a reminder of the correctness bar that phase needs.
