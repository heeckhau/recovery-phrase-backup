"""Cross-tool recovery suite (see README.md in this directory).

Run via ./run.sh (inside the pinned nix shell). For each oracle tool it:

  theirs -> ours   generates shares with the oracle, checks the oracle
                   recovers them itself, checks rpb-slip39 recovers them too,
                   and writes them to slip39/tests/interop/<tool>.json, which
                   a plain `cargo test` re-checks without any of these tools;
  ours -> theirs   splits the same secrets with rpb-slip39 and checks the
                   oracle recovers them (live only: needs the oracle).

Plus, both ways: a wrong passphrase gives the *same* wrong bytes in both
tools, a tampered share is rejected by both, and surplus shares are handled
the way each tool documents (we ignore them; the reference rejects them).

Fixtures are regenerated only by running this script, and the result is
reviewed like any other change. Shares are random, so every run rewrites
them; the secrets are derived from the case names and stay fixed.
"""
import argparse
import datetime
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / "scripts" / "interop"
WORK = ROOT / "target" / "interop"
FIXTURES = ROOT / "slip39" / "tests" / "interop"
OURS = ROOT / "target" / "release" / "examples" / "interop"

SEEDKIT_VERSION = "v0.3.3"

SCHEMES = {
    "1of1": (1, [[1, 1]]),
    "2of3": (1, [[2, 3]]),
    "multi": (2, [[2, 3], [3, 5], [1, 1]]),
}


# ---- setup -----------------------------------------------------------------

def run(cmd, **kwargs):
    return subprocess.run(cmd, check=True, **kwargs)


def ensure_venv(name, requirements):
    venv = WORK / f"venv-{name}"
    python = venv / "bin" / "python"
    stamp = venv / ".requirements"
    wanted = (HERE / requirements).read_text()
    if not python.exists() or not stamp.exists() or stamp.read_text() != wanted:
        run([sys.executable, "-m", "venv", "--clear", str(venv)])
        run([str(python), "-m", "pip", "install", "-q", "--no-deps", "-r", str(HERE / requirements)])
        stamp.write_text(wanted)
    return python


def ensure_seedkit():
    binary = WORK / "bin" / "seedkit"
    stamp = WORK / "bin" / ".seedkit-version"
    if not binary.exists() or not stamp.exists() or stamp.read_text() != SEEDKIT_VERSION:
        env = dict(os.environ, GOBIN=str(WORK / "bin"), GOPATH=str(WORK / "gopath"), GOFLAGS="-modcacherw")
        run(["go", "install", f"github.com/gavincarr/seedkit@{SEEDKIT_VERSION}"], env=env)
        stamp.write_text(SEEDKIT_VERSION)
    return binary


def build_ours():
    run(["cargo", "build", "-q", "--release", "-p", "rpb-slip39", "--example", "interop"], cwd=ROOT)


# ---- line-protocol subprocesses --------------------------------------------

class LineProcess:
    """A long-running helper speaking one JSON request/response per line."""

    def __init__(self, cmd):
        self.proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)

    def __call__(self, **req):
        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()
        return json.loads(self.proc.stdout.readline())

    def close(self):
        self.proc.stdin.close()
        self.proc.wait()


class Seedkit:
    def __init__(self, binary, ours):
        self.binary = str(binary)
        self.ours = ours  # only to read each share's group/member index

    def _run(self, args, stdin):
        p = subprocess.run([self.binary, *args], input=stdin, capture_output=True, text=True)
        if p.returncode != 0:
            return {"error": (p.stderr or p.stdout).strip()}
        return {"out": p.stdout.strip()}

    def version(self):
        return SEEDKIT_VERSION

    def phrase_from_entropy(self, entropy_hex):
        return self._run(["eb", entropy_hex], "")

    def entropy_from_phrase(self, phrase):
        return self._run(["be"], phrase + "\n")

    def generate(self, phrase, group_threshold, groups, passphrase):
        args = ["bs", "-t", str(group_threshold)]
        for t, n in groups:
            args += ["-g", f"{t}of{n}"]
        if passphrase:
            args += ["-p", passphrase]
        r = self._run(args, phrase + "\n")
        if "error" in r:
            return r
        # Output is one share per line with nothing marking the groups, so
        # group them by the indices encoded in the shares themselves.
        shares = [line.strip() for line in r["out"].splitlines() if line.strip()]
        grouped = [[] for _ in groups]
        for share in shares:
            info = self.ours(op="inspect", mnemonic=share)
            grouped[info["group_index"]].append((info["member_index"], share))
        return {"mnemonics": [[s for _, s in sorted(g)] for g in grouped]}

    def combine(self, mnemonics, passphrase):
        """Shares -> BIP39 phrase -> entropy hex, via seedkit's own commands."""
        args = ["sb"] + (["-p", passphrase] if passphrase else [])
        r = self._run(args, "\n".join(mnemonics) + "\n")
        if "error" in r:
            return r
        e = self.entropy_from_phrase(r["out"])
        if "error" in e:
            return e
        return {"secret_hex": e["out"], "phrase": r["out"]}


