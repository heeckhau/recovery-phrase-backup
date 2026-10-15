# Cross-tool recovery suite

The official SLIP-39 vectors prove that `combine` decodes what the reference
implementation encoded. They say nothing about the other direction (whether
*our* shares recover elsewhere) and nothing at the BIP39-phrase level, which
is where the product lives. This suite closes both gaps. It is the concrete
form of the "no lock-in" promise: a backup made here must be recoverable with
tools that have nothing to do with this project, and the other way round.

## Running it

```sh
scripts/interop/run.sh               # run every check and regenerate the fixtures
scripts/interop/run.sh --check-only  # run every check, leave the fixtures alone
```

It enters a nix shell pinned to one nixpkgs revision (`shell.nix`) for Python
and Go, so it needs nix, and network access on the first run to install the
pinned oracles. Current result: 266/266 checks across 19 cases. CI runs
`--check-only` on every push.

## Oracles

Each is pinned, and its version is recorded in every fixture.

- `trezor/python-shamir-mnemonic` 0.3.0: the reference implementation (MIT),
  through its Python API. Byte level: every length from 128 to 512 bits,
  both extendable values, iteration exponents 0 to 2, with and without a
  passphrase.
- `pjkundert/python-slip39` 14.1.0: its BIP39 mode (12- and 24-word phrases)
  and its raw mode (a 512-bit seed). It is GPL-3.0, so it runs only as an
  external program from the harness, in its own venv; nothing from it is
  linked, vendored or shipped. The fixtures are its output data.
- `gavincarr/seedkit` v0.3.3: a Go CLI, BIP39 phrase to shares to phrase, at
  all five phrase lengths, with and without a SLIP-39 passphrase. Its GitHub
  repository is no longer reachable; the Go module proxy still serves v0.3.3,
  verified against Go's checksum database on install.
- `iancoleman/slip39` is not included: it is browser-only and adds no
  parameter the three above don't cover.

## What is checked

**Directions.** Theirs to ours: every oracle's shares recover in
`rpb-slip39` to the same bytes, both live in the harness and from the
committed fixtures in a plain `cargo test` (`slip39/tests/interop.rs`). Ours
to theirs: shares from `rpb-slip39` recover in every oracle; this half needs
the oracles, so it runs live only. For the oracles that work with BIP39
phrases (python-slip39's BIP39 mode and seedkit), both sides go through the
words: their shares must recover to the exact original phrase through
`rpb_core::Phrase`, and our shares are split from the phrase itself.

**Per case**, in both directions where they apply:
- two different minimal recovery sets;
- a set with surplus groups and members;
- a wrong passphrase gives the *same* wrong bytes in both tools, which pins
  our Feistel/PBKDF2 layer against independent implementations;
- a tampered share (two adjacent words swapped) is rejected by both;
- `inspect` reads back the tool's extendable flag and iteration exponent.

Planting a bug in our salt derivation made 16 checks fail in each half.

## What it found

These are recorded in the fixtures.

- **Defaults.** All three tools default to `extendable = true` and iteration
  exponent 1. Ours used 0, and now defaults to 1 as well.
- **Passphrase.** python-slip39's BIP39 mode always uses an empty SLIP-39
  passphrase (a passphrase there is the BIP39 one, used for addresses only);
  seedkit's `-p` is the SLIP-39 passphrase.
- **Lengths.** python-slip39 refuses to create shares for 160-, 192- and
  224-bit secrets (15-, 18- and 21-word phrases), but recovers ours at those
  lengths.
- **Surplus shares.** The reference implementation and seedkit reject more
  groups or members than the thresholds need, as the spec says ("GM MUST be
  equal to GT"); python-slip39 and `rpb-slip39` ignore the surplus. Ours is
  deliberate (commit `228e746`): the surplus is first checked to belong to
  the same backup. Each fixture records its tool's behaviour and the harness
  checks it, so a change on either side shows up.

## How it works

- `run.py` drives everything: `oracle_shamir.py` and `oracle_slip39.py` (one
  venv each, since python-slip39 ships a fork under the reference's module
  name), the seedkit binary, and `slip39/examples/interop.rs`, a small example
  binary that exposes `split`, `combine` and `inspect` over JSON lines.
  Examples are not part of the library.
- Fixtures live in `slip39/tests/interop/<tool>.json`. Secrets are derived
  from the case names and stay fixed; shares are random, so regenerating
  rewrites them. Regenerating is a reviewed change, never automatic.
- Python dependencies are pinned as full transitive freezes
  (`requirements-*.txt`, installed with `--no-deps`): versions rather than
  hashes, since wheel hashes differ per platform.
