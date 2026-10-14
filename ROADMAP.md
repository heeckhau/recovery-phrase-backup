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
phrase — see Phase 2 for redundant, split backup shares. (Removed
2026-10-14 by Decision A: words now always mean a wallet's BIP39 phrase,
and the converter is CLI-only.)
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
printed via the browser's native print dialog (no new dependencies, no
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
rapid release cadence) against the auditability priority. If a CLI
`--pdf` lands, it writes one file per share and never all shares (or the
phrase) in one file — see "Printing" in "Considerations — Distribution".

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
- Answer "just use python-slip39" up front too — see "Comparison with
  python-slip39" below.

**Is there enough value?** (To revisit.) As a public good, yes; as a
business, no — and there is no funding plan (dropped 2026-10-12): this is
built and maintained as a personal open-source project. The gap is
narrower than "no tool exists". Tools that back up an existing BIP39
phrase with SLIP-39 already exist: `python-slip39` (compared in detail
below), `seedkit` (a Go CLI whose README says it is unaudited) and
`iancoleman/slip39` (a web tool). As of 2026-10-12, none of them ships as
a reproducible, signed single file that opens offline, and none is designed
around a recovery an heir can carry out. That is the gap: real, but it has
to be argued rather than assumed. Beyond that, the code that matters
already exists and is verified, and the scope is small and chain-agnostic.
The real cost is trust: reproducible builds, signatures, the official
vectors and the cross-tool suite are launch requirements, not polish, and a
backup bug surfaces years later at the worst moment — so this comes with an
open-ended maintenance commitment. v1 does not wait for an external audit
(decided 2026-10-12): instead, independent reviews by several other LLMs
before release (step 6). Those reviews are published with their findings
and fixes, and the project never calls itself "audited" — an LLM review is
a useful second pair of eyes, not a professional audit, and claiming
otherwise would spend the trust the rest of the plan is built to earn. A
human audit stays welcome if one ever becomes possible. Low-regret test: ship a v1 that does only the backup loop as a
single signed file and put it in front of self-custody communities
(EthStaker first, because it is reachable); if after 3–6 months nobody
endorses or uses it, stop — something useful will have been built anyway.

## Comparison with python-slip39 (researched 2026-10-12)

[`pjkundert/python-slip39`](https://github.com/pjkundert/python-slip39) is
the closest existing tool: it backs up an existing BIP39 phrase's entropy
as SLIP-39 shares (`--using-bip39`), the same scheme as Decision A, so the
two should recover each other's shares (the cross-tool suite will prove
it). Active: latest release 14.1.0 on 2026-01-06 (PyPI).

**What it does:**
- *Create*: generates new seeds (`secrets.token_bytes`, optional extra
  entropy via `--entropy`, with an entropy-quality analysis that watermarks
  the PDF when the entropy looks weak), or backs up an existing phrase or a
  128/256/512-bit hex secret. Only those three lengths: it refuses to
  create shares for 15-, 18- and 21-word phrases (confirmed by the
  cross-tool suite), though it recovers such shares made elsewhere.
- *Output*: one PDF of cut-out cards in several sizes (business, credit,
  index, …), double-sided, with QR codes; group/threshold labels per card;
  a cover page carrying the BIP39 phrase. Cards show derived addresses and
  QR codes unless `--anonymous` is set.
- *Coins*: ETH, BTC (legacy, SegWit, native SegWit — the default), LTC,
  DOGE, BSC and XRP (beta), via `hdwallet`.
- *Recovery*: `slip39-recovery` ignores invalid and duplicate shares and
  can return the BIP39 phrase or the raw entropy. Like ours, it accepts
  surplus shares, where the reference implementation rejects them.
- *Extras*: BIP-38 and encrypted-JSON paper wallets, xpub output, an
  address-stream generator, serial-port output, and an "invoice" extra
  (web3, solc, a local Ethereum node).
- *Distribution*: `pip install slip39`, or a desktop app built with
  PyInstaller on FreeSimpleGUI/Tk. Since v14 the packaged app supports only
  macOS on Apple Silicon; elsewhere it is pip or Nix from source.