# ---- cases -----------------------------------------------------------------

def secret_for(tool, name, length):
    # The label predates the rename to rpb; changing it would change every fixture.
    digest = hashlib.sha512(f"pkc-slip39 interop {tool} {name}".encode()).digest()
    return digest[:length].hex()


def wrong_passphrase(passphrase):
    return passphrase + "x" if passphrase else "wrong"


def recovery_sets(scheme, mnemonics):
    """Two different minimal sets, and one with surplus shares (or None)."""
    if scheme == "1of1":
        return [[mnemonics[0][0]]], None
    if scheme == "2of3":
        g = mnemonics[0]
        return [[g[0], g[2]], [g[1], g[2]]], list(g)
    g0, g1, g2 = mnemonics
    return [g0[0:2] + g1[0:3], g1[2:5] + [g2[0]]], g0 + g1 + g2


def tamper(mnemonic):
    """Swap two adjacent differing words: still all valid words, but RS1024
    is guaranteed to catch a change of at most three words."""
    words = mnemonic.split()
    for i in range(5, len(words) - 1):
        if words[i] != words[i + 1]:
            words[i], words[i + 1] = words[i + 1], words[i]
            return " ".join(words)
    raise AssertionError("no adjacent differing words to swap")


# Reference implementation: byte level, every parameter it exposes.
SHAMIR_CASES = [
    # name, bytes, scheme, passphrase, iteration_exponent, extendable
    ("128-1of1-e0-ext", 16, "1of1", "", 0, True),
    ("160-2of3-pass-e1-noext", 20, "2of3", "TREZOR", 1, False),
    ("192-multi-e1-ext", 24, "multi", "", 1, True),
    ("224-2of3-e0-noext", 28, "2of3", "", 0, False),
    ("256-multi-pass-e0-ext", 32, "multi", "TREZOR", 0, True),
    ("256-1of1-e2-noext", 32, "1of1", "", 2, False),
    ("512-2of3-pass-e0-ext", 64, "2of3", "TREZOR", 0, True),
    ("128-multi-pass-e0-noext", 16, "multi", "TREZOR", 0, False),
    ("256-2of3-defaults", 32, "2of3", "", None, None),
]

# python-slip39: its BIP39 mode (empty SLIP-39 passphrase, by its design)
# at the lengths it supports: it only accepts 128-, 256- and 512-bit
# secrets, so 15/18/21-word phrases are probed separately: plus its raw
# mode for a 512-bit seed.
SLIP39_CASES = [
    # name, bytes, scheme, iteration_exponent, extendable, mode
    ("12-words-2of3-defaults", 16, "2of3", None, None, "bip39"),
    ("12-words-multi-e0", 16, "multi", 0, None, "bip39"),
    ("24-words-1of1-noext", 32, "1of1", None, False, "bip39"),
    ("24-words-multi", 32, "multi", None, None, "bip39"),
    ("512-raw-2of3-pass", 64, "2of3", 0, None, "raw"),
]
SLIP39_UNSUPPORTED_BYTES = [20, 24, 28]

# seedkit: BIP39 phrase in, with or without a SLIP-39 passphrase; it fixes
# extendable and the iteration exponent itself.
SEEDKIT_CASES = [
    # name, bytes, scheme, passphrase
    ("12-words-2of3", 16, "2of3", ""),
    ("15-words-multi-pass", 20, "multi", "TREZOR"),
    ("18-words-1of1", 24, "1of1", ""),
    ("21-words-2of3-pass", 28, "2of3", "TREZOR"),
    ("24-words-multi", 32, "multi", ""),
]


# ---- checks ----------------------------------------------------------------

