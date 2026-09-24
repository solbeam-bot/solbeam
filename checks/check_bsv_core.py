#!/usr/bin/env python3
"""
SOLBEAM — BSV header / proof-of-work / Merkle checks.

Validates the logic the Solana light client must reproduce on-chain, against
live BSV data: header serialisation and hashing, the compact-bits target,
parent linkage, Merkle root rebuild, branch folding, the odd-level duplication
rule, and tamper rejection.

Usage:  python3 check_bsv_core.py
"""

import json
import sys
import urllib.error
import urllib.request

from bsvlib import le, sha256d

API = {
    "main": "https://api.whatsonchain.com/v1/bsv/main",
    "test": "https://api.whatsonchain.com/v1/bsv/test",
}

MAIN_FIXTURE = {
    "height": 800000,
    "hash": "000000000000000000ad9056924410005d91b57f100bce345944e5caf56e8565",
    "previousblockhash": "00000000000000000b6ae23bbe9f549844c20943d8c20b8ceedbae8aa1dde8e0",
    "merkleroot": "244993b9d8d961f5b4c91afa569adf9b6d8cd18e0bb6f769f5e62cdf2cc1468d",
    "version": 770793472,
    "time": 1688957834,
    "bits": "180d589d",
    "nonce": 742652432,
}

# Testnet blocks with small transaction counts, so the whole tree is
# downloadable while still exercising odd-level duplication.
MERKLE_FIXTURES = [("test", 1759725), ("test", 1759683), ("test", 1759717)]
MERKLE_FALLBACK = ("main", 1000)


def h(a: bytes, b: bytes) -> bytes:
    return sha256d(a + b)


def header_bytes(hdr: dict) -> bytes:
    """version(4) prev(32) merkle(32) time(4) bits(4) nonce(4), little-endian."""
    raw = b"".join([
        (int(hdr["version"]) & 0xFFFFFFFF).to_bytes(4, "little"),
        le(hdr["previousblockhash"]),
        le(hdr["merkleroot"]),
        int(hdr["time"]).to_bytes(4, "little"),
        int(hdr["bits"], 16).to_bytes(4, "little"),
        int(hdr["nonce"]).to_bytes(4, "little"),
    ])
    assert len(raw) == 80, f"header must be 80 bytes, got {len(raw)}"
    return raw


def block_hash(hdr: dict) -> str:
    return sha256d(header_bytes(hdr))[::-1].hex()


def target_from_bits(bits_hex: str) -> int:
    bits = int(bits_hex, 16)
    exponent, mantissa = bits >> 24, bits & 0x007FFFFF
    return mantissa >> (8 * (3 - exponent)) if exponent <= 3 \
        else mantissa << (8 * (exponent - 3))


def hash_as_int(hdr: dict) -> int:
    return int.from_bytes(sha256d(header_bytes(hdr)), "little")


def merkle_root(txids_display: list) -> bytes:
    level = [le(t) for t in txids_display]
    if not level:
        raise ValueError("empty block")
    while len(level) > 1:
        if len(level) % 2:
            level.append(level[-1])
        level = [h(level[i], level[i + 1]) for i in range(0, len(level), 2)]
    return level[0]


def merkle_branch(txids_display: list, index: int) -> list:
    level, proof, i = [le(t) for t in txids_display], [], index
    while len(level) > 1:
        if len(level) % 2:
            level.append(level[-1])
        proof.append(level[i ^ 1])
        level = [h(level[j], level[j + 1]) for j in range(0, len(level), 2)]
        i //= 2
    return proof


def fold_branch(txid_display: str, index: int, branch: list) -> bytes:
    current, i = le(txid_display), index
    for sibling in branch:
        current = h(current, sibling) if i % 2 == 0 else h(sibling, current)
        i //= 2
    return current


