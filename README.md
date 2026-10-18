# Recovery Phrase Backup

Back up a wallet's recovery phrase (seed phrase) so that no single place can
lose it, or reveal it.

This tool splits an existing BIP39 recovery phrase (12 to 24 words) into
**shares** using [SLIP-39](https://github.com/satoshilabs/slips/blob/master/slip-0039.md),
the open standard hardware wallets use. You choose how many shares to make
and how many are needed: with "2 of 3", any two shares give your phrase back,
and any single share on its own reveals nothing. Recovery returns your
original words, so it works with every wallet; no wallet needs to support
SLIP-39.

It is one HTML file that runs offline in your browser. It is not a wallet: it
never signs anything and never touches a network.

> **Status: pre-release.** The code is complete and tested, but it has not
> been through its independent review yet ([ROADMAP.md](ROADMAP.md)).
> Until v1, don't use it for a phrase that guards real funds.

## How it works

1. **Your phrase.** Type it in a numbered word grid (or paste it). The page
   shows its *fingerprint* (8 hex characters) and its first Ethereum and
   Bitcoin addresses, so you can confirm it is your wallet before anything
   is split. Published test phrases and other weak phrases get a warning.
2. **How to split it.** 2 of 3, 3 of 5, or your own numbers, including
   groups ("2 of 3 places, each needing 2 of 3 people"). Schemes that are
   weaker than they look, such as 3 of 3, get a warning.
3. **How to record the shares.** Write them by hand (the default; you can
   print blank cards to write on) or print them, all on one sheet to cut
   apart or one card per page.
4. **One share at a time.** For each share, record it, then type back three
   of its words to check your copy.
5. **A recovery test.** Type in enough of your written or printed shares; the
   result must reproduce the fingerprint.
6. **Done.** Guidance on what to do with your original phrase, and an
   optional "how to recover" sheet with no secret on it, for your heir.

Before showing you any share, the tool recovers your phrase from the new
shares itself: every set of shares that should work must give it back
(for very large schemes, a random sample of them), or it makes none. Your
own recovery test still matters: it checks the copies you made.

Every share card carries the fingerprint. It shows that shares belong
together, and a correct recovery reproduces it. It is not an address, so a
found card reveals nothing about your funds. Cards also carry no share
number or threshold, and a note asks whoever keeps the card to keep it safe
and private and give it back only to you or whoever you named.

**This is a backup of a BIP39 phrase.** It is not Trezor's
SLIP-39-as-wallet-seed scheme: restoring these shares directly on a Trezor
would open a *different* wallet. Recover the phrase here (or with another
SLIP-39 tool), then restore the phrase in your wallet.

## Using it

**The release file**: download `recovery-phrase-backup.html` and
`SHA256SUMS` from a
[release](https://github.com/heeckhau/recovery-phrase-backup/releases),
then disconnect from the network and open the file in a browser. (The
[website](https://heeckhau.github.io/recovery-phrase-backup/#download)
offers the latest build of `main` instead, with its SHA-256.) The page
shows its version at the bottom. The
recommended setup is a live USB system such as Tails, without a network
connection:
offline alone doesn't stop malware or a malicious extension from recording
the phrase and sending it once the computer is back online. If you can't,
at least disconnect and use a fresh browser profile without extensions.

**On Tails**: copy the file to the Tails computer (a USB stick works),
start Tails with networking off ("Offline Mode" in the Welcome Screen's
additional settings), and open the file in Tor Browser. Set Tor Browser's
security level to Standard first (the shield icon): Safer and Safest block
WebAssembly, which the page needs, and the page says so if it can't load.
For this one file, offline, Standard is fine. Tails before 6.14.1 only lets
Tor Browser open files in its `Tor Browser` folder, so put the file there.

**Check the download.** Each release carries `SHA256SUMS` and a GitHub
attestation: a record, signed by GitHub, that this repository's CI built
the file from the release's tag.

```sh
shasum -a 256 -c SHA256SUMS
gh attestation verify recovery-phrase-backup.html --repo heeckhau/recovery-phrase-backup
```

**Check that the release matches the source.** The attestation trusts
GitHub; this doesn't. The release file is built reproducibly in a pinned
Linux x86-64 container, so anyone with Docker gets the same bytes:

```sh
git checkout v0.1.0             # the release's tag
scripts/build-reproducible.sh   # writes dist/recovery-phrase-backup.html and prints its SHA-256
```

Compare that hash with the one in `SHA256SUMS`. (Builds are byte-identical on one
platform but differ across platforms, which is why the reference build runs
in a container. On Apple Silicon it runs under emulation and takes a couple
of minutes.)

**The command-line tool** does the same splitting and recovery for scripting
or a Raspberry Pi, and can also convert a raw private key between hex and
WIF. It asks for secrets with hidden input rather than taking them as
arguments.

```sh
cargo install --path cli        # installs `rpb`
rpb slip39 split --group-threshold 1 --group 2:3    # prompts for the phrase
rpb slip39 combine                                   # prompts for shares, one per line
rpb slip39 inspect                                   # which group/share a single share is
rpb convert                                          # raw key: hex <-> WIF, addresses
```

## Recovering without this tool

SLIP-39 is an open standard, so nobody's backup depends on this project
still existing:

1. Recover the secret with any SLIP-39 implementation, for example Trezor's
   reference tool: `pip install "shamir-mnemonic[cli]"`, then
   `shamir recover`. It prints the master secret as hex.
2. That hex is your phrase's *entropy*. Any BIP39 tool turns entropy back
   into words, for example the entropy input of Ian Coleman's offline BIP39
   tool (a copy is in [`reference/`](reference/bip39-standalone.html)).

The cross-tool suite ([scripts/interop/](scripts/interop/README.md)) checks
on every change that shares made here recover in python-shamir-mnemonic,
python-slip39 and seedkit, and that theirs recover here.

## What to know before relying on it

- **Use it on a clean system, offline.** The page sends nothing, but other
  software on the computer could, even later once it is back online. The
  machine is trusted for the session; nothing protects a phrase typed into a
  compromised computer, which is why a live USB system such as Tails is the
  recommendation.
- **Printing is the one moment everything is in one place.** Until the cards
  are cut apart and stored separately, a printout of all shares is your whole
  phrase. A home printer on your own network is fine; avoid office, shared
  and cloud printers, and never "Save as PDF". Writing by hand avoids this
  entirely.
- **A SLIP-39 passphrase is never checked.** A wrong one silently recovers a
  different phrase. That is why it is off by default and the fingerprint
  exists: a recovery that doesn't reproduce the fingerprint is wrong. If you
  set one, the recovery test asks you to type it again from your copy.
- **A new backup doesn't retire the old one.** Backing up the same phrase
  again makes new shares with the same fingerprint, which can't be mixed
  with the old ones, and the old shares still recover the phrase until
  they're destroyed. All shares of one backup start with the same two words
  (shown at the end and on the "how to recover" sheet), which tells them
  apart; a share from another backup is refused as soon as it is added.
- **Your wallet's own BIP39 passphrase (the "25th word") is not part of the
  backup.** If your wallet uses one, back it up separately.
- **Reviewed, not audited.** Before v1 the code is reviewed by several other
  LLMs, and what they find is fixed. That is a second pair of eyes, not a
  professional audit, and this project doesn't claim one.

## Building and testing

The Rust toolchain is pinned in `rust-toolchain.toml`; `rustup` installs it on
first use.

| What | How |
| --- | --- |
| Rust tests and lints | `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` |
| Release file, demo and website (this machine) | `python3 scripts/build.py` writes `dist/recovery-phrase-backup.html`, `dist/demo.html` and `dist/site/` |
| Release file (reference) | `scripts/build-reproducible.sh` (Docker); `--verify-reproducible` builds twice and compares |
| Browser tests | build the release file, then `cd tests/e2e && npm ci && npx playwright install chromium firefox && npx playwright test` |
| Cross-tool suite | `scripts/interop/run.sh --check-only` (nix) |
| Development page | `cd wasm && wasm-pack build --target web --out-dir ../web/pkg`, then serve `web/` with `python3 -m http.server` and open `index.html?test` |

CI (`.github/workflows/ci.yml`, actions pinned by commit) runs all of these
on every push, plus `cargo audit` and a check that no em dash appears in the
repository. On `main` it then deploys the tested `dist/site/` (landing
page, demo and release file) to GitHub Pages.

**Releasing.** The version lives in `Cargo.toml` (`[workspace.package]`),
and the build shows it on the page and the website.
1. Set the version (say `0.2.0`) in `Cargo.toml`, run `cargo check` to
   update `Cargo.lock`, and commit.
2. Tag and push: `git tag -a v0.2.0 -m v0.2.0 && git push origin v0.2.0`.
   CI runs every check on the tag, then its `release` job checks that the
   tag matches `Cargo.toml` and drafts a GitHub release (a pre-release
   before 1.0) with the tested file, `SHA256SUMS` and an attestation.
3. Rebuild with `scripts/build-reproducible.sh`, check the hash matches the
   draft's `SHA256SUMS`, and publish the draft.
4. Set `main` to the next version with a `-dev` suffix (`0.3.0-dev`), so
   builds of `main` don't carry the release's version.

**Layout.**
- `slip39/`: `rpb-slip39`, the SLIP-39 implementation (GF(256) Shamir
  sharing, the Feistel encryption, RS1024, the share codec), written from
  the spec and checked against the official 45 test vectors. It has no
  dependency on the rest of the project.
- `core/`: `rpb-core`, BIP39 phrases (fingerprint, addresses, weak-phrase
  check) and raw-key encoding (hex, WIF, Bitcoin and Ethereum addresses).
- `wasm/`: thin bindings for the browser.
- `cli/`: the `rpb` command.
- `web/`: the page, plain HTML and JavaScript with no framework.
- `scripts/`: the release build and the cross-tool suite.
- `tests/e2e/`: browser tests that drive the built release file, in Chromium
  and in Firefox (which Tor Browser, on Tails, is built on).
- `reference/`: the legacy single-file tools this project started from, kept
  unmodified for reference.

## License

MIT OR Apache-2.0, at your option ([LICENSE-MIT](LICENSE-MIT),
[LICENSE-APACHE](LICENSE-APACHE)). Vendored data keeps its own terms: the SLIP-39 wordlist comes from
`satoshilabs/slips` (CC-BY-SA-4.0) and the official test vectors from
`trezor/python-shamir-mnemonic` (MIT); the files in `reference/` keep their
original licenses.
