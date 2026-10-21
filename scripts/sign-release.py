#!/usr/bin/env python3
"""Checks a draft release, signs it as heeckhau.eth, and publishes it.

    scripts/sign-release.py --notes NOTES.md               # sign in a wallet, paste the signature
    scripts/sign-release.py --notes NOTES.md --ledger      # sign with a Ledger, through cast
    scripts/sign-release.py --notes NOTES.md --trezor      # sign with a Trezor, through cast
    scripts/sign-release.py --dry-run ...                  # every check, nothing uploaded

Run it after CI has drafted the release for the version in Cargo.toml. It
stops at the first thing that doesn't match:

  1. The tag v<version> exists, here and on origin, on the same commit.
  2. The release for that tag (a draft, or already published) has the
     release file and SHA256SUMS, and SHA256SUMS matches the file.
  3. GitHub's attestation for the file verifies.
  4. scripts/build-reproducible.sh, run in a clean checkout of the tag,
     gives the same bytes.
  5. heeckhau.eth still resolves to the pinned address below.
  6. The one-line message (version, file hash, commit) is signed with
     personal_sign (EIP-191), and the signature verifies against the pinned
     address.
  7. SIGNATURE.txt goes up next to the file, the notes (from --notes, or the
     release's own) get a "Signed by heeckhau.eth" section, and after a last
     confirmation a draft is published. A release that is already published
     gets the signature and the section, and stays as it is otherwise.

Needs gh (logged in), Docker, and Foundry's cast; without cast on the PATH it
re-runs itself under `nix shell nixpkgs#foundry`.
"""
import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import build  # noqa: E402

ENS_NAME = "heeckhau.eth"
# The signing key's address. ENS can be pointed elsewhere, so the signature
# is checked against this address, and the name only has to agree with it.
ADDRESS = "0x9b18e9e9aa3dD35100b385b7035C0B1E44AfcA14"
DEFAULT_RPC = "https://ethereum-rpc.publicnode.com"
FILE = "recovery-phrase-backup.html"
SECTION = "### Signed by heeckhau.eth"


def fail(msg):
    sys.exit(f"sign-release: {msg}")


def step(msg):
    print(f"\n==> {msg}", flush=True)


def run(*cmd, cwd=ROOT, capture=True):
    result = subprocess.run(cmd, cwd=cwd, capture_output=capture, text=True)
    if result.returncode != 0:
        detail = (result.stderr or result.stdout or "").strip() if capture else ""
        fail(f"`{' '.join(cmd)}` failed" + (f":\n{detail}" if detail else ""))
    return result.stdout.strip() if capture else ""


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def message(tag, file_hash, commit):
    # One line, so pasting it into a web form can't change its line endings.
    return f"Recovery Phrase Backup {tag}: {FILE} has SHA-256 {file_hash}, built from commit {commit}."


def signature_file(tag, msg, sig, link):
    lines = [
        f"Recovery Phrase Backup {tag}, signed by {ENS_NAME} ({ADDRESS}).",
        "",
        "Message (one line, signed with personal_sign, EIP-191):",
        msg,
        "",
        "Signature:",
        sig,
        "",
        "Check it with Foundry's cast:",
        f'cast wallet verify --address {ADDRESS} "{msg}" {sig}',
        "",
        "or paste the address, message and signature into https://etherscan.io/verifiedSignatures.",
    ]
    if link:
        lines += ["", f"Etherscan lists it: {link}"]
    return "\n".join(lines) + "\n"


def notes_section(msg, sig, link):
    text = f"""{SECTION}

Independently of GitHub, `{ENS_NAME}` (`{ADDRESS}`) signed this message, which names the release file's SHA-256 and the commit it is built from (also in `SIGNATURE.txt`):

```
{msg}
```

Signature: `{sig}`

To check it, paste the address, the message and the signature into [Etherscan's signature checker](https://etherscan.io/verifiedSignatures), or run:

```sh
cast wallet verify --address {ADDRESS} "{msg}" {sig}
```
"""
    if link:
        text += f"\nEtherscan lists it: {link}\n"
    return text


