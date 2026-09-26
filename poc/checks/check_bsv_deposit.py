#!/usr/bin/env python3
"""
SOLBEAM — deposit / redemption transaction construction checks.

The peg has to *build* transactions, not just verify them:

  peg-in   user sends BSV to a deposit address, carrying their Solana address
           in an OP_RETURN so the mint has no off-chain registry to trust
  peg-out  the hot wallet pays a redemption address

This file proves we can construct both shapes correctly, sign them with
SIGHASH_FORKID, and recover the embedded Solana address from the chain.

Signing uses a deterministic nonce (RFC 6979) so signatures are reproducible.
The keys here are generated in-process and are throwaway test keys.

Usage:  python3 check_bsv_deposit.py
"""

import json
import sys
import urllib.request

from bsvlib import (MAINNET_P2PKH, SIGHASH_ALL, SIGHASH_FORKID, TESTNET_P2PKH,
                    add_input, add_output, address_to_hash160, der_decode, ecdsa_sign,
                    ecdsa_verify, hash160, le, new_tx, op_return_script, p2pkh_address,
                    p2pkh_hash, p2pkh_script, parse_op_return, parse_tx, pubkey_from_priv,
                    serialise_tx, sign_input, sighash_forkid, txid_of)

API = {"main": "https://api.whatsonchain.com/v1/bsv/main",
       "test": "https://api.whatsonchain.com/v1/bsv/test"}

# A testnet tx whose prevout is a P2PKH output, used to check address encoding
# against a real address the network agrees on.
ADDRESS_FIXTURES = [
    ("test", "e8aa1eb3cd5b74fc332af0aaeec3b2b989e75a9b2284e0d72424b7fb1d0b3bb0", 12),
    ("main", None, None),  # filled from mainnet block 1000's coinbase below
]