class Checker:
    def __init__(self, tool):
        self.tool = tool
        self.failures = []
        self.count = 0

    def expect(self, ok, what):
        self.count += 1
        if not ok:
            self.failures.append(f"{self.tool}: {what}")
            print(f"  FAIL {what}", file=sys.stderr)


def check_case(c, tool, case, ours, combine_theirs, generate_ours_spec):
    """Shared checks for one case. `combine_theirs(mnemonics, passphrase)`
    recovers with the oracle; `generate_ours_spec` is the split request."""
    name, secret, passphrase, mnemonics = case["name"], case["secret_hex"], case["passphrase"], case["mnemonics"]
    wrong = case["wrong_passphrase"]
    sets, surplus = recovery_sets(case["scheme"], mnemonics)

    # theirs -> theirs (the oracle agrees with itself)
    for i, s in enumerate(sets):
        r = combine_theirs(s, passphrase)
        c.expect(r.get("secret_hex") == secret, f"{name}: oracle recovers its own set {i}: {r}")
    tampered_set = [tamper(sets[0][0])] + sets[0][1:]
    c.expect("error" in combine_theirs(tampered_set, passphrase), f"{name}: oracle rejects a tampered share")
    r = combine_theirs(sets[0], wrong)
    wrong_secret = r.get("secret_hex")
    c.expect(wrong_secret is not None and wrong_secret != secret, f"{name}: oracle gives a different secret for a wrong passphrase: {r}")
    surplus_ok = None
    if surplus is not None:
        surplus_ok = combine_theirs(surplus, passphrase).get("secret_hex") == secret

    # theirs -> ours
    for i, s in enumerate(sets):
        r = ours(op="combine", mnemonics=s, passphrase=passphrase)
        c.expect(r.get("secret_hex") == secret, f"{name}: ours recovers oracle set {i}: {r}")
        if "bip39_phrase" in case:
            r = ours(op="combine_phrase", mnemonics=s, passphrase=passphrase)
            c.expect(r.get("phrase") == case["bip39_phrase"], f"{name}: ours recovers the original phrase from oracle set {i}: {r}")
    r = ours(op="combine", mnemonics=sets[0], passphrase=wrong)
    c.expect(r.get("secret_hex") == wrong_secret, f"{name}: wrong passphrase gives the same bytes in ours and the oracle: {r} vs {wrong_secret}")
    c.expect("error" in ours(op="combine", mnemonics=tampered_set, passphrase=passphrase), f"{name}: ours rejects the tampered share")
    if surplus is not None:
        r = ours(op="combine", mnemonics=surplus, passphrase=passphrase)
        c.expect(r.get("secret_hex") == secret, f"{name}: ours ignores surplus shares: {r}")
    info = ours(op="inspect", mnemonic=mnemonics[0][0])

    # ours -> theirs (for phrase cases, split from the words themselves)
    if "bip39_phrase" in case:
        spec = {k: v for k, v in generate_ours_spec.items() if k != "secret_hex"}
        split = ours(op="split_phrase", phrase=case["bip39_phrase"], **spec)
    else:
        split = ours(op="split", **generate_ours_spec)
    if "error" in split:
        c.expect(False, f"{name}: ours splits: {split}")
    else:
        our_sets, _ = recovery_sets(case["scheme"], split["mnemonics"])
        for i, s in enumerate(our_sets):
            r = combine_theirs(s, generate_ours_spec["passphrase"])
            c.expect(r.get("secret_hex") == secret, f"{name}: oracle recovers our set {i}: {r}")
            if "phrase" in r and "bip39_phrase" in case:
                c.expect(r["phrase"] == case["bip39_phrase"], f"{name}: oracle recovers the original phrase from our set {i}")
        theirs_wrong = combine_theirs(our_sets[0], wrong).get("secret_hex")
        ours_wrong = ours(op="combine", mnemonics=our_sets[0], passphrase=wrong).get("secret_hex")
        c.expect(theirs_wrong is not None and theirs_wrong == ours_wrong, f"{name}: wrong passphrase on our shares agrees: {theirs_wrong} vs {ours_wrong}")
        c.expect("error" in combine_theirs([tamper(our_sets[0][0])] + our_sets[0][1:], generate_ours_spec["passphrase"]), f"{name}: oracle rejects our tampered share")

    return {
        "name": name,
        "secret_hex": secret,
        **({"bip39_phrase": case["bip39_phrase"]} if "bip39_phrase" in case else {}),
        "passphrase": passphrase,
        "wrong_passphrase": wrong,
        "wrong_secret_hex": wrong_secret,
        "group_threshold": SCHEMES[case["scheme"]][0],
        "groups": SCHEMES[case["scheme"]][1],
        "extendable": info.get("extendable"),
        "iteration_exponent": info.get("iteration_exponent"),
        "mnemonics": mnemonics,
        "recovery_sets": sets,
        "surplus_set": surplus,
        "surplus_accepted_by_tool": surplus_ok,
        "tampered_set": tampered_set,
    }