def with_section(notes, section):
    # Replaces an earlier signature section (up to the next ### heading), so
    # running the script again doesn't add a second one. Otherwise the section
    # goes before "### Feedback welcome!" if there is one, or at the end.
    notes = notes.replace("\r\n", "\n").rstrip() + "\n"
    old = re.search(rf"^{re.escape(SECTION)}\n.*?(?=^### |\Z)", notes, re.M | re.S)
    if old:
        return notes[: old.start()] + section.rstrip() + "\n\n" + notes[old.end():].lstrip()
    feedback = re.search(r"^### Feedback", notes, re.M)
    if feedback:
        return notes[: feedback.start()] + section.rstrip() + "\n\n" + notes[feedback.start():]
    return notes + "\n" + section


def ensure_cast():
    if shutil.which("cast"):
        return
    if os.environ.get("SIGN_RELEASE_IN_NIX") or not shutil.which("nix"):
        fail("needs Foundry's cast (https://getfoundry.sh), or nix to fetch it")
    print("cast not found: re-running under `nix shell nixpkgs#foundry`", flush=True)
    os.environ["SIGN_RELEASE_IN_NIX"] = "1"
    os.execvp("nix", ["nix", "shell", "nixpkgs#foundry", "-c", sys.executable, *sys.argv])


def check_tag(tag):
    step(f"The tag {tag}, here and on origin")
    commit = run("git", "rev-parse", "--verify", "--quiet", f"{tag}^{{commit}}")
    remote = run("git", "ls-remote", "origin", f"refs/tags/{tag}^{{}}", f"refs/tags/{tag}")
    # An annotated tag lists its commit as the ^{} line; a lightweight one is
    # the commit itself.
    lines = remote.splitlines()
    peeled = [line for line in lines if line.endswith("^{}")]
    remote_commits = {line.split()[0] for line in (peeled or lines)}
    if not remote_commits:
        fail(f"{tag} isn't on origin")
    if remote_commits != {commit}:
        fail(f"{tag} is {commit} here but {', '.join(remote_commits)} on origin")
    print(f"{tag} is commit {commit}")
    return commit


def check_release(tag, work):
    step("The release and its files")
    info = run("gh", "release", "view", tag, "--json", "isDraft,assets,url",
               "--jq", '"\\(.isDraft)\\t\\(.url)\\t\\([.assets[].name] | join(","))"')
    is_draft, url, assets = info.split("\t")
    if is_draft != "true":
        print("already published: the signature will be added to it")
    assets = set(filter(None, assets.split(",")))
    if not {FILE, "SHA256SUMS"} <= assets:
        fail(f"the release has {sorted(assets)}, not {FILE} and SHA256SUMS")
    released = work / "release"
    released.mkdir()
    run("gh", "release", "download", tag, "--dir", str(released), "--pattern", FILE, "--pattern", "SHA256SUMS")
    file_hash = sha256(released / FILE)
    sums = (released / "SHA256SUMS").read_text().split()
    if sums != [file_hash, FILE]:
        fail(f"SHA256SUMS says {' '.join(sums)}, but the release's {FILE} has SHA-256 {file_hash}")
    print(f"{url}\n{FILE}: SHA-256 {file_hash}, as SHA256SUMS says")
    return released / FILE, file_hash, url, is_draft == "true"


def check_attestation(path):
    step("GitHub's attestation for the file")
    repo = run("gh", "repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner")
    run("gh", "attestation", "verify", str(path), "--repo", repo)
    print(f"verified: {repo}'s CI built it")


def check_reproducible(tag, file_hash, work):
    step(f"Rebuilding {tag} in Docker from a clean checkout (a few minutes)")
    checkout = work / "checkout"
    run("git", "worktree", "add", "--detach", str(checkout), tag)
    try:
        run(str(checkout / "scripts" / "build-reproducible.sh"), cwd=checkout, capture=False)
        rebuilt = sha256(checkout / "dist" / FILE)
    finally:
        subprocess.run(["git", "worktree", "remove", "--force", str(checkout)], cwd=ROOT)
    if rebuilt != file_hash:
        fail(f"the rebuilt file has SHA-256 {rebuilt}, the release's {file_hash}")
    print(f"same bytes: SHA-256 {rebuilt}")


def check_ens(rpc):
    step(f"{ENS_NAME} resolves to {ADDRESS}")
    resolved = run("cast", "resolve-name", ENS_NAME, "--rpc-url", rpc)
    if resolved.lower() != ADDRESS.lower():
        fail(f"{ENS_NAME} resolves to {resolved}, not the pinned {ADDRESS}")
    print("it does")