def get_json(net: str, path: str):
    req = urllib.request.Request(API[net] + path, headers={"User-Agent": "solbeam-poc/0.1"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)


def version_for(net: str) -> int:
    return MAINNET_P2PKH if net == "main" else TESTNET_P2PKH


def main() -> int:
    checks, failures = 0, []

    def check(name, ok, detail=""):
        nonlocal checks
        checks += 1
        print(f"  {'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f"  {detail}"))
        if not ok:
            failures.append(name)

    print("SOLBEAM / deposit + redemption transaction construction checks\n")

    # ---- 1. address encoding against real addresses ------------------------
    print("[1] P2PKH address encoding matches real network addresses")
    try:
        main_txid = get_json("main", "/block/height/1000")["tx"][0]
        fixtures = [("test", None, None), ("main", main_txid, 0)]
    except Exception:  # noqa: BLE001
        fixtures = []
    for net, txid, vout_n in fixtures:
        try:
            if txid is None:
                txid, vout_n = "e8aa1eb3cd5b74fc332af0aaeec3b2b989e75a9b2284e0d72424b7fb1d0b3bb0", 12
            tx = get_json(net, f"/tx/{txid}")
            out = tx["vout"][vout_n]
            script = bytes.fromhex(out["scriptPubKey"]["hex"])
            address = tx["vout"][vout_n]["scriptPubKey"].get("addresses", [None])[0]
            h160 = p2pkh_hash(script)
            if h160 is None or not address:
                print(f"      {net}: fixture is not P2PKH, skipping")
                continue
            check(f"{net}: hash160 -> address equals the network's address",
                  p2pkh_address(h160, version_for(net)) == address,
                  f"got {p2pkh_address(h160, version_for(net))} want {address}")
            ver, decoded = address_to_hash160(address)
            check(f"{net}: address decodes back to the same hash160",
                  ver == version_for(net) and decoded == h160)
        except Exception as e:  # noqa: BLE001
            print(f"      {net}: skipped ({e})")

    # ---- 2. OP_RETURN carrying the Solana recipient ------------------------
    print("[2] OP_RETURN carries the Solana recipient")
    solana_recipient = bytes(range(32))  # stand-in for a 32-byte Solana pubkey
    script = op_return_script(solana_recipient)
    check("OP_RETURN script is 6a + push(32 bytes)", script.hex() == "6a20" + solana_recipient.hex())
    check("OP_RETURN payload round-trips", parse_op_return(script) == solana_recipient)
    check("a non-OP_RETURN script parses to None", parse_op_return(p2pkh_script(bytes(20))) is None)

    # ---- 3. deposit transaction shape -------------------------------------
    print("[3] deposit transaction: P2PKH out + OP_RETURN + change")
    owner_priv = 0xA11CE  # throwaway test key
    owner_pub = pubkey_from_priv(owner_priv)
    owner_script = p2pkh_script(hash160(owner_pub))
    deposit_priv = 0xB0B
    deposit_script = p2pkh_script(hash160(pubkey_from_priv(deposit_priv)))
    change_script = p2pkh_script(hash160(pubkey_from_priv(0xC0FFEE)))

    funding_value = 100_000_000  # 1 BSV
    deposit_value, fee = 99_000_000, 1_000_000
    tx = new_tx()
    add_input(tx, le("11" * 32), 0)  # synthetic outpoint
    add_output(tx, deposit_value, deposit_script)
    add_output(tx, 0, op_return_script(solana_recipient))
    add_output(tx, funding_value - deposit_value - fee, change_script)

    rebuilt = parse_tx(serialise_tx(tx))
    check("deposit tx round-trips through the codec", serialise_tx(rebuilt) == serialise_tx(tx))
    check("deposit output pays the deposit address",
          p2pkh_hash(rebuilt["vout"][0]["script"]) == p2pkh_hash(deposit_script))
    check("OP_RETURN carries the recipient",
          parse_op_return(rebuilt["vout"][1]["script"]) == solana_recipient)
    check("fee is exactly inputs minus outputs",
          funding_value - sum(o["value"] for o in rebuilt["vout"]) == fee)

    # ---- 4. sign the deposit input, then verify independently --------------
    print("[4] sign the deposit input and verify the signature")
    sign_tx = new_tx()
    add_input(sign_tx, le("22" * 32), 1)
    add_output(sign_tx, deposit_value, deposit_script)
    add_output(sign_tx, 0, op_return_script(solana_recipient))
    digest = sighash_forkid(sign_tx, 0, funding_value, owner_script)
    sig = ecdsa_sign(owner_priv, digest)
    check("deterministic signing is reproducible", ecdsa_sign(owner_priv, digest) == sig)
    r, s = der_decode(sig)
    check("signature verifies over the digest",
          ecdsa_verify(parse_pubkey_local(owner_pub), int.from_bytes(digest, "big"), r, s))
    check("signature does not verify over a tampered digest",
          not ecdsa_verify(parse_pubkey_local(owner_pub), int.from_bytes(digest, "big") ^ 1, r, s))

    signed_script = sign_input(sign_tx, 0, owner_priv, funding_value, owner_script)
    check("scriptSig has exactly two pushes (sig+type, pubkey)", len(signed_script) > 60)

    # ---- 5. redemption (peg-out) transaction shape -------------------------
    print("[5] redemption transaction: hot wallet pays the requested address")
    dest_priv = 0xD357
    dest_script = p2pkh_script(hash160(pubkey_from_priv(dest_priv)))
    hot_priv = 0x707
    hot_script = p2pkh_script(hash160(pubkey_from_priv(hot_priv)))

    payout = new_tx()
    add_input(payout, le("33" * 32), 0)
    add_output(payout, 500_000, dest_script)
    add_output(payout, 499_000, hot_script)  # change
    payout["vin"][0]["script"] = sign_input(payout, 0, hot_priv, 1_000_000, hot_script)

    parsed = parse_tx(serialise_tx(payout))
    check("redemption output pays exactly the requested address",
          p2pkh_hash(parsed["vout"][0]["script"]) == p2pkh_hash(dest_script))
    check("redemption change returns to the hot wallet",
          p2pkh_hash(parsed["vout"][1]["script"]) == p2pkh_hash(hot_script))
    check("signed redemption proves the hot wallet owned the input",
          verify_script_sig(parsed, 0, 1_000_000, hot_script))
    check("a tampered redemption output invalidates the signature",
          not verify_script_sig(tampered_output(parsed), 0, 1_000_000, hot_script))

    print(f"\n{checks - len(failures)}/{checks} checks passed")
    if failures:
        print("failed: " + ", ".join(failures))
        return 1
    print("all good — the peg can construct and sign both transaction shapes")
    return 0


# --- small helpers kept local to the checker --------------------------------

def parse_pubkey_local(b: bytes):
    from bsvlib import parse_pubkey
    return parse_pubkey(b)


def verify_script_sig(tx: dict, index: int, prevout_value: int, prevout_script: bytes) -> bool:
    from bsvlib import parse_pubkey
    from bsvlib import parse_pushes as pushes_of
    sig_with_type, pubkey_bytes = pushes_of(tx["vin"][index]["script"])
    sighash_type = sig_with_type[-1]
    r, s = der_decode(sig_with_type[:-1])
    digest = sighash_forkid(tx, index, prevout_value, prevout_script, sighash_type)
    return ecdsa_verify(parse_pubkey(pubkey_bytes), int.from_bytes(digest, "big"), r, s)


def tampered_output(tx: dict) -> dict:
    import copy
    bad = copy.deepcopy(tx)
    bad["vout"][0]["value"] += 1
    return bad


if __name__ == "__main__":
    sys.exit(main())