def run_shamir(c, ours, oracle):
    cases = []
    for name, length, scheme, passphrase, e, ext in SHAMIR_CASES:
        gt, groups = SCHEMES[scheme]
        secret = secret_for("shamir", name, length)
        req = dict(op="generate", secret_hex=secret, group_threshold=gt, groups=groups, passphrase=passphrase)
        if e is not None:
            req["iteration_exponent"] = e
        if ext is not None:
            req["extendable"] = ext
        gen = oracle(**req)
        if "error" in gen:
            c.expect(False, f"{name}: oracle generates: {gen}")
            continue
        case = dict(name=name, secret_hex=secret, passphrase=passphrase, wrong_passphrase=wrong_passphrase(passphrase), scheme=scheme, mnemonics=gen["mnemonics"])
        spec = dict(secret_hex=secret, group_threshold=gt, groups=groups, passphrase=passphrase, iteration_exponent=e or 0)
        cases.append(check_case(c, "shamir", case, ours, lambda m, p: oracle(op="combine", mnemonics=m, passphrase=p), spec))
    return cases


def run_slip39(c, ours, oracle):
    cases = []
    for name, length, scheme, e, ext, mode in SLIP39_CASES:
        gt, groups = SCHEMES[scheme]
        secret = secret_for("python-slip39", name, length)
        req = dict(group_threshold=gt, groups=groups)
        if e is not None:
            req["iteration_exponent"] = e
        if ext is not None:
            req["extendable"] = ext
        case = dict(name=name, secret_hex=secret, scheme=scheme)
        if mode == "bip39":
            phrase = oracle(op="phrase_from_entropy", entropy_hex=secret)["phrase"]
            gen = oracle(op="generate_bip39", phrase=phrase, **req)
            case.update(bip39_phrase=phrase, passphrase="")

            def combine(m, p):
                # The phrase-level recovery only exists without a SLIP-39
                # passphrase; a wrong one is checked through the raw path.
                return oracle(op="combine_bip39", mnemonics=m) if p == "" else oracle(op="combine_raw", mnemonics=m, passphrase=p)
        else:
            gen = oracle(op="generate_raw", secret_hex=secret, passphrase="TREZOR", **req)
            case.update(passphrase="TREZOR")

            def combine(m, p):
                return oracle(op="combine_raw", mnemonics=m, passphrase=p)
        if "error" in gen:
            c.expect(False, f"{name}: oracle generates: {gen}")
            continue
        case.update(mnemonics=gen["mnemonics"], wrong_passphrase=wrong_passphrase(case["passphrase"]))
        spec = dict(secret_hex=secret, group_threshold=gt, groups=groups, passphrase=case["passphrase"], iteration_exponent=e or 0)
        cases.append(check_case(c, "python-slip39", case, ours, combine, spec))
    return cases


def probe_slip39_unsupported(c, ours, oracle):
    """15/18/21-word phrases: python-slip39 refuses to generate shares for
    them. Assert that (so a change shows up) and record whether it can at
    least recover *our* shares of those lengths."""
    recovers_ours = {}
    for length in SLIP39_UNSUPPORTED_BYTES:
        secret = secret_for("python-slip39", f"unsupported-{length}", length)
        phrase = oracle(op="phrase_from_entropy", entropy_hex=secret).get("phrase")
        gen = oracle(op="generate_bip39", phrase=phrase, group_threshold=1, groups=[[2, 3]])
        c.expect("error" in gen, f"{length * 8}-bit: python-slip39 refuses to generate shares, as documented: {gen}")
        split = ours(op="split", secret_hex=secret, group_threshold=1, groups=[[2, 3]], passphrase="", iteration_exponent=0)
        r = oracle(op="combine_bip39", mnemonics=split["mnemonics"][0][:2])
        recovers_ours[str(length * 8)] = r.get("secret_hex") == secret
    return recovers_ours


