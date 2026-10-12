"""Adapter for trezor/python-shamir-mnemonic (the SLIP-39 reference
implementation). Run inside its own venv by run.py; one JSON request per
line on stdin, one JSON response per line on stdout.

Omitted optional fields fall through to the library's own defaults, so the
harness can record what those defaults are.
"""
import json
import sys

import shamir_mnemonic


def handle(req):
    op = req["op"]
    if op == "version":
        from importlib.metadata import version

        return {"version": version("shamir-mnemonic")}
    if op == "generate":
        kwargs = {}
        if "iteration_exponent" in req:
            kwargs["iteration_exponent"] = req["iteration_exponent"]
        if "extendable" in req:
            kwargs["extendable"] = req["extendable"]
        groups = shamir_mnemonic.generate_mnemonics(
            req["group_threshold"],
            [tuple(g) for g in req["groups"]],
            bytes.fromhex(req["secret_hex"]),
            passphrase=req["passphrase"].encode(),
            **kwargs,
        )
        return {"mnemonics": groups}
    if op == "combine":
        secret = shamir_mnemonic.combine_mnemonics(req["mnemonics"], passphrase=req["passphrase"].encode())
        return {"secret_hex": secret.hex()}
    raise ValueError(f"unknown op {op}")


for line in sys.stdin:
    if not line.strip():
        continue
    try:
        response = handle(json.loads(line))
    except Exception as e:  # any oracle failure is reported, not fatal
        response = {"error": f"{type(e).__name__}: {e}"}
    print(json.dumps(response), flush=True)