def get(net: str, path: str):
    req = urllib.request.Request(API[net] + path, headers={"User-Agent": "solbeam-poc/0.1"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)


def usable_block(net: str, height: int, max_txs: int = 512):
    try:
        block = get(net, f"/block/height/{height}")
    except Exception:  # noqa: BLE001 - probing on purpose
        return None
    txs = block.get("tx") or []
    return block if txs and len(txs) == block.get("txcount", len(txs)) and len(txs) <= max_txs else None


def main() -> int:
    checks, failures = 0, []

    def check(name, ok, detail=""):
        nonlocal checks
        checks += 1
        print(f"  {'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f"  {detail}"))
        if not ok:
            failures.append(name)

    print("SOLBEAM / BSV core checks\n")

    print("[1] header serialisation and double-SHA256")
    hdr = dict(MAIN_FIXTURE)
    try:
        live = get("main", f"/block/{MAIN_FIXTURE['height']}/header")
        hdr = {**hdr, **{k: live[k] for k in MAIN_FIXTURE if k in live}}
        print(f"      live header, mainnet height {hdr['height']}")
    except Exception as e:  # noqa: BLE001
        print(f"      live fetch failed ({e}); using pinned fixture")
    check("header serialises to exactly 80 bytes", len(header_bytes(hdr)) == 80)
    check("double-SHA256 matches the block hash", block_hash(hdr) == hdr["hash"],
          f"got {block_hash(hdr)}")

    print("[2] proof of work against the compact bits target")
    target = target_from_bits(hdr["bits"])
    print(f"      bits {hdr['bits']} -> target {target:#066x}")
    check("block hash meets its target", hash_as_int(hdr) <= target)
    tampered = dict(hdr, nonce=(int(hdr["nonce"]) + 1) & 0xFFFFFFFF)
    check("tampered nonce does NOT meet the target", hash_as_int(tampered) > target)

    print("[3] header linkage to the parent block")
    try:
        parent = get("main", f"/block/{hdr['height'] - 1}/header")
        check("prev hash points at the parent block", hdr["previousblockhash"] == parent["hash"])
    except Exception as e:  # noqa: BLE001
        print(f"      skipped (parent fetch failed: {e})")

    print("[4] synthetic Merkle trees (ordering and duplication)")
    leaves = [sha256d(bytes([i])) for i in range(5)]
    disp = [x[::-1].hex() for x in leaves]
    l1 = [h(leaves[0], leaves[1]), h(leaves[2], leaves[3]), h(leaves[4], leaves[4])]
    l2 = [h(l1[0], l1[1]), h(l1[2], l1[2])]
    expected_odd = h(l2[0], l2[1])
    check("5-leaf root matches a hand-built tree", merkle_root(disp) == expected_odd)
    check("every 5-leaf branch folds to the root",
          all(fold_branch(disp[i], i, merkle_branch(disp, i)) == expected_odd for i in range(5)))
    even = [sha256d(bytes([100 + i])) for i in range(4)]
    even_disp = [x[::-1].hex() for x in even]
    expected_even = h(h(even[0], even[1]), h(even[2], even[3]))
    check("4-leaf root matches a hand-built tree", merkle_root(even_disp) == expected_even)
    check("every 4-leaf branch folds to the root",
          all(fold_branch(even_disp[i], i, merkle_branch(even_disp, i)) == expected_even
              for i in range(4)))

    print("[5] Merkle root and branch against real blocks")
    tested_real = False
    for net, height in MERKLE_FIXTURES + [MERKLE_FALLBACK]:
        block = usable_block(net, height)
        if not block:
            print(f"      {net} {height}: unavailable, skipping")
            continue
        txs = block["tx"]
        root = merkle_root(txs)
        label = f"{net} {height} ({len(txs)} txs)"
        check(f"{label}: rebuilt root equals merkleroot",
              root[::-1].hex() == block["merkleroot"], f"got {root[::-1].hex()}")
        if len(txs) > 1:
            idxs = sorted({0, len(txs) // 2, len(txs) - 1})
            check(f"{label}: branches fold for indices {idxs}",
                  all(fold_branch(txs[i], i, merkle_branch(txs, i)) == root for i in idxs))
            bad = merkle_branch(txs, 0)
            bad[-1] = bytes(32)
            check(f"{label}: a tampered branch does not fold",
                  fold_branch(txs[0], 0, bad) != root)
            tested_real = True
    check("at least one real multi-tx block verified", tested_real)

    print(f"\n{checks - len(failures)}/{checks} checks passed")
    if failures:
        print("failed: " + ", ".join(failures))
        return 1
    print("all good — this is the logic the Solana light client must reproduce on-chain")
    return 0


if __name__ == "__main__":
    sys.exit(main())