def verify(msg, sig):
    result = subprocess.run(["cast", "wallet", "verify", "--address", ADDRESS, msg, sig],
                            capture_output=True, text=True)
    return result.returncode == 0


def sign(msg, args):
    step(f"Signing as {ENS_NAME}")
    print(f"The message, one line:\n\n{msg}\n")
    if args.ledger or args.trezor:
        device = "--ledger" if args.ledger else "--trezor"
        print(f"Check that your {device[2:].capitalize()} shows exactly this message, then approve it.", flush=True)
        cmd = ["cast", "wallet", "sign", device, *args.cast_args, msg]
        sig = run(*cmd).splitlines()[-1].strip()
        link = ""
    else:
        print(f"Sign it as {ADDRESS} with personal_sign, for example at\n"
              "https://etherscan.io/verifiedSignatures (\"Sign Message\"), which also publishes it.\n"
              "Copy the message exactly, and check that your wallet shows exactly it.\n")
        sig = input("Signature (0x...): ").strip()
        link = input("Etherscan link to the signature, if you published it (or Enter): ").strip()
    if not re.fullmatch(r"0x[0-9a-fA-F]{130}", sig):
        fail(f"{sig!r} isn't a 65-byte signature")
    if not verify(msg, sig):
        fail(f"the signature doesn't verify for {ADDRESS}: wrong account, or the message changed")
    print(f"verified: {ADDRESS} signed it")
    return sig, link


def confirm(question):
    return input(f"{question} [y/N] ").strip().lower() in ("y", "yes")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--notes", type=Path, help="release notes to use (default: the release's own)")
    device = parser.add_mutually_exclusive_group()
    device.add_argument("--ledger", action="store_true", help="sign with a Ledger through cast")
    device.add_argument("--trezor", action="store_true", help="sign with a Trezor through cast")
    parser.add_argument("--rpc-url", default=os.environ.get("ETH_RPC_URL", DEFAULT_RPC),
                        help=f"Ethereum RPC for the ENS lookup (default: $ETH_RPC_URL or {DEFAULT_RPC})")
    parser.add_argument("--dry-run", action="store_true", help="check and sign, but upload and publish nothing")
    parser.add_argument("cast_args", nargs="*",
                        help="after --, extra arguments for `cast wallet sign`, such as --mnemonic-index 1")
    args = parser.parse_args()
    if args.cast_args and not (args.ledger or args.trezor):
        parser.error("extra cast arguments only apply with --ledger or --trezor")
    if args.notes and not args.notes.is_file():
        parser.error(f"no file {args.notes}")

    ensure_cast()
    tag = "v" + build.workspace_version()
    with tempfile.TemporaryDirectory(prefix="sign-release-") as tmp:
        work = Path(tmp)
        commit = check_tag(tag)
        path, file_hash, url, is_draft = check_release(tag, work)
        check_attestation(path)
        check_reproducible(tag, file_hash, work)
        check_ens(args.rpc_url)
        msg = message(tag, file_hash, commit)
        sig, link = sign(msg, args)

        step("SIGNATURE.txt and the release notes")
        sig_path = work / "SIGNATURE.txt"
        sig_path.write_text(signature_file(tag, msg, sig, link))
        notes = args.notes.read_text() if args.notes else run("gh", "release", "view", tag, "--json", "body", "--jq", ".body")
        notes_path = work / "notes.md"
        notes_path.write_text(with_section(notes, notes_section(msg, sig, link)))
        if args.dry_run:
            kept = Path(tempfile.mkdtemp(prefix="sign-release-dry-run-"))
            shutil.copy(sig_path, kept)
            shutil.copy(notes_path, kept)
            print(f"dry run: nothing uploaded; SIGNATURE.txt and notes.md are in {kept}")
            return
        run("gh", "release", "upload", tag, str(sig_path), "--clobber")
        run("gh", "release", "edit", tag, "--notes-file", str(notes_path))
        print(f"uploaded SIGNATURE.txt and the notes: {url}")
        if not is_draft:
            return

        step("Publishing")
        if not confirm(f"Everything checks out. Publish {tag} now?"):
            print(f"Not published. The draft is ready: {url}")
            return
        run("gh", "release", "edit", tag, "--draft=false")
        print(f"published: {run('gh', 'release', 'view', tag, '--json', 'url', '--jq', '.url')}")


if __name__ == "__main__":
    main()