**Trust chain, the part that matters most for a backup tool:**
- Core dependencies include the author's own forks of the libraries that
  do the sensitive work — `shamir-mnemonic-slip39` (of Trezor's reference
  implementation), `hdwallet-slip39` (a 200+-coin HD wallet library),
  `tabulate-slip39` — plus `crypto-licensing`, a module for "Ed25519-signed
  license checking and automatic issuance after cryptocurrency payment".
  Some advanced features need a paid license tied to an agent identity and
  a machine ID; a secret-handling tool that carries payment and licensing
  code is a harder thing to audit and to trust.
- GPL-3.0 (one package index lists it as GPL-3.0 plus a proprietary
  license). No audit or reproducible build is advertised.
- Output is one PDF holding every share — and, with `--using-bip39`, the
  phrase itself on the cover page. That single file recovers everything,
  which is exactly what splitting was meant to prevent (see "Printing" in
  "Considerations — Distribution").

**Where this project differs** (the pitch, in order of weight):
1. *Runs anywhere offline with nothing to install.* One HTML file, any OS,
   any modern browser, including Tails, Linux and Intel Macs — where
   python-slip39 needs pip or Nix, which in practice means a network
   connection on the very machine that should never have one.
2. *A short trust chain you can check.* A small Rust codebase on a handful
   of mostly RustCrypto crates, an independent SLIP-39 implementation (not a
   wrapper of the reference), reproducible build, published hash and
   signature, MIT/Apache-2.0. No licensing, payment or network code at
   all.
3. *Private cards by default.* A fingerprint, never an address or QR code
   (Decision D): a found share reveals nothing about the funds.
4. *The recovery process, not just the cards.* Per-share write-and-verify,
   a recovery test before finishing, labelled cards with instructions an
   heir can follow. The tool never creates a file holding enough shares to
   recover the phrase, and is designed so the safe path is the easy one.
5. *Compatible, not competing.* Shares made here recover in python-slip39,
   seedkit and the reference implementation, and theirs recover here —
   tested both ways by the cross-tool suite (PLAN.md) — so choosing this
   tool never locks anyone in.
6. *Every phrase length.* All five BIP39 lengths, where python-slip39
   handles only 12 and 24 words.

