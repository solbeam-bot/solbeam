#!/usr/bin/env python3
"""
SOLBEAM — synthetic regtest BSV chain (Python standard library only).

Phase 1A. The deposit path is exercised against a chain we build ourselves, so
it needs no BSV node and runs anywhere Python runs.

Why this is not circular. Phase 1B replays the *identical* cases against a real
SV Node and asserts that the resulting mint instruction is **byte-identical** to
the one produced here. Until 1B runs, this module proves our logic is
self-consistent; 1B is what proves it agrees with the reference implementation.

What this is NOT: a consensus implementation. Blocks are mined at the regtest
target (trivially easy), difficulty never retargets — which is genuine regtest
behaviour — and the genesis block is synthetic. Phase 1B replaces the genesis
with the real regtest one and pins every byte format.

Time is compressed by TIMESCALE (see TEST_PLAN.md §2.3.1): one hour of
production time is one second here. Tests must assert *relationships*, not
absolute seconds, or the matrix breaks the day the scale changes.
"""

import bsvlib as B

# Regtest mines at a trivially easy target: roughly half of all nonces pass.
REGTEST_BITS = B.REGTEST_BITS
REGTEST_TARGET = B.target_from_bits(REGTEST_BITS)

ZERO_HASH_DISPLAY = "00" * 32
COINBASE_SUBSIDY = 50 * 100_000_000          # 50 BSV in satoshis
COINBASE_MATURITY = 100                      # blocks before a coinbase is spendable

# Production: one hour -> one second.
TIMESCALE = 3600
CONFIRMATIONS_REQUIRED = 12                  # ~2 h in production -> 2 s here
MINT_WAIT_SECONDS = 2 * 3600 // TIMESCALE    # = 2
REDEMPTION_DEADLINE_SECONDS = 6 * 3600 // TIMESCALE   # = 6
CHALLENGE_WINDOW_SECONDS = 24 * 3600 // TIMESCALE     # = 24
UNBONDING_SECONDS = 7 * 24 * 3600 // TIMESCALE        # = 168, comfortably > 6 + 24


def minimal_push_int(n: int) -> bytes:
    """Minimal little-endian script number, as BIP34 requires for the coinbase height."""
    if n == 0:
        return b""
    raw = n.to_bytes((n.bit_length() + 7) // 8, "little")
    if raw[-1] & 0x80:
        raw += b"\x00"
    return raw


def coinbase_raw(height: int, pay_script: bytes, tag: bytes = b"SOLBEAM/1A") -> bytes:
    """A coinbase paying the subsidy to `pay_script`, with the height committed
    in the scriptSig so every block's coinbase txid is unique (BIP34)."""
    tx = B.new_tx()
    tx["vin"].append({
        "txid": bytes(32),
        "vout": 0xFFFFFFFF,
        "script": B.push_data(minimal_push_int(height)) + B.push_data(tag),
        "sequence": 0xFFFFFFFF,
    })
    B.add_output(tx, COINBASE_SUBSIDY, pay_script)
    return B.serialise_tx(tx)


class Chain:
    """A synthetic regtest chain: mine blocks, reorg them, spend the coinbase."""

    def __init__(self, tag: bytes = b"SOLBEAM/1A", start_time: int = 1_700_000_000):
        self.tag = tag
        self.bits = REGTEST_BITS
        self.target = REGTEST_TARGET
        self.blocks = []
        self.time = start_time
        # Genesis: synthetic, and unspendable via OP_RETURN — as Bitcoin's is.
        self.mine_block(coinbase_script=B.op_return_script(b"solbeam-1a-genesis"))

    # -- accessors ---------------------------------------------------------

    @property
    def tip(self) -> dict:
        return self.blocks[-1]

    @property
    def height(self) -> int:
        return self.blocks[-1]["height"]

    def at(self, height: int) -> dict:
        if not 0 <= height <= self.height:
            raise KeyError(f"no block at height {height}")
        return self.blocks[height]

    def headers(self, from_height: int, to_height: int) -> dict:
        """height -> header dict, for the range the verifier will walk."""
        return {h: self.blocks[h]["header"] for h in range(from_height, to_height + 1)}

    def find_tx(self, txid: str):
        """Return (block, index) for a txid, or None. Index 0 is the coinbase."""
        for block in self.blocks:
            if txid in block["txids"]:
                return block, block["txids"].index(txid)
        return None

    # -- mining ------------------------------------------------------------

    def _mine(self, header: dict) -> dict:
        for nonce in range(0x100000000):
            header["nonce"] = nonce
            if B.hash_as_int(header) <= self.target:
                return header
        raise RuntimeError("no valid nonce found — the target is not the regtest one")

    def mine_block(self, txs_raw=(), coinbase_script: bytes = b"") -> dict:
        """Mine one block. `txs_raw` are serialised non-coinbase transactions."""
        height = len(self.blocks)
        cb = coinbase_raw(height, coinbase_script or B.op_return_script(self.tag), self.tag)
        raws = [cb] + list(txs_raw)
        txids = [B.txid_of(r) for r in raws]
        root = B.merkle_root(txids)
        prev = self.blocks[-1]["hash"] if self.blocks else ZERO_HASH_DISPLAY
        self.time += 1
        header = self._mine({
            "version": 0x20000000,
            "previousblockhash": prev,
            "merkleroot": root[::-1].hex(),
            "time": self.time,
            "bits": self.bits,
            "nonce": 0,
        })
        block = {
            "height": height,
            "header": header,
            "hash": B.block_hash(header),
            "raw": raws,
            "txids": txids,
            "merkle": root,
            "coinbase_script": coinbase_script or B.op_return_script(self.tag),
        }
        self.blocks.append(block)
        return block

    def mine_empty(self, count: int, coinbase_script: bytes = b"") -> None:
        for _ in range(count):
            self.mine_block(coinbase_script=coinbase_script)

    # -- reorg -------------------------------------------------------------

    def invalidate_from(self, height: int) -> list:
        """Drop the block at `height` and everything after it — a reorg.

        Returns the orphaned blocks, so a test can assert a deposit that lived
        only on the discarded branch is no longer mintable.
        """
        if not 1 <= height <= self.height:
            raise ValueError(f"cannot invalidate height {height} (tip is {self.height})")
        orphaned = self.blocks[height:]
        self.blocks = self.blocks[:height]
        return orphaned

    # -- spendable coins ---------------------------------------------------

    def mature_coinbases(self) -> list:
        """Coinbase outputs buried by at least COINBASE_MATURITY blocks."""
        out = []
        for block in self.blocks:
            if self.height - block["height"] >= COINBASE_MATURITY:
                out.append({
                    "txid": block["txids"][0],
                    "vout": 0,
                    "value": COINBASE_SUBSIDY,
                    "script": block["coinbase_script"],
                    "height": block["height"],
                })
        return out
