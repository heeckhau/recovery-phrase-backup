# Roadmap

A tool to back up a wallet's BIP39 recovery phrase as SLIP-39 shares, so that
no single place can lose it, or reveal it. One HTML file that runs offline,
plus a command-line tool. See [README.md](README.md) for what it does and how
to use it.

**Guiding principle: auditability first.** This handles real recovery
phrases, so every choice (dependencies, architecture, UI) favors the shortest
possible trust chain over convenience or polish: minimal, well-known
dependencies; a plain static page over a framework; code someone can read end
to end over code that's merely tested.

## Where it stands (2026-10-15)

Complete and tested, not yet released.

- **`rpb-slip39`**: SLIP-39 written from the spec (no suitable Rust crate
  exists; see History), checked against all 45 official vectors. Accepts
  every length SLIP-39 allows. Secret material is zeroized; the share digest
  and GF(256) lookups are constant-time.
- **`rpb-core`**: BIP39 phrases at all five lengths, their BIP32 fingerprint
  and first Ethereum and native SegWit addresses, and a weak-phrase check;
  raw keys (hex, WIF) for the CLI.
- **The page**: backup and recovery wizards with spot checks, a recovery
  test, print cards (or handwriting), and a terminal look. Details under
  Decisions.
- **The CLI** (`rpb`): split, combine and inspect shares, convert raw keys;
  secrets are prompted for with hidden input, never taken from argv.
- **Trust**: a single-file release build that is reproducible (byte-identical
  in a pinned Linux x86-64 container), and CI on every push: tests, clippy,
  `cargo audit`, the reproducible build, the cross-tool suite (266/266
  checks against three other SLIP-39 tools), ten browser tests against the
  release file itself, and a check that no em dash appears in the repo.

Not done yet: the independent review and a signed release. Until then the
website offers the latest tested build of `main` for download, with its
SHA-256.

## Next

1. **Independent review.** Several other LLMs each review `rpb-slip39`, the
   page and the build, given the code and the threat model (offline, single
   file, real recovery phrases). Findings are fixed as they come in, like
   any other bug. v1 does not wait for a professional audit, and the
   project never calls itself "audited": an LLM review is a useful second
   pair of eyes, not an audit, and claiming otherwise would spend the trust
   the rest of the plan earns. A human audit stays welcome if one ever
   becomes possible.
2. **A name.** *Done (2026-10-15):* Recovery Phrase Backup. The
   repository and release file are `recovery-phrase-backup`, the crates
   `rpb-*` and the command `rpb` (formerly `private-key-convertor`, `pkc-*`
   and `pkc`). Chosen over shorter names because it says what the tool does
   and is free everywhere; "key" names were dropped because the tool backs
   up phrases, not keys.
3. **Release v1.** LICENSE files (*done 2026-10-15*: MIT OR Apache-2.0);
   make the repository public (*done 2026-10-15*); a release job
   that builds the HTML in the reference container and publishes it with
   `SHA256SUMS` and a signature; verification steps in the README.
4. **A website.** *Online (2026-10-15)* at
   `heeckhau.github.io/recovery-phrase-backup` (no custom domain: a lapsed
   domain could serve a fake tool, and the cards and recovery sheet name no
   URL anyway). `scripts/build.py` writes it to `dist/site/`, CI builds it
   in the reference container and tests it, and CI's `pages` job deploys
   those tested files from `main`. Until v1 it also offers the release file
   itself for download, with its SHA-256: a moving latest build, but better
   than no download at all.
   - *A demo build* (`dist/demo.html`): the same page, but it only backs
     up a few published sample phrases (offered as buttons), refusing any
     other at its first word that isn't a sample's, with a pointer to the
     download. A permanent "demo, sample data only" banner, and "DEMO, NOT A
     REAL BACKUP" across every printed card. Backing up arbitrary phrases
     would invite real use online, which the whole design argues against.
     Recovery warns as soon as a typed share isn't a sample, but still
     recovers it, with a warning to treat the phrase as exposed: once the
     words are typed, refusing protects nothing and only fails someone who
     may be in a real emergency. The two sample
     phrases are derived from fixed labels, and `rpb-core` flags them in the
     real tool like any published test phrase.
   - *A landing page*: the problem, how splitting solves it, "try the demo",
     how to use it for real (download, verify, go offline), the comparisons
     below, and recovery without this tool. Same terminal style, no
     analytics.
   - *An explainer animation*: a short SVG/CSS loop in the page's style. A
     phrase card splits into three; they go to three places; one burns and
     the other two still recover the phrase; a thief finds one and learns
     nothing. With reduced motion the same story is told as text. A
     commissioned hand-drawn cartoon can replace it later.
