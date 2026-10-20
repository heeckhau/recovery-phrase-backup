# Design

Why Recovery Phrase Backup works the way it does. See [README.md](README.md)
for what it does and how to use it.

**Guiding principle: auditability first.** This handles real recovery
phrases, so every choice (dependencies, architecture, UI) favors the shortest
possible trust chain over convenience or polish: minimal, well-known
dependencies; a plain static page over a framework; code someone can read end
to end over code that's merely tested.

## The problem

A recovery phrase on paper or metal is one copy, and one copy fails in both
directions: burn it and it is gone, find it and it is stolen. The usual
mitigations each move the risk rather than remove it. A BIP39 passphrase
adds a second secret that itself needs a backup. Multisig spreads *signing*,
not the backup, and is complex and chain-specific. Hardware-wallet Shamir
(Trezor) requires that hardware and replaces the phrase rather than backing
it up. Custodial recovery (Ledger Recover) hands the secret to third
parties.

What a backup tool has to get right:
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
  option) and "just use python-slip39" (see [How it compares](#how-it-compares)).

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

**E. SLIP-39 written from the spec.** No suitable Rust crate existed: the
one with a compatible license called itself "untested work-in-progress", and
the maintained one is GPL-3.0. So `rpb-slip39` implements the spec itself,
checked against all 45 official vectors and, in the cross-tool suite,
against three other implementations. Secret material is zeroized; the share
digest and GF(256) lookups are constant-time. Before `split` returns any
share, it recovers the secret from the new mnemonics: every set of shares
that should work must give it back (a random 64 sets per level, for schemes
with more), or no shares are made.

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
  Published test phrases (BIP39 vectors and the default phrases of Hardhat,
  Foundry, Truffle and Ganache), single-byte patterns and phrases with few
  distinct words get a warning and need an explicit "back it up anyway".
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
  shares typed in fully, reproducing the fingerprint. A SLIP-39 passphrase is
  typed again there, from the user's copy: a test that reused the one typed
  at the start would pass with a passphrase recorded wrong. It can be skipped.
- The last step asks what to do with the original phrase: a backup that
  leaves the single original in a drawer hasn't removed the risk it exists
  to remove. It explains the trade-off and leaves the choice to the user.
  It also offers the "how to recover" sheet (see Share cards) and says to
  keep it with a copy of the HTML file, for example on a USB stick: without
  the file, an heir needs someone technical. And it says that a new backup
  of the same phrase doesn't retire this one: the old cards keep working
  until destroyed.

**Recovery.** Shares go in one at a time, with progress towards the
threshold; then the phrase, and a fingerprint check before any address is
shown. Surplus shares are ignored once they are confirmed to belong to the
same backup (the spec and the reference implementation reject them instead).
Two backups of the same phrase share the fingerprint, so the fingerprint
can't tell their cards apart; the first two words can, since they encode the
backup's random identifier (with the extendable flag and the iteration
exponent: exactly 20 bits). A share whose first two words differ from those
already added is refused as soon as it is added, naming both, and the "how to
recover" sheet and the last step of the backup show them.

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
small for a home printer. On a live USB system all of this disappears at
shutdown.

**Share cards.** A card carries the numbered words, the fingerprint, and a
note to whoever keeps it: someone trusted you with this; it is useless alone
but needed, so keep it safe and private, don't copy or photograph it, and
give it back only to the owner or whoever they named. It carries nothing a
finder could plan with: no position ("share 2 of 3"), no threshold, no group,
no passphrase hint, no mention of SLIP-39 or a wallet. (A share's first words
still encode its group and threshold for SLIP-39 software; that is part of
the standard.) Those details go on the secret-free "how to recover" sheet the
owner keeps for their heir. Written for someone who isn't technical, it has
the fingerprint, the threshold, the steps, and lines the owner fills in by
hand: where the HTML file is kept, which wallet app to restore in, who has
the shares, and whether there is a wallet passphrase. It warns against typing
the shares into a hardware wallet (a Trezor accepts them and opens a
different, empty wallet), and ends with recovery without the file for
someone technical. It names no URL, like the cards.

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

**Out of scope**: non-English wordlists (MetaMask and Trezor are English-only
in practice), mobile, a desktop app, and generating validator keys (a
validator mnemonic is a BIP39 phrase, which this tool already backs up).

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
   any modern browser, including on a live USB system.
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
  validator key generation, considered early on and left out: a different
  product with higher stakes, and a validator mnemonic is a BIP39 phrase
  this tool already backs up.
- The cross-tool suite's oracles:
  [`trezor/python-shamir-mnemonic`](https://github.com/trezor/python-shamir-mnemonic)
  (the reference implementation, MIT), `pjkundert/python-slip39`, and
  `gavincarr/seedkit` (see [scripts/interop/](scripts/interop/README.md)).