def run_seedkit(c, ours, seedkit):
    cases = []
    for name, length, scheme, passphrase in SEEDKIT_CASES:
        gt, groups = SCHEMES[scheme]
        secret = secret_for("seedkit", name, length)
        phrase = seedkit.phrase_from_entropy(secret)["out"]
        gen = seedkit.generate(phrase, gt, groups, passphrase)
        if "error" in gen:
            c.expect(False, f"{name}: seedkit generates: {gen}")
            continue
        case = dict(name=name, secret_hex=secret, bip39_phrase=phrase, passphrase=passphrase, wrong_passphrase=wrong_passphrase(passphrase), scheme=scheme, mnemonics=gen["mnemonics"])
        spec = dict(secret_hex=secret, group_threshold=gt, groups=groups, passphrase=passphrase, iteration_exponent=0)
        cases.append(check_case(c, "seedkit", case, ours, seedkit.combine, spec))
    return cases


def defaults_from(cases, name):
    case = next(x for x in cases if x["name"] == name)
    return {"extendable": case["extendable"], "iteration_exponent": case["iteration_exponent"]}


def write_fixture(tool, version, cases, defaults, notes, extra):
    surplus = {x["surplus_accepted_by_tool"] for x in cases if x["surplus_set"] is not None}
    data = {
        "tool": tool,
        "version": version,
        "generated": datetime.date.today().isoformat(),
        "generator": "scripts/interop/run.py",
        "defaults": defaults,
        "surplus_accepted_by_tool": surplus.pop() if len(surplus) == 1 else sorted(surplus, key=str),
        "notes": notes,
        **extra,
        "cases": cases,
    }
    path = FIXTURES / f"{tool}.json"
    path.write_text(json.dumps(data, indent=1) + "\n")
    print(f"wrote {path.relative_to(ROOT)} ({len(cases)} cases)")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check-only", action="store_true", help="run every check but don't rewrite the fixtures")
    args = parser.parse_args()

    WORK.mkdir(parents=True, exist_ok=True)
    FIXTURES.mkdir(parents=True, exist_ok=True)
    build_ours()
    ours = LineProcess([str(OURS)])
    shamir = LineProcess([str(ensure_venv("shamir", "requirements-shamir.txt")), "-I", str(HERE / "oracle_shamir.py")])
    slip39 = LineProcess([str(ensure_venv("slip39", "requirements-slip39.txt")), "-I", str(HERE / "oracle_slip39.py")])
    seedkit = Seedkit(ensure_seedkit(), ours)

    failures, total = [], 0
    results = []
    for tool, runner, oracle, version in [
        ("python-shamir-mnemonic", run_shamir, shamir, shamir(op="version")["version"]),
        ("python-slip39", run_slip39, slip39, slip39(op="version")["version"]),
        ("seedkit", run_seedkit, seedkit, seedkit.version()),
    ]:
        print(f"{tool} {version}")
        c = Checker(tool)
        cases = runner(c, ours, oracle)
        extra = {}
        if tool == "python-slip39":
            extra = {
                "unsupported_secret_bits": [n * 8 for n in SLIP39_UNSUPPORTED_BYTES],
                "recovers_our_shares_of_unsupported_lengths": probe_slip39_unsupported(c, ours, oracle),
            }
        failures += c.failures
        total += c.count
        results.append((tool, version, cases, extra))

    for p in (ours, shamir, slip39):
        p.close()

    print(f"{total - len(failures)}/{total} checks passed")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        sys.exit(1)

    if not args.check_only:
        notes = {
            "python-shamir-mnemonic": "Reference implementation; byte level. Rejects surplus groups/shares, as the spec says.",
            "python-slip39": "BIP39 mode shares the phrase's entropy with an empty SLIP-39 passphrase (a passphrase there is the BIP39 one, for addresses only); the 512-bit case uses its raw mode.",
            "seedkit": "BIP39 phrase in and out; -p is the SLIP-39 passphrase. Secrets here are recovered as phrase, then entropy, through seedkit's own commands.",
        }
        defaults = {
            "python-shamir-mnemonic": lambda cs: defaults_from(cs, "256-2of3-defaults"),
            "python-slip39": lambda cs: dict(defaults_from(cs, "12-words-2of3-defaults"), slip39_passphrase_in_bip39_mode="always empty"),
            "seedkit": lambda cs: defaults_from(cs, "12-words-2of3"),
        }
        for tool, version, cases, extra in results:
            write_fixture(tool, version, cases, defaults[tool](cases), notes[tool], extra)


if __name__ == "__main__":
    main()
