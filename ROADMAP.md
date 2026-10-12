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

Candidate for that stretch goal: [fullbleed](https://docs.fullbleed.dev/getting-started/rust/)
(HTML/CSS → PDF, MIT, 3 deps, no browser). Evaluated 2026-10-12 at
v2.5.22: native renders a share card fine (~12 ms), so a CLI
`slip39 split --pdf` is feasible today. Not usable in `web/` as shipped:
it builds for `wasm32-unknown-unknown` but panics at render time
(`std::time::Instant::now()`, ~112 call sites; would need e.g.
`web-time` upstream), and yields a ~6 MB wasm. Before adopting, weigh
audit cost (~231k lines in one young crate, first release 2026-02,
rapid release cadence) against the auditability priority, and remember
any PDF is the unencrypted-file problem the print hygiene section warns
about.

Chosen as the next phase because it's a bounded extension of the existing
architecture — no new curve, no new external ecosystem to learn — that
directly serves this tool's existing users.

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

### Out of scope — validator key generation
Earlier drafts planned a Phase 3 (wagyu-key-gen's functionality: BLS12-381
validator keys via EIP-2333/2334, EIP-2335 keystores, deposit data) and a
Phase 4 (one tool covering both, packaged as a Tauri desktop app). Dropped
2026-10-12. Under the product framing below, stakers are already covered —
a validator mnemonic is a BIP39 phrase, so backing it up is the same
feature — while generating keystores and deposit files is a different
product with slashing-level stakes. That work stays an experiment in
`heeckhau/wagyu-key-gen`; nothing here depends on it.

## Product framing (proposed, 2026-09-15 — not decided)

**Target audience: self-custody holders** — anyone who holds a BIP39
recovery phrase, whatever wallet or chain. Stakers are part of this group
and are not treated differently: their validator mnemonic is a BIP39
phrase like any other. Targeting "stakers first" was considered and
dropped: it narrows the audience for no gain on the core problem, and
pulls the tool toward chain-specific features.

**The problem.** A recovery phrase on paper or metal is one copy, and one
copy fails in both directions: burn it and it is gone, find it and it is
stolen. Every self-custody holder lives with that trade-off, and the usual
mitigations each move the risk rather than remove it — a BIP39 passphrase
adds a second secret that itself needs a backup and can be brute-forced if
weak; multisig spreads *signing*, not the backup, and is complex and
chain-specific; hardware-wallet Shamir (Trezor) requires that hardware and
replaces the phrase rather than backing it up; custodial recovery (Ledger
Recover) hands the secret to third parties.

**The message.**

> Back up your recovery phrase so that no single place can lose it — or
> reveal it.

**The story.** The tool splits an existing phrase into shares using
SLIP-39 — the open standard hardware wallets use — so that any two of
three (or whatever threshold the user chooses) recover it, and any one
share alone reveals nothing. Recovery gives back the original phrase, so
it works with every wallet: no wallet needs to support SLIP-39. It runs
offline, in a single file that can be inspected and rebuilt, and is
checked against the standard's official test vectors. Stated as plainly as
what it does: it is not a wallet, never signs anything, never touches a
network.

**What the product has to get right** (each is a decision below or a step
in the plan):
- Say precisely that this is a *backup of an existing BIP39 phrase*, not
  Trezor's SLIP-39-as-seed scheme: a Trezor restoring these shares
  directly would open a different wallet. The difference is the
  wallet-agnostic feature, but only if it is said out loud (Decision A).
- Support every BIP39 length, above all 12 words — MetaMask's default
  (Decision C).
- Take a stance on SLIP-39's own passphrase, which is a trap for a backup
  meant to be recovered by an heir years later (Decision D).
- Treat the process as the product. Shamir backups fail operationally —
  shares stored together, heirs who do not know shares exist, a 3-of-3
  threshold that is worse than a single copy, shares nobody can identify.
  Printed cards with plain-language recovery instructions, threshold
  guidance and share labelling are where "safer than paper" is actually
  earned: the shares are still paper; the safety is in the distribution.
- Answer "just buy a Trezor" up front: no hardware purchase, works for the
  phrase you already have, open and verifiable, and Ledger/MetaMask users
  have no Shamir option at all.

**Is there enough value?** (To revisit.) As a public good,
yes; as a business, no. The gap is narrower than "no tool exists". Tools
that back up an existing BIP39 phrase with SLIP-39 already exist:
`python-slip39` (Python, GPL-3.0, a desktop GUI and PDF cards), `seedkit`
(a Go CLI whose README says it is unaudited) and `iancoleman/slip39` (a web
tool). As of 2026-10-12, none of them advertises an audit or ships as a
reproducible, signed single file that opens offline, and none is designed
around a recovery an heir can carry out. That is the gap: real, but it has
to be argued rather than assumed, and the pitch has to say why this over
those. Beyond that, the code that matters already exists and is verified,
and the scope is small and chain-agnostic. Nobody pays for backup tools, but this
is what Ethereum public-goods funding is for (EF ESP, EthStaker,
RetroPGF-style rounds), and an audit is the one thing that funding would
need to cover. The real cost is trust: reproducible builds, signatures,
official vectors and an audit are launch requirements, not polish, and a
backup bug surfaces years later at the worst moment — so this comes with
an open-ended maintenance commitment. Low-regret test: ship a v1 that does
only the backup loop as a single signed file, put it in front of
self-custody communities (EthStaker first, because it is reachable), and
apply for audit funding; if after 3–6 months there is no endorsement and
no funding interest, stop — something useful will have been built anyway.

## Considerations — UI and distribution under the product framing (2026-09-16, thinking in progress)

What the framing above implies for the page and for how it ships, written
down while the approach is still open. Nothing here is decided; it is the
consequence list to weigh the framing against. Offline use is not a
recommendation in this design, it is the premise.

### UI

- **The front door flips.** Today the page is "Private Key Converter"
  with a hex input, SLIP-39 on a second tab. Under the framing the two
  primary actions are **Back up a phrase** and **Recover a phrase**;
  hex/WIF conversion leaves the page and stays in the CLI (decided
  2026-10-12) — a raw-key input next to a phrase input is confusion and
  attack surface the backup flow doesn't need.
- **Input is a phrase, entered word by word.** 12–24 BIP39 words with
  per-word autocomplete (BIP39 words are unique in their first four
  letters), checksum validation, and the phrase's first ETH/BTC address
  (once BIP32/44 exists) shown on screen *before* splitting so the user
  confirms they typed the right phrase — on screen only, never on a card
  (Decision D). Paste stays possible but is not the primary path —
  clipboards leak.
- **Wizard, not a form**, because the process is the product: choose a
  scheme from plain-language presets ("2 of 3: any two shares recover; one
  alone reveals nothing", "3 of 5"), groups and passphrase behind
  Advanced → review → shares shown **one at a time** with a "write it
  down, then re-enter it" verification per share (what Trezor does; it
  catches the transcription error that would otherwise surface in ten
  years) → a "test your recovery now" step. Recovery: shares one at a
  time with the existing inspect badge, then the phrase and its
  fingerprint.
- **Print cards**: one share per page, labelled ("Share 2 of 3 — any 2
  recover"), numbered words, the fingerprint, and recovery instructions an
  heir can follow; plus a secret-free "how to recover" sheet. Offer blank
  handwriting templates as well as filled cards.
- **Hygiene**: a "clear everything" button, clear on tab hide, no storage
  of any kind, no copy-to-clipboard for shares, a `navigator.onLine`
  warning banner. The `?test` prefill goes away in release builds.
- **Out of scope for now**: non-English BIP39 wordlists (MetaMask and
  Trezor are English-only in practice), mobile.
- **Testing**: the wizard's verification steps are the correctness-
  critical UI; they warrant browser-level tests (Playwright against the
  built single file), and the print layout a visual check.

### Distribution

- **The artifact is one HTML file.** Wasm base64-inlined, opens from
  `file://` (an inline module script works there; only cross-file imports
  break — wasm-bindgen's `initSync` takes the bytes directly), CSP
  tightened to `default-src 'none'` plus a build-time script hash and
  `wasm-unsafe-eval`. Roughly 600 KB. Reproducible: pin the toolchain,
  `wasm-pack` and `wasm-opt` (or drop `wasm-opt`, whose output varies by
  version), publish on GitHub Releases with `SHA256SUMS` and a signature.
  The user flow is iancoleman's: download once, verify, disconnect, open —
  and keep the file.
- **Don't host the real thing casually.** A hosted copy undermines the
  story and invites online use; a demo page, if any, should accept only
  test vectors or nag hard. The recommended environment is a live USB
  (Tails) or a fresh browser profile with **no extensions** — extensions
  are the browser's real attack surface for a page like this, and they are
  the honest price of skipping a native app.
- **No desktop app.** A backup tool writes nothing to disk — shares are
  printed or handwritten — so it needs no installers, no code signing, no
  per-OS builds. With validator keys out of scope nothing else would need
  one either; the only native path is the CLI.
- **Printing is the one output, and where the hygiene lives.**
  Print-to-PDF leaves an unencrypted file, often in a cloud-synced folder;
  network printers spool jobs. Guidance inside the flow: print to a
  directly connected printer or handwrite; never "save as PDF".
  Handwriting-first templates make the safest path the default.
- **No lock-in, and say so.** SLIP-39 is a standard:
  `python-shamir-mnemonic` recovers the secret without this tool, and
  entropy → BIP39 words is one library call. Document that procedure so
  nobody's backup depends on this project still existing — part of the
  trust story, not a footnote.
- **The CLI stays** as the power-user / Raspberry-Pi path, secondary.

Net effect: v1 gets *simpler* to ship (one file, no signing infrastructure
for binaries) and *harder* to get right in the two places that matter —
the wizard's verification steps and the reproducible-build/signature
pipeline.

## Proposed next steps (review of 2026-09-14, reordered 2026-10-12)

The findings behind these are in [`PLAN.md`](PLAN.md), "Review". Four
decisions come first because they drive everything after them: A and B
from the code review, C and D from the product framing above. B is
settled (2026-10-12); A, C and D are still proposals.

**Decision A — what 24 words mean.** Proposal: 24 words always means a
BIP39 seed phrase (entropy → PBKDF2 seed → BIP32/44 for ETH/BTC). Drop
Phase 1's raw-entropy encoding.
SLIP-39 then backs up the phrase's entropy, so shares recover the words
and every key derived from them. This is a *backup of a BIP39 phrase*, not
Trezor's SLIP-39-as-BIP32-seed scheme — a Trezor would not recover the
same wallet from these shares — and the UI should say so.

*What replaces the raw-entropy encoding.* It was doing two jobs, and each
has a better standard answer:

- As an **interchange format for a raw key** (input/output): hex (`0x` +
  64 chars, what MetaMask/geth export) for Ethereum and WIF for Bitcoin —
  both already supported. Hex has no checksum; the ecosystem's convention
  compensates by always showing the derived address next to it, so a
  mistyped key reveals itself as a wrong address. Decimal or other numeric
  encodings buy nothing over hex and no wallet imports them. Inventing a
  checksummed text format for Ethereum keys (a Base58Check "WIF for ETH",
  a bech32 key) would be worse than the problem: one more non-standard
  thing to explain.
- As a **paper backup** — words with a checksum, easier to write and read
  than hex: this is what the BIP39 trick was really for, and SLIP-39 is the
  right replacement. A 1-of-1 SLIP-39 mnemonic (33 words for a 256-bit
  secret) *is* "the secret as checksummed words, no sharing" — the spec's
  own first test vector is exactly that. It is strictly better here: a
  different wordlist from BIP39, so it cannot be mistaken for a seed phrase
  (a wallet rejects it outright instead of silently deriving the wrong
  account); a far stronger checksum (RS1024, three words, detects any three
  errors, versus BIP39's 8 bits); a standard with independent
  implementations; and it scales from "just words" to real m-of-n sharing
  in the same format.

The rule that falls out: **BIP39 wordlist → a seed phrase, always. SLIP-39
wordlist → a backup of *some* secret (16–32 bytes). Hex/WIF → a raw key.**
Nothing depended on the raw-entropy encoding — the legacy HTML tool never
had it; it was added in Phase 1 — so it can simply go.

*Residual ambiguity to design for:* SLIP-39 does not tag what the bytes
are — a raw secp256k1 key or a seed phrase's entropy. With the converter
out of the page (Decision B) the page only ever splits phrases, so the
ambiguity is confined to the CLI, which can still back up a raw key. Two
cheap answers: the CLI names what it is backing up ("a seed phrase" or "a
raw key"), and Combine — in both — shows a 32-byte result both ways, "as a
raw key → address X" and "as a seed phrase → these 24 words → address Y",
so the user recognises the one they backed up. (Shorter secrets can only be
phrases.) That is the existing "show the derived addresses so a wrong
passphrase is visible" pattern, extended by one row.

**Decision B — distribution shape.** Settled 2026-10-12: one single-file,
signed HTML (wasm inlined as base64, opens from disk, reproducible build,
sha256 + signature — the `reference/bip39-standalone.html.asc` model) for
backup and recovery, plus the CLI. No desktop app: with validator keys out
of scope nothing writes keystores or deposit files. The hex/WIF converter
stays in the CLI and is left out of the HTML.

**Decision C — every BIP39 length, not just 24 words.** Proposal: accept
12/15/18/21/24-word phrases (128/160/192/224/256-bit entropy). MetaMask's
default is 12 words, so a 24-word-only tool excludes most of the target
audience. SLIP-39 supports any secret that is a multiple of 16 bits and at
least 128 bits, and `pkc-slip39`'s internal `split_secret` already does;
only the public `split`/`combine` API is pinned to 32 bytes (and the
official 128-bit vectors are currently asserted to be *rejected* — those
assertions flip to positive checks). Recovery shows the phrase at its
original length.

**Decision D — SLIP-39's own passphrase.** SLIP-39 encrypts the secret
under a passphrase that is separate from BIP39's "25th word", and a wrong
one silently yields a different secret with no error — for a backup meant
to be recovered by an heir in ten years, a trap. Proposal: default to no
passphrase (the threshold is the security), keep the field behind
"Advanced" with the trap spelled out, and print a verification fingerprint
on every share card. (Deniability, the property the unauthenticated design
buys, is not something this product promises.)

*The fingerprint* (settled 2026-10-12): the phrase's BIP32 master-key
fingerprint — 4 bytes of hash160 of the master public key, with an empty
BIP39 passphrase — printed as 8 hex characters. **Not** an address: an
address printed on every card would let anyone who finds a single share
look up the balance and learn what the other shares unlock. The
fingerprint does two jobs:
- *Shares visibly belong together.* Every share of a set carries the same
  value, so whoever gathers them can tell before recovering that they come
  from one backup — clearer than SLIP-39's own common identifier, which
  only shows up as matching leading words.
- *Recovery is visibly right.* The recovered phrase must reproduce it; a
  wrong passphrase or a mistyped share produces a different value.

It reveals nothing on chain. Bitcoin wallets (Sparrow, Coldcard) display
the same value and put it in PSBTs and descriptors, so for those users it
links a card to their wallet setup, not to their funds. At 32 bits it gives
no practical help to anyone guessing the phrase. The screen may still show
the first address *before* splitting, so the user can confirm they typed
the right phrase; that address is never printed.

Then, in order:

1. **Decision C in `pkc-slip39`, plus the cross-tool suite** — the public
   `split`/`combine` take and return 16–32-byte secrets; the official
   128-bit vectors flip from "rejected" to positive checks; and the
   byte-level half of the cross-tool recovery suite (PLAN.md) proves our
   shares and other tools' shares recover each other. Comes first because
   it changes the public API.
2. **Publish `pkc-slip39` as its own crate** — LICENSE, README, `cargo
   publish`, now with an API that won't break immediately after. Small,
   fills a real gap, and the cheapest way to get outside eyes on the most
   sensitive code.
3. **The single-file build and CI**, before any page rewrite, because it
   shapes how the page code is written (`file://`, inlined wasm through
   `initSync`, `default-src 'none'` CSP with a build-time script hash):
   `rust-toolchain.toml`, pinned `wasm-pack`/`wasm-opt`, the inlining
   script, a reproducibility check (build twice, compare hashes), and CI
   running tests, clippy, `cargo audit`, the wasm build and the cross-tool
   suite. Signing waits for step 6.
4. **Land Decisions A and D in the core and the page**: a real
   seed-phrase pipeline replacing the raw-entropy BIP39 input, at all five
   lengths; the converter out of the page; the passphrase behind
   "Advanced"; a 1-of-1 SLIP-39 mnemonic as the "key as checksummed words"
   replacement in the CLI. Add BIP32/BIP44 derivation (RustCrypto `bip32` on
   the existing `k256`) for the fingerprint (BIP32) and the on-screen
   address check (BIP44). The phrase-level half
   of the cross-tool suite lands here.
5. **The wizard, print cards and recovery instructions** (the open Phase 2
   item, promoted: under this framing it is the product) — the flow in
   "Considerations" above, and a `@media print` layout per share with its
   label, threshold, the fingerprint, and plain-language instructions an
   heir can follow without understanding any of this.
6. **Release**: a release job publishing the HTML with `SHA256SUMS` and a
   signature. Then v1, EthStaker, and the audit-funding application
   described above.
7. **A rename** — `private-key-convertor` no longer describes the tool.

## Prior art consulted
- [`reference/bitcoin-ethereum-private-key-converter.html`](reference/bitcoin-ethereum-private-key-converter.html) — the legacy tool being replaced (bitaddress.org + coinb.in `coinjs` + js-sha3 code, all BSD/MIT).
- [`reference/bip39-standalone.html`](reference/bip39-standalone.html) — `iancoleman/bip39` (MIT), UX reference for mnemonic/entropy input and multi-format output.
- `iancoleman.io/shamir39` — considered and rejected as the Phase 2 scheme (see above); UX reference only.
- [`stake-house/wagyu-key-gen`](https://github.com/stake-house/wagyu-key-gen) — target functionality of the dropped validator-key phases (see "Out of scope").
- Other tools that back up a BIP39 phrase with SLIP-39, used as oracles by the cross-tool recovery suite (PLAN.md): [`trezor/python-shamir-mnemonic`](https://github.com/trezor/python-shamir-mnemonic) (reference implementation, MIT), [`pjkundert/python-slip39`](https://github.com/pjkundert/python-slip39) (`--using-bip39`, GUI and PDF cards, GPL-3.0), [`gavincarr/seedkit`](https://pkg.go.dev/github.com/gavincarr/seedkit) (Go CLI, MIT, unaudited), [`iancoleman/slip39`](https://github.com/iancoleman/slip39) (web tool).