**Gaps the comparison exposed in this plan** — its features, and one
thing neither tool does — and what to do about each:
- *Adopt* — guidance on the original phrase after splitting (python-slip39's
  README doesn't cover this either; a backup that leaves the single
  original copy in a drawer hasn't removed the risk); BIP39-passphrase ("25th word")
  awareness; the on-screen address check in the formats people recognise,
  including Bitcoin native SegWit (m/84'); a warning for weak or known
  phrases (test vectors, repeated words). See "Considerations — UI".
- *Consider for later* — generating a new phrase directly as shares, so a
  single copy never exists at all. It needs trustworthy entropy and an
  entropy check; it is the strongest form of the product, but not v1.
- *Deliberately not* — addresses or QR codes on cards (privacy; and QR
  scanning needs a camera on the recovery machine); a single file holding
  every share, or the phrase on any printed page; paper
  wallets; xpub, address streams, serial output, invoicing (not backup);
  multi-coin derivation beyond the address check; non-English wordlists
  (out of scope for now).

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
- **The address check speaks the user's format**: Ethereum
  (m/44'/60'/0'/0/0) and Bitcoin native SegWit (m/84'), not only legacy
  P2PKH, which few Bitcoin users would recognise as theirs.
- **The BIP39 passphrase ("25th word")**: an optional field used *only*
  for the address check, so users who have one see their real address.
  It is not part of the backup, and the page says so plainly: the shares
  recover the words; the 25th word needs its own backup.
- **Weak or known phrases get a warning** — published test vectors
  ("abandon … about"), repeated words, obviously low-entropy input —
  before anything is split.
- **What to do with the original.** After a verified backup, the wizard
  raises a question python-slip39's README never addresses: a backup that leaves the
  single original copy in a drawer hasn't removed the risk it exists to
  remove. It explains the trade-off and lets the user choose; it doesn't
  pretend the original is gone.
- **Wizard, not a form**, because the process is the product: choose a
  scheme from plain-language presets ("2 of 3: any two shares recover; one
  alone reveals nothing", "3 of 5"), groups and passphrase behind
  Advanced → review → shares shown **one at a time** with a "write it
  down, then re-enter it" verification per share (what Trezor does; it
  catches the transcription error that would otherwise surface in ten
  years) → a "test your recovery now" step. Recovery: shares one at a
  time with the existing inspect badge, then the phrase and its
  fingerprint. *Settled 2026-10-14 (from the step 5 sketch):* the
  per-share check is a spot check of three random words — full re-entry
  of 20–33 words per share is the kind of step people skip — and the
  "test your recovery" step is the full check, with a threshold of shares
  typed in completely and the fingerprint reproduced. Entry is a numbered
  word grid with per-word autocomplete and validation (paste into the
  first box still fills it). Cards carry no per-share label: a name tells
  a finder whose share it is.
- **Print cards**: one share per page, labelled ("Share 2 of 3 — any 2
  recover"), numbered words, the fingerprint, and recovery instructions an
  heir can follow; plus a secret-free "how to recover" sheet. Offer blank
  handwriting templates as well as filled cards. Each share is printed on
  its own, from its own wizard step — there is no "print all" — and the
  phrase never appears on any printed page.
- **Hygiene**: a "clear everything" button, fields wiped when the page
  closes, no storage of any kind, no copy-to-clipboard for shares, a
  `navigator.onLine` warning banner. The `?test` prefill goes away in
  release builds. *Not* clearing on tab hide (settled 2026-10-14):
  usability comes first — people switch to their wallet mid-wizard to
  compare addresses — and the device is trusted for the session anyway
  (malware on it would see every keystroke); clearing only guards
  against a screen left open, which the button covers.
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
  `wasm-unsafe-eval`. 516 KB as built (step 3). Reproducible: pinned
  toolchain, no wasm-pack or wasm-opt, published on GitHub Releases with
  `SHA256SUMS` and a signature.
  The user flow is iancoleman's: download once, verify, disconnect, open —
  and keep the file.
- **Don't host the real thing casually.** A hosted copy undermines the
  story and invites online use; a demo page, if any, should accept only
  test vectors or nag hard. The recommended environment is a live USB
  (Tails) or a fresh browser profile with **no extensions** — extensions
  are the browser's real attack surface for a page like this, and they are
  the honest price of skipping a native app.
- **No desktop app.** The tool itself writes nothing to disk — shares are
  printed or handwritten — so it needs no installers, no code signing, no
  per-OS builds. With validator keys out of scope nothing else would need
  one either; the only native path is the CLI.
- **Printing is the one output, and where the hygiene lives** (revised
  2026-10-12). The risk is not a PDF as such; it is a *lasting copy of
  enough shares to recover the phrase, in one place* — that undoes the
  split. A file outlives the session in ways typing the phrase does not:
  Desktop/Documents folders often sync to OneDrive or iCloud, backups and
  recent-file lists keep copies, and infostealers search files for
  recovery phrases. A single share in a file reveals nothing.
  Two limits on what the page can promise: every browser's print dialog
  offers "Save as PDF", so the page cannot prevent a file — it can only
  avoid making one and warn; and printing itself spools the job to disk
  (and some printers keep jobs or route them through a cloud service), so
  "print" and "save as PDF" differ less than they seem. On a live USB
  (Tails) all of this disappears at shutdown.
  So the design aims at the specific risk rather than at files, without
  friction that would push people to worse shortcuts (a phone photo of the
  phrase): one share per print and no "print all", so a careless save
  captures one harmless share; the phrase never on any printed page; a
  short warning at the print step — a saved file is a copy that may sync
  to the cloud, so print and don't keep it, and prefer a directly
  connected printer; Tails recommended for the cautious; handwriting
  templates as the alternative.
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
from the code review, C and D from the product framing above. All four
are settled: B and C on 2026-10-12, A and D on 2026-10-14.

**Decision A — what 24 words mean.** Settled 2026-10-14: 24 words always means a
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

**Decision C — every BIP39 length, not just 24 words.** Settled
2026-10-12. The product accepts 12/15/18/21/24-word phrases
(128/160/192/224/256-bit entropy): MetaMask's default is 12 words, so a
24-word-only tool excludes most of the target audience. Recovery shows the
phrase at its original length. The crate is more general than the product:
`pkc-slip39` accepts every length SLIP-39 allows (at least 128 bits, a
multiple of 16), including 512-bit BIP32 seeds that other tools such as
python-slip39 can produce, so it never refuses shares another tool made
validly. The five BIP39 lengths are a product rule, enforced where phrases
are entered (step 4), not in the library. Done in the crate (commit
`a04a520`); lifting the old 256-bit restriction also exposed a missing
"group threshold ≤ group count" check, now fixed (`7a0ac57`).

**Decision D — SLIP-39's own passphrase.** SLIP-39 encrypts the secret
under a passphrase that is separate from BIP39's "25th word", and a wrong
one silently yields a different secret with no error — for a backup meant
to be recovered by an heir in ten years, a trap. Settled 2026-10-14: no
passphrase by default — the field starts empty (the threshold is the
security) and kept behind "Advanced" with the trap spelled out; a
verification fingerprint is printed on every share card. (Deniability, the property the unauthenticated design
buys, is not something this product promises.) Evidence from the
cross-tool suite: python-slip39's BIP39 mode never sets a SLIP-39
passphrase at all; and all three other tools default to iteration exponent
1 where ours used 0. Settled 2026-10-12: the CLI and the page now default
to 1 (the spec itself sets no default).

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
   `split`/`combine` take and return secrets of any SLIP-39 length; the
   official 128-bit vectors flip from "rejected" to positive checks; and
   the byte-level half of the cross-tool recovery suite (PLAN.md) proves
   our shares and other tools' shares recover each other. Comes first
   because it changes the public API. *Done (2026-10-12): API, vectors,
   and the suite — 250/250 checks against python-shamir-mnemonic,
   python-slip39 and seedkit (`scripts/interop/run.sh`). Its CI job lands
   with step 3.*
2. **Publish `pkc-slip39` as its own crate** — *postponed (2026-10-12).*
   Nothing needs it published: the workspace builds it locally, and the
   outside eyes it was meant to attract come from the step 6 reviews
   instead. Publishing to crates.io is public and permanent, so it waits
   until the API has been through the phrase pipeline (step 4) and there
   is a reason to. The LICENSE files it would have added move to step 6,
   which needs them for the release anyway.
3. **The single-file build and CI**, before any page rewrite, because it
   shapes how the page code is written (`file://`, inlined wasm through
   `initSync`, `default-src 'none'` CSP with a build-time script hash):
   `rust-toolchain.toml`, pinned `wasm-pack`/`wasm-opt`, the inlining
   script, a reproducibility check (build twice, compare hashes), and CI
   running tests, clippy, `cargo audit`, the wasm build and the cross-tool
   suite. Signing waits for step 6. *Done (2026-10-12):* Rust pinned to
   1.99.0; `python3 scripts/build.py` writes
   `dist/private-key-convertor.html` (516 KB), which opens from `file://`
   under `default-src 'none'` with the script and stylesheet allowed only
   by hash, and `--verify-reproducible` rebuilds from clean and compares
   bytes. wasm-pack and wasm-opt are both gone from the release path —
   cargo plus `wasm-bindgen` pinned to `Cargo.lock` does the same job with
   two fewer tools to trust. Local paths are remapped out of the wasm,
   including the toolchain's `rust-src`, which otherwise leaks the
   builder's home directory. The `?test` prefill is stripped from the
   release file. CI (`.github/workflows/ci.yml`, actions pinned by
   commit) runs all of the above.
   *Reproducible per platform, so one reference platform.* Builds are
   byte-identical on the same host platform but not across platforms
   (macOS ARM, Linux ARM and Linux x86-64 each gave a different file):
   the host feeds into Cargo's crate hashes, which order functions and
   data in the wasm. So the reference build is
   `scripts/build-reproducible.sh` — `build.py` in one Linux x86-64
   container pinned by digest. CI runs exactly that, and the same
   container under emulation on a Mac gives CI's bytes
   (`08a7de19…` for commit `6d66e00`). Release verification (step 6)
   means: run that script, compare the hash.
4. **Land Decisions A and D in the core and the page**: a real
   seed-phrase pipeline replacing the raw-entropy BIP39 input, at all five
   lengths; the converter out of the page; the passphrase behind
   "Advanced"; a 1-of-1 SLIP-39 mnemonic as the "key as checksummed words"
   replacement in the CLI. Add BIP32/BIP44 derivation (RustCrypto `bip32` on
   the existing `k256`) for the fingerprint (BIP32) and the on-screen
   address check (BIP44). The phrase-level half
   of the cross-tool suite lands here. *Done (2026-10-14):* `pkc-core`'s
   `Phrase` (all five lengths; fingerprint and first ETH / native SegWit
   addresses, checked against BIP-84's vector, the `73c5da0a` fingerprint
   and MetaMask's first account) replaces the raw-entropy encoding, and
   key parsing rejects words outright. The page is now "back up / recover a
   phrase", with a live fingerprint-and-address check before splitting, an
   optional BIP39 passphrase that only affects that check, and the SLIP-39
   passphrase behind Advanced, empty by default. The CLI's `slip39 split`
   takes a phrase or a raw key and says which; the 1-of-1 words backup is
   `--group-threshold 1 --group 1:1`. Cross-tool suite: 266/266, including
   phrase identity both ways. Left for step 5: the weak-phrase warning and
   the "what to do with the original" guidance (wizard steps).
5. **The wizard, print cards and recovery instructions** (the open Phase 2
   item, promoted: under this framing it is the product) — the flow in
   "Considerations" above, and a `@media print` layout per share with its
   label, threshold, the fingerprint, and plain-language instructions an
   heir can follow without understanding any of this — printed one share
   at a time, with the print-step warning. *Done (2026-10-14):* backup
   is six steps — phrase (word grid, live fingerprint/address check,
   test-phrase warning needing an explicit "back it up anyway"), scheme
   (2-of-3, 3-of-5 or custom, with guardrails for 3-of-3-style schemes),
   review, one share at a time with a three-word spot check, a full
   recovery test that must reproduce the fingerprint, and guidance on the
   original phrase. Cards print one per page, or blank for handwriting;
   the "how to recover" sheet carries no secret. Recovery takes shares one
   at a time with progress towards the threshold and confirms the
   fingerprint before showing addresses. Seven Playwright tests drive the
   release file itself in CI (`tests/e2e/`), including that nothing
   reaches the clipboard or storage and the strict CSP never fires; a
   planted spot-check bug fails exactly the test that should catch it.
6. **Independent review, then release.** Reviews of `pkc-slip39`, the
   page and the build by several other LLMs, each given the code and the
   threat model (offline, single file, real recovery phrases); findings and
   the fixes for them recorded in the repo (e.g. `reviews/`), with the
   reviewing model and the commit reviewed. Then LICENSE files, a README,
   a release job publishing the HTML with `SHA256SUMS` and a signature,
   v1, and EthStaker.
7. **A rename** — `private-key-convertor` no longer describes the tool.

## Prior art consulted
- [`reference/bitcoin-ethereum-private-key-converter.html`](reference/bitcoin-ethereum-private-key-converter.html) — the legacy tool being replaced (bitaddress.org + coinb.in `coinjs` + js-sha3 code, all BSD/MIT).
- [`reference/bip39-standalone.html`](reference/bip39-standalone.html) — `iancoleman/bip39` (MIT), UX reference for mnemonic/entropy input and multi-format output.
- `iancoleman.io/shamir39` — considered and rejected as the Phase 2 scheme (see above); UX reference only.
- [`stake-house/wagyu-key-gen`](https://github.com/stake-house/wagyu-key-gen) — target functionality of the dropped validator-key phases (see "Out of scope").
- Other tools that back up a BIP39 phrase with SLIP-39, used as oracles by the cross-tool recovery suite (PLAN.md): [`trezor/python-shamir-mnemonic`](https://github.com/trezor/python-shamir-mnemonic) (reference implementation, MIT), [`pjkundert/python-slip39`](https://github.com/pjkundert/python-slip39) (`--using-bip39`, GUI and PDF cards, GPL-3.0; see "Comparison with python-slip39"), [`gavincarr/seedkit`](https://pkg.go.dev/github.com/gavincarr/seedkit) (Go CLI, MIT, unaudited), [`iancoleman/slip39`](https://github.com/iancoleman/slip39) (web tool).
