#!/usr/bin/env python3
"""Add the raw 80-byte mainnet headers to `headers_mainnet.json`.

The fixture was fetched in W1.1 with `height`, `hash`, `prev`, `bits`, `time`
and `chainwork` — everything the *difficulty* replay needs. The on-chain
integration test needs the real serialised header as well, because
`push_header` hashes the bytes and checks linkage by hash. Those bytes are not
derivable from the summary fields: version, merkle root and nonce are all
required to reproduce the hash.

So fetch the missing fields from WhatsOnChain, rebuild each header, and verify
the rebuild against the fixture's own `hash` and `prev` before writing anything.
A mismatch is fatal — a fixture that does not hash to its own recorded hash
would make the integration test meaningless.

Idempotent: responses are cached under /tmp/woc_headers_cache.json, and records
that already carry a verified `raw` are left alone.
"""
import hashlib
import json
import os
import struct
import sys
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURE = os.path.join(HERE, "headers_mainnet.json")
CACHE = "/tmp/woc_headers_cache.json"
API = "https://api.whatsonchain.com/v1/bsv/main/block/height/{}"


def sha256d(b: bytes) -> bytes:
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def fetch(height: int) -> dict:
    req = urllib.request.Request(API.format(height), headers={"User-Agent": "solbeam-fixture/1.0"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)


def build(rec: dict, blk: dict) -> bytes:
    version = struct.pack("<I", blk["version"] & 0xFFFFFFFF)
    prev = bytes.fromhex(rec["prev"])[::-1]
    merkle = bytes.fromhex(blk["merkleroot"])[::-1]
    t = struct.pack("<I", rec["time"])
    bits = struct.pack("<I", int(rec["bits"], 16))
    nonce = struct.pack("<I", blk["nonce"] & 0xFFFFFFFF)
    raw = version + prev + merkle + t + bits + nonce
    assert len(raw) == 80, len(raw)
    return raw


def main() -> int:
    with open(FIXTURE) as f:
        headers = json.load(f)

    cache = {}
    if os.path.exists(CACHE):
        with open(CACHE) as f:
            cache = json.load(f)

    for i, rec in enumerate(headers):
        h = rec["height"]
        if "raw" in rec:
            raw = bytes.fromhex(rec["raw"])
            assert sha256d(raw)[::-1].hex() == rec["hash"], f"existing raw wrong at {h}"
            continue
        key = str(h)
        if key not in cache:
            for attempt in range(5):
                try:
                    cache[key] = fetch(h)
                    break
                except Exception as e:  # noqa: BLE001 — retry any transport error
                    if attempt == 4:
                        print(f"FAILED to fetch {h}: {e}", file=sys.stderr)
                        return 1
                    time.sleep(1.5 * (attempt + 1))
            time.sleep(0.25)
            if i % 25 == 0:
                with open(CACHE, "w") as f:
                    json.dump(cache, f)
                print(f"  fetched {i}/{len(headers)}", file=sys.stderr, flush=True)
        blk = cache[key]
        assert blk["hash"] == rec["hash"], f"hash differs at {h}: {blk['hash']} vs {rec['hash']}"
        assert blk["previousblockhash"] == rec["prev"], f"prev differs at {h}"
        assert blk["time"] == rec["time"], f"time differs at {h}"
        assert blk["bits"].lower() == rec["bits"].lower(), f"bits differs at {h}"
        raw = build(rec, blk)
        assert sha256d(raw)[::-1].hex() == rec["hash"], f"rebuilt raw does not hash to {h}"
        rec["raw"] = raw.hex()

    with open(CACHE, "w") as f:
        json.dump(cache, f)
    with open(FIXTURE, "w") as f:
        json.dump(headers, f, indent=0)
        f.write("\n")

    # Final whole-file verification: every header hashes to its recorded hash and
    # every consecutive pair links.
    for i, rec in enumerate(headers):
        raw = bytes.fromhex(rec["raw"])
        assert sha256d(raw)[::-1].hex() == rec["hash"], f"post-write hash mismatch at {i}"
        if i:
            assert raw[4:36].hex() == bytes.fromhex(headers[i - 1]["hash"])[::-1].hex(), \
                f"post-write linkage gap at {rec['height']}"
    print(f"ok: {len(headers)} headers with raw bytes, full linkage, all hashes verified")
    return 0


if __name__ == "__main__":
    sys.exit(main())
