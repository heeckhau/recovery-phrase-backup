"""Adapter for pjkundert/python-slip39 (GPL-3.0). Run inside its own venv by
run.py: as an external program only; nothing from it is linked, vendored or
shipped. One JSON request per line on stdin, one JSON response per line on
stdout.

Its BIP39 mode (`using_bip39`) shares the phrase's entropy and always uses
an empty SLIP-39 passphrase: a passphrase given in that mode is the BIP39
"25th word", used only to derive addresses.
"""
import json
import logging
import sys

import slip39
from slip39.recovery import produce_bip39, recover, recover_bip39

logging.disable(logging.CRITICAL)  # it logs every mnemonic at INFO


def groups_dict(groups):
    return {f"g{i}": tuple(g) for i, g in enumerate(groups)}


def generate(req, master_secret, using_bip39, passphrase):
    kwargs = {}
    if "iteration_exponent" in req:
        kwargs["iteration_exponent"] = req["iteration_exponent"]
    if "extendable" in req:
        kwargs["extendable"] = req["extendable"]
    details = slip39.create(
        "interop",
        group_threshold=req["group_threshold"],
        groups=groups_dict(req["groups"]),
        master_secret=master_secret,
        passphrase=passphrase,
        using_bip39=using_bip39,
        **kwargs,
    )
    return {"mnemonics": [mnems for (_, mnems) in details.groups.values()]}


def handle(req):
    op = req["op"]
    if op == "version":
        from importlib.metadata import version

        return {"version": version("slip39")}
    if op == "phrase_from_entropy":
        return {"phrase": produce_bip39(entropy=bytes.fromhex(req["entropy_hex"]))}
    if op == "generate_bip39":
        return generate(req, req["phrase"], True, None)
    if op == "generate_raw":
        return generate(req, bytes.fromhex(req["secret_hex"]), False, req["passphrase"])
    if op == "combine_bip39":
        entropy = recover(req["mnemonics"], using_bip39=True, as_entropy=True)
        return {"secret_hex": entropy.hex(), "phrase": produce_bip39(entropy=entropy)}
    if op == "combine_raw":
        return {"secret_hex": recover(req["mnemonics"], passphrase=req["passphrase"]).hex()}
    if op == "entropy_from_phrase":
        return {"entropy_hex": recover_bip39(req["phrase"], as_entropy=True).hex()}
    raise ValueError(f"unknown op {op}")


for line in sys.stdin:
    if not line.strip():
        continue
    try:
        response = handle(json.loads(line))
    except Exception as e:  # any oracle failure is reported, not fatal
        response = {"error": f"{type(e).__name__}: {e}"}
    print(json.dumps(response), flush=True)