5. **Outreach, with a stopping rule.** Put v1 in front of self-custody
   communities, EthStaker first because it is reachable. If after 3 to 6
   months nobody endorses or uses it, stop: something useful will have been
   built anyway.

## Later, maybe

- **Publish `rpb-slip39` on crates.io.** Postponed: nothing needs it
  published, and publishing is public and permanent. Worth doing once the
  API has settled and someone else could use it; there is still no audited,
  permissively licensed Rust SLIP-39 crate.
- **Generate a new phrase directly as shares**, so a single copy never
  exists at all. The strongest form of the product, but it needs trustworthy
  entropy and an entropy check.
- **A CLI `--pdf`.** [fullbleed](https://docs.fullbleed.dev/getting-started/rust/)
  (HTML/CSS to PDF, MIT) was evaluated on 2026-10-12 at v2.5.22: it renders a
  card natively in about 12 ms, but panics in the browser
  (`std::time::Instant::now()`) and yields a ~6 MB wasm, and its ~231k lines
  in one young crate weigh against auditability. If it lands, the CLI writes
  one file per share, never all shares in one file.
- **A smarter word grid.** Nice to have; the grid works, this only saves
  typing. Ideas so far, none decided:
  - *Complete as you type.* As soon as the letters typed can only be one
    word, show the rest of it selected (as a browser's address bar does):
    typing on over the matching letters keeps it, any other letter drops
    it, space or Enter accepts it. No jumping to the next cell, or a word
    typed out in full spills over. Three letters already identify 687 of the
    2048 BIP39 words and 431 of the 1024 SLIP-39 words; today's completion
    always needs four. Android keyboards may hold back the completion until
    space, which is no worse than now.
  - *Offer a fix for a typo, never apply it silently.* A red cell whose word
    is one typo away from exactly one word offers it ("uncover?") as a
    button or on Enter. One typo points to a single word 99% of the time in
    SLIP-39 (its list is built so one typo never turns a word into another)
    and 90% in BIP39, where 1,624 typos land on another valid word that no
    check can catch. Silent correction is out: a wrong fix passes a 12-word
    phrase's checksum 1 time in 16 and backs up the wrong wallet, and in the
    spot check it would hide the very mistake the check exists to find.
    Open: whether the spot check offers fixes at all.
  - *Find a wrong word in a share.* A share whose words are all valid but
    whose checksum fails most likely has one misread word. RS1024 detects
    any 3 wrong words, so a single one can be located without ambiguity:
    trying every word in every position (about 20,000 checksums, instant)
    finds it. Point to the position only ("word 7 doesn't match the
    checksum; compare it with your copy"), not the word that would fit: the
    right word is the one on the paper, and an offered word invites
    accepting it without looking. With two wrong words the search finds
    nothing and the message stays as it is; with three or more it can,
    rarely, point to a word that is right, which comparing with the copy
    shows. BIP39's 4 to 8 checksum
    bits are too weak for this.
  - *A suggestion list of our own* instead of the browser's `<datalist>`,
    which filters by prefix only, looks different in every browser and can
    show neither typo fixes nor ranked candidates.
- **Tests for the CLI and the wasm bindings.** The risk sits in `core` and
  `slip39`, which are tested, and the page is covered by browser tests; the
  thin layers in between are not.
- **Out of scope for now**: non-English wordlists (MetaMask and Trezor are
  English-only in practice), mobile, a desktop app, and validator keys (see
  History).

## Product

**Audience: self-custody holders.** Anyone who holds a BIP39 recovery phrase,
whatever wallet or chain. Stakers are included without special treatment:
their validator mnemonic is a BIP39 phrase like any other.

**The problem.** A recovery phrase on paper or metal is one copy, and one copy
fails in both directions: burn it and it is gone, find it and it is stolen.
The usual mitigations each move the risk rather than remove it. A BIP39
passphrase adds a second secret that itself needs a backup. Multisig spreads
*signing*, not the backup, and is complex and chain-specific. Hardware-wallet
Shamir (Trezor) requires that hardware and replaces the phrase rather than
backing it up. Custodial recovery (Ledger Recover) hands the secret to third
parties.

**The message.**

> Back up your recovery phrase so that no single place can lose it, or
> reveal it.

**What the product has to get right:**
- Say precisely that this is a *backup of an existing BIP39 phrase*, not
  Trezor's SLIP-39-as-seed scheme. Restoring these shares directly on a
  Trezor opens a different wallet; recovering the phrase and restoring it is
  what makes the backup work with every wallet.
- Support every BIP39 length, above all 12 words (MetaMask's default).
- Treat the process as the product. Shamir backups fail operationally:
  shares stored together, heirs who don't know shares exist, a 3-of-3
  threshold that is worse than a single copy, shares nobody can identify.
  The safety is in the distribution and the recovery, not in the paper.
- Answer "just buy a Trezor" (no hardware, works for the phrase you already
  have, open and verifiable; Ledger and MetaMask users have no Shamir
  option) and "just use python-slip39" (see How it compares).

**Is there enough value?** As a public good, yes; as a business, no. There
is no funding plan: this is a personal open-source project with an
open-ended maintenance commitment, because a backup bug surfaces years later
at the worst moment. The gap is narrower than "no tool exists" (see How it
compares): none of the existing tools ships as a reproducible, signed single
file that opens offline, and none is designed around a recovery an heir can
carry out.

## Decisions

**A. Words always mean a BIP39 phrase.** 12 to 24 BIP39 words are always a
wallet's recovery phrase (entropy, then PBKDF2 seed, then BIP32/44). SLIP-39
backs up the phrase's entropy, so shares recover the words and every key
derived from them. The earlier encoding of a raw private key as 24 BIP39
words is gone: indistinguishable from a seed phrase, it was a loss-of-funds
trap. Its two jobs have better standard answers. As an interchange format
for a raw key: hex for Ethereum and WIF for Bitcoin, always shown with the
derived address, which catches a typo. As checksummed words on paper: a
1-of-1 SLIP-39 share (`rpb slip39 split --group-threshold 1 --group 1:1`),
which uses a different wordlist (so no wallet mistakes it for a seed phrase)
and a far stronger checksum (RS1024, three words, versus BIP39's 8 bits). The
rule: **BIP39 wordlist, a seed phrase; SLIP-39 wordlist, a backup of some
secret; hex or WIF, a raw key.** Because SLIP-39 does not record what the
bytes are, the CLI says whether it is backing up a phrase or a raw key, and
recovery shows a 32-byte result both ways.

**B. One signed offline HTML file, plus the CLI.** The page is one file with
the wasm inlined, opening from `file://` under `default-src 'none'` with its
script and stylesheet allowed only by hash. No desktop app: the tool writes
nothing to disk, so nothing needs installers or code signing. The hex/WIF
converter stays in the CLI: a raw-key input next to a phrase input is
confusion and attack surface the backup flow doesn't need.

**C. Every length.** The page accepts all five BIP39 lengths and recovers
the phrase at its original length. The library is more general: `rpb-slip39`
accepts every length SLIP-39 allows (including 512-bit seeds that other tools
produce), so it never refuses shares another tool made validly.

**D. No SLIP-39 passphrase by default.** SLIP-39 can encrypt under its own
passphrase, separate from the wallet's "25th word", and a wrong one silently
yields a different secret. For a backup meant to be recovered by an heir in
ten years, that is a trap, so the field starts empty, sits behind Advanced,
and explains the risk; the threshold is the security. The iteration exponent
defaults to 1, matching the reference implementation, python-slip39 and
seedkit (the spec sets no default). Deniability, the property the
unauthenticated design buys, is not something this product promises.

**The fingerprint.** Every card carries the phrase's BIP32 master-key
fingerprint (4 bytes of hash160 of the master public key, empty BIP39
passphrase), as 8 hex characters. It shows that shares belong together, and
a correct recovery must reproduce it, which is how a wrong passphrase or a
mistyped share shows up. It is never an address: an address on every card
would let anyone who finds one share look up the balance. Bitcoin wallets
such as Sparrow and Coldcard show the same value, so for their users it links
a card to their wallet setup, not to their funds. At 32 bits it gives no
practical help to anyone guessing the phrase.

**The backup wizard.** Six steps: the phrase, the scheme, a review, the
shares one at a time, a recovery test, and done.
- The phrase goes into a numbered word grid with autocomplete (both
  wordlists are unique by four letters); paste still works. Its fingerprint
  and first Ethereum and native SegWit addresses appear on screen only, so
  the user can confirm it is their wallet. An optional wallet passphrase
  field changes only those addresses and says it is not part of the backup.
  Published test phrases, single-byte patterns and phrases with few distinct
  words get a warning and need an explicit "back it up anyway".
- Schemes come as presets (2 of 3, 3 of 5) or custom groups, with warnings
  for schemes weaker than they look (3 of 3, every group required, 1 of 1).
- Each share gets a spot check of three random words, with live feedback
  (green when a word matches, red once a complete word doesn't), moving on
  by itself when all three match. Full re-entry of 20 to 33 words per share
  is the kind of step people skip. Shares can be revisited: back always,
  forward only past a checked share. Skipping the checks takes a dialog and
  a second confirmation ("I'll test restoring from my copies myself before I
  rely on them") and leads to the recovery test.
- The recovery test is the real proof: a threshold of the written or printed
  shares typed in fully, reproducing the fingerprint. It can be skipped.
- The last step asks what to do with the original phrase: a backup that
  leaves the single original in a drawer hasn't removed the risk it exists
  to remove. It explains the trade-off and leaves the choice to the user.

**Recovery.** Shares go in one at a time, with progress towards the
threshold; then the phrase, and a fingerprint check before any address is
shown. Surplus shares are ignored once they are confirmed to belong to the
same backup (the spec and the reference implementation reject them instead).

**Recording and printing.** Before seeing any share, the user picks how to
record them: **by hand** (the default; optional blank cards to write on;
nothing secret goes through a printer) or **printed**, all shares in one job,
as one sheet with dashed cut lines or one card per page. The choice can be
switched while recording, keeping the same shares and checks: going back to
choose again would split again and orphan any card already made. The risk with
printing is a lasting copy of enough shares to recover the phrase in one
place: until the cards are cut apart and separated (and in any saved file or
print spool), the printout is the whole phrase. The page can't prevent this
(every print dialog offers "Save as PDF", and printing itself spools to disk),
so it says so plainly at the print step: a home printer on your own network
is fine, avoid office, shared and cloud printers, never save as PDF, cut the
cards apart right away. Printing one share per job used to make a careless
save capture only one harmless share, but it was tedious, and the gain is
small for a home printer. On a live USB (Tails) all of this disappears at
shutdown.

**Share cards.** A card carries the numbered words, the fingerprint, and a
note to whoever keeps it: someone trusted you with this; it is useless alone
but needed, so keep it safe and private, don't copy or photograph it, and
give it back only to the owner or whoever they named. It carries nothing a
finder could plan with: no position ("share 2 of 3"), no threshold, no group,
no passphrase hint, no mention of SLIP-39 or a wallet. (A share's first words
still encode its group and threshold for SLIP-39 software; that is part of
the standard.) Those details go on the secret-free "how to recover" sheet the
owner keeps for their heir.

**Hygiene.** A "Clear all" button, fields wiped when the page closes, no
storage of any kind, no clipboard writes, a banner whenever the machine is
online (with the example of a malicious browser extension), and the dev-only
`?test` prefill stripped from the release file. The page does not clear on
tab hide: people switch to their wallet mid-wizard to compare addresses, and
the device is trusted for the session anyway (malware on it would see every
keystroke), so clearing would only guard against a screen left open, which
the button covers.

**Reproducible builds, per platform.** The release build pins Rust (1.99.0)
and `wasm-bindgen` (to `Cargo.lock`), uses neither wasm-pack nor wasm-opt,
and remaps local paths (including the toolchain's `rust-src`) out of the
wasm. Builds are byte-identical on one host platform but not across platforms
(macOS ARM, Linux ARM and Linux x86-64 each gave a different file), because
the host feeds into Cargo's crate hashes, which order functions and data. So
the reference build is `scripts/build-reproducible.sh`: one Linux x86-64
container pinned by digest. CI uses it, and the same container on a Mac
reproduces CI's bytes.

## How it compares

**[python-slip39](https://github.com/pjkundert/python-slip39)** (researched
2026-10-12, latest release 14.1.0 of 2026-01-06) is the closest tool: it
backs up a BIP39 phrase's entropy as SLIP-39 shares, the same scheme as
Decision A, and the cross-tool suite confirms the two recover each other's
shares. It generates new seeds, prints one PDF of cut-out cards (with
addresses and QR codes unless `--anonymous`), and adds paper wallets, xpub
output and invoicing. But it only creates shares for 12- and 24-word phrases;
its core dependencies include the author's own forks of the sensitive
libraries plus a licensing module that issues paid licenses after
cryptocurrency payment; it is GPL-3.0 with no advertised audit or
reproducible build; its packaged app supports only macOS on Apple Silicon
(elsewhere it is pip or Nix, which in practice means a network on the
machine that should never have one); and its PDF holds every share, plus the
phrase on the cover page.

**Others**: [seedkit](https://pkg.go.dev/github.com/gavincarr/seedkit) (a Go
CLI, BIP39 phrase to shares and back, unaudited by its own README) and
[iancoleman/slip39](https://github.com/iancoleman/slip39) (a web tool).

**Where this project differs:**
1. *Runs anywhere offline with nothing to install*: one HTML file, any OS,
   any modern browser, including Tails.
2. *A short trust chain you can check*: a small Rust codebase on a handful
   of mostly RustCrypto crates, an independent SLIP-39 implementation,
   reproducible builds, MIT/Apache-2.0, and no licensing, payment or network
   code.
3. *Private cards*: a fingerprint, never an address or QR code, and nothing
   on a card that helps a finder.
4. *The recovery process, not just the cards*: spot checks, a recovery test,
   and instructions an heir can follow.
5. *Compatible, not competing*: shares recover in python-slip39, seedkit and
   the reference implementation, and theirs recover here, tested both ways,
   so choosing this tool never locks anyone in.
6. *Every phrase length*: all five BIP39 lengths.

Deliberately not: addresses or QR codes on cards, a PDF the tool writes
itself, the phrase on any printed page, paper wallets, xpub, invoicing, and
multi-coin derivation beyond the address check.

## History

- **2026-09, Phase 1: a converter.** Started as a rewrite of
  `reference/bitcoin-ethereum-private-key-converter.html` (a legacy
  single-file JS tool) as a Rust/WASM workspace (`core`, `cli`, `wasm`,
  `web/`), fixing its gaps: WIF checksum validation, compressed keys,
  testnet, EIP-55 casing. It also encoded a raw key as 24 BIP39 words, later
  removed by Decision A.
- **2026-09, Phase 2: SLIP-39.** No suitable Rust crate existed: the one with
  a compatible license called itself "untested work-in-progress", and the
  maintained one is GPL-3.0. So `rpb-slip39` was written from the spec and
  checked against the official vectors. A security review on 2026-09-10 to
  14 fixed seven findings, including secrets passed as CLI arguments,
  non-constant-time comparisons and lookups, and a missing CSP.
- **2026-09-14 to 16: a review and a product.** A review of the code found
  the raw-key words colliding with seed phrases, no BIP32 derivation and no
  trust story (no README, license, CI or reproducible build). It led to the
  product framing above and Decisions A to D. Two directions were dropped:
  validator key generation (wagyu-key-gen's functionality, experimented with
  in `heeckhau/wagyu-key-gen`; a different product with slashing-level
  stakes, and stakers are already covered by phrase backup) and a funding
  plan.
- **2026-10-12 to 14: building it.** Every SLIP-39 length in the library
  (which exposed and fixed a missing "group threshold at most group count"
  check); the cross-tool suite; the reproducible single-file build and CI;
  BIP39 phrases with fingerprint and addresses in place of the raw-key
  words; the wizards, print cards and browser tests; then rounds of
  refinement (printing choices, share navigation, keeper-facing cards, live
  spot checks, the terminal theme, and no em dashes).

## Prior art

- [`reference/bitcoin-ethereum-private-key-converter.html`](reference/bitcoin-ethereum-private-key-converter.html):
  the legacy tool this started from (bitaddress.org, coinb.in `coinjs` and
  js-sha3 code, all BSD/MIT).
- [`reference/bip39-standalone.html`](reference/bip39-standalone.html):
  `iancoleman/bip39` (MIT), the model for a single signed file that opens
  from disk, and a UX reference.
- `iancoleman.io/shamir39`: a single-author, non-standard scheme, considered
  and rejected in favour of SLIP-39; UX reference only.
- [`stake-house/wagyu-key-gen`](https://github.com/stake-house/wagyu-key-gen):
  the target of the dropped validator-key phases.
- The cross-tool suite's oracles:
  [`trezor/python-shamir-mnemonic`](https://github.com/trezor/python-shamir-mnemonic)
  (the reference implementation, MIT), `pjkundert/python-slip39`, and
  `gavincarr/seedkit` (see [scripts/interop/](scripts/interop/README.md)).
