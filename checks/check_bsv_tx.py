#!/usr/bin/env python3
"""
SOLBEAM — BSV transaction codec + SIGHASH_FORKID checks against real transactions.

Recomputes the signature hash from first principles and verifies the *network's
own* signatures against it. If the preimage were wrong by a single byte this
fails, so it pins the highest-risk component of the peg to real chain data.

Usage:  python3 check_bsv_tx.py
"""

import json
import sys
import urllib.request

from bsvlib import (SIGHASH_FORKID, der_decode, ecdsa_verify, hash160, parse_pubkey,
                    parse_pushes, parse_tx, p2pkh_hash, serialise_tx, sha256d,
                    sighash_forkid, txid_of)

API = {
    "main": "https://api.whatsonchain.com/v1/bsv/main",
    "test": "https://api.whatsonchain.com/v1/bsv/test",
}

FIXTURES = [
    ("test", "715253f39e85e094d7316697064da42687741b7cf7071d12ceffbacabd9945ab"),
    ("test", "dacbfb56f5f95182f48bbd5750b11d10411f0dcafcae17a480e8a85169329920"),
    ("test", "17976aa3ffa8e778b43dae818978d5a4f7b3ebff56a5ae4ba4e4fb95e3c48c80"),
]


def get_raw_tx(net: str, txid: str) -> bytes:
    """Raw tx: the endpoint returns plain hex, sometimes JSON-quoted."""
    with urllib.request.urlopen(f"{API[net]}/tx/{txid}/hex", timeout=30) as r:
        body = r.read().decode().strip()
    if body.startswith('"'):
        body = json.loads(body)
    return bytes.fromhex(body)


def main() -> int:
    checks, failures = 0, []

    def check(name, ok, detail=""):
        nonlocal checks
        checks += 1
        print(f"  {'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f"  {detail}"))
        if not ok:
            failures.append(name)

    print("SOLBEAM / BSV transaction codec + SIGHASH_FORKID checks\n")
    verified = 0

    for net, txid in FIXTURES:
        print(f"[{net}] {txid[:24]}…")
        try:
            raw = get_raw_tx(net, txid)
        except Exception as e:  # noqa: BLE001
            print(f"      unavailable ({e}), skipping\n")
            continue
        tx = parse_tx(raw)
        check("re-serialises byte-for-byte", serialise_tx(tx) == raw)
        check("txid = double-SHA256 of the raw transaction", txid_of(raw) == txid)

        for idx, vin in enumerate(tx["vin"]):
            parent = parse_tx(get_raw_tx(net, vin["txid"][::-1].hex()))
            prev = parent["vout"][vin["vout"]]
            expected = p2pkh_hash(prev["script"])
            if expected is None:
                continue
            pushes = parse_pushes(vin["script"])
            if len(pushes) != 2:
                continue
            sig_with_type, pubkey_bytes = pushes
            sighash_type = sig_with_type[-1]
            r, s = der_decode(sig_with_type[:-1])
            pubkey = parse_pubkey(pubkey_bytes)

            check(f"input {idx}: pubkey hashes to the prevout P2PKH address",
                  hash160(pubkey_bytes) == expected)
            check(f"input {idx}: signature carries SIGHASH_FORKID",
                  bool(sighash_type & SIGHASH_FORKID))
            digest = sighash_forkid(tx, idx, prev["value"], prev["script"], sighash_type)
            check(f"input {idx}: real network signature verifies against our digest",
                  ecdsa_verify(pubkey, int.from_bytes(digest, "big"), r, s))
            check(f"input {idx}: a tampered digest fails to verify",
                  not ecdsa_verify(pubkey, int.from_bytes(digest, "big") ^ 1, r, s))
            verified += 1
        print()

    check("at least one real input fully verified", verified > 0, f"verified {verified}")
    print(f"{checks - len(failures)}/{checks} checks passed")
    if failures:
        print("failed: " + ", ".join(failures))
        return 1
    print("all good — the Go/Rust implementations must reproduce exactly these digests")
    return 0


if __name__ == "__main__":
    sys.exit(main())
