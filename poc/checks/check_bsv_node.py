#!/usr/bin/env python3
"""
SOLBEAM — Phase 1B: pin our byte formats against a real SV Node.

Phase 1A proved our logic is self-consistent. This proves it agrees with the
reference implementation, which is the one thing synthetic testing cannot
establish. It does that by rebuilding the *same* deposit on a real regtest
chain and asserting that every primitive we compute matches what the node
reports, byte for byte.

The specific unknowns this settles:
  * our 80-byte header serialisation vs the node's raw header
  * our Merkle branch and index convention vs `getmerkleproof2`
  * our transaction codec vs `decoderawtransaction`
  * our txid computation vs the node's txids

Usage:
    python3 check_bsv_node.py               # needs a running SV Node
    python3 check_bsv_node.py --selftest    # no node: exercises the same code
                                            # paths against a synthetic stub

Configuration (see poc/.env.example):
    SOLBEAM_RPC, SOLBEAM_RPC_USER, SOLBEAM_RPC_PASS

Without SOLBEAM_RPC the checker SKIPS and exits 0, so the suite stays green on
a machine that has no node. Set SOLBEAM_REQUIRE_NODE=1 to make skipping a
failure — which is what CI on the x86_64 host should do.

What `--selftest` does and does not prove: it runs every parse and every
assertion against a stub that *imitates* the node's JSON. It therefore pins our
code paths, not the node's actual response shape. Only a live node settles that,
and the first live run records the raw response to
`fixtures/node_merkleproof_raw.json` so the shape becomes part of the repo.
"""

import json
import os
import sys
import traceback
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bsvlib as B
import bsvchain as C
from check_bsv_pegin import DEPOSIT_SCRIPT, RECIPIENT, DEPOSIT_VALUE, \
    build_deposit, serialise_instruction, verify_deposit, p2pkh_script_for

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURE_DIR = os.path.normpath(os.path.join(HERE, "..", "fixtures"))

RPC_URL = os.environ.get("SOLBEAM_RPC", "")
RPC_USER = os.environ.get("SOLBEAM_RPC_USER", "solbeam")
RPC_PASS = os.environ.get("SOLBEAM_RPC_PASS", "solbeam")
REQUIRE_NODE = os.environ.get("SOLBEAM_REQUIRE_NODE", "0") == "1"

MIN_CONFIRMATIONS = C.CONFIRMATIONS_REQUIRED

# How much the funding transaction pays to the deposit owner. It must exceed the
# deposit amount plus the fee, or the change output is negative — which is the
# exact bug the first live run hit: `sendtoaddress(addr, 1.0)` funded precisely
# the 1 BSV deposit, so change was -1000 sats and `.to_bytes` raised
# OverflowError. The offline stub funded from a 50 BSV coinbase and never
# exercised the same arithmetic, which is why the selftest passed while the live
# run failed. Both paths now use these constants.
FUND_BSV = 2.0
FUND_VALUE = int(FUND_BSV * 100_000_000)
DEPOSIT_FEE = 1000


class NodeUnavailable(Exception):
    pass


# ---------------------------------------------------------------------------
# transports — a real JSON-RPC client, and an in-process stub with the same API
# ---------------------------------------------------------------------------

class NodeRPC:
    """Minimal standalone JSON-RPC client for SV Node (no dependencies)."""

    def __init__(self, url, user, password):
        self.url = url
        self.auth = f"{user}:{password}"
        self._id = 0

    def call(self, method, *params):
        self._id += 1
        payload = json.dumps({"jsonrpc": "1.0", "id": self._id,
                              "method": method, "params": list(params)}).encode()
        req = urllib.request.Request(self.url, data=payload, headers={
            "content-type": "text/plain",
            "authorization": "Basic " + __import__("base64").b64encode(
                self.auth.encode()).decode(),
        })
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                body = json.load(r)
        except Exception as e:  # noqa: BLE001
            raise NodeUnavailable(str(e)) from e
        if body.get("error"):
            raise RuntimeError(f"{method}: {body['error']}")
        return body["result"]

    # -- the surface the pin needs ----------------------------------------
    def getblockcount(self):                      return self.call("getblockcount")
    def getblockhash(self, h):                    return self.call("getblockhash", h)
    def getblockheader(self, h, verbose=True):    return self.call("getblockheader", h, verbose)
    def getblock(self, h, verbose=1):             return self.call("getblock", h, verbose)
    def getrawtransaction(self, txid, verbose=False):
        return self.call("getrawtransaction", txid, verbose)
    def decoderawtransaction(self, raw):          return self.call("decoderawtransaction", raw)
    def sendrawtransaction(self, raw):            return self.call("sendrawtransaction", raw)
    def generatetoaddress(self, n, addr):         return self.call("generatetoaddress", n, addr)
    def getnewaddress(self):                      return self.call("getnewaddress", "")
    def sendtoaddress(self, addr, amount):        return self.call("sendtoaddress", addr, amount)
    def getnetworkhashps(self):                   return self.call("getnetworkhashps")
    def getblockchaininfo(self):                  return self.call("getblockchaininfo")
    def getmerkleproof2(self, txid):              return self.call("getmerkleproof2", txid)


class StubRPC:
    """The same interface, backed by the synthetic chain from Phase 1A.

    It imitates SV Node's JSON shape. Where the real shape is unknown — and
    `getmerkleproof2` is exactly that — it uses the most likely one AND the
    harness prints what it received, so the first live run is a comparison
    rather than a surprise.
    """

    def __init__(self, chain: C.Chain):
        self.chain = chain

    def getblockcount(self):
        return self.chain.height

    def getblockhash(self, h):
        return self.chain.at(h)["hash"]

    def getblockheader(self, h, verbose=True):
        blk = self.chain.at(h) if isinstance(h, int) else \
            next(b for b in self.chain.blocks if b["hash"] == h)
        if not verbose:
            return B.header_bytes(blk["header"]).hex()
        hdr = dict(blk["header"])
        hdr["height"] = blk["height"]
        hdr["hash"] = blk["hash"]
        hdr["bits"] = f"{int(hdr['bits']):08x}"
        return hdr

    def getblock(self, h, verbose=1):
        blk = self.chain.at(h) if isinstance(h, int) else \
            next(b for b in self.chain.blocks if b["hash"] == h)
        if not verbose:
            return "".join(r.hex() for r in blk["raw"])
        return {"hash": blk["hash"], "height": blk["height"],
                "merkleroot": blk["merkle"][::-1].hex(), "tx": list(blk["txids"])}

    def getrawtransaction(self, txid, verbose=False):
        for blk in self.chain.blocks:
            if txid in blk["txids"]:
                raw = blk["raw"][blk["txids"].index(txid)]
                if not verbose:
                    return raw.hex()
                tx = B.parse_tx(raw)
                return {"txid": txid, "version": tx["version"],
                        "vout": [{"value": o["value"] / 1e8,
                                  "n": i,
                                  "scriptPubKey": {"hex": o["script"].hex()}}
                                 for i, o in enumerate(tx["vout"])]}
        raise RuntimeError(f"tx {txid} not found")

    def decoderawtransaction(self, raw):
        tx = B.parse_tx(bytes.fromhex(raw))
        return {"txid": B.txid_of(bytes.fromhex(raw)), "version": tx["version"],
                "vin": [{"txid": v["txid"][::-1].hex(), "vout": v["vout"]}
                        for v in tx["vin"]],
                "vout": [{"value": o["value"] / 1e8, "n": i,
                          "scriptPubKey": {"hex": o["script"].hex()}}
                         for i, o in enumerate(tx["vout"])]}

    def sendrawtransaction(self, raw):
        return B.txid_of(bytes.fromhex(raw))

    def getmerkleproof2(self, txid):
        blk, index = self.chain.find_tx(txid)
        branch = B.merkle_branch(blk["txids"], index)
        return {"txid": txid, "blockhash": blk["hash"], "index": index,
                "merkleroot": blk["merkle"][::-1].hex(),
                "proof": [b.hex() for b in branch]}

    # not needed by the pin, present so the surface matches
    def generatetoaddress(self, n, addr): raise NotImplementedError
    def getnewaddress(self):              raise NotImplementedError
    def sendtoaddress(self, a, v):        raise NotImplementedError
    def getnetworkhashps(self):           return 0


def extract_branch(proof: dict):
    """Pull (branch, index) out of a getmerkleproof2 response.

    The exact shape is one of the things Phase 1B exists to discover, so this
    accepts the plausible spellings and reports what it actually saw.
    """
    raw_keys = sorted(proof.keys())
    branch = None
    for key in ("proof", "branches", "merkle", "merklebranch", "branch"):
        if key in proof and isinstance(proof[key], list):
            branch = [bytes.fromhex(x) if isinstance(x, str) else bytes(x)
                      for x in proof[key]]
            break
    if branch is None:
        raise AssertionError(f"no branch found in {raw_keys}")
    for key in ("index", "pos", "txpos", "position"):
        if key in proof:
            return branch, int(proof[key]), raw_keys
    raise AssertionError(f"no index found in {raw_keys}")


# ---------------------------------------------------------------------------
# the pin
# ---------------------------------------------------------------------------

def run_pin(rpc, log, check) -> None:
    """Every assertion Phase 1B exists to make. `check(name, ok, detail)`."""

    # -- build a real deposit on the node's chain -------------------------
    node_txid = None
    raw_from_node = None
    if isinstance(rpc, NodeRPC):
        miner_addr = rpc.getnewaddress()
        rpc.generatetoaddress(101, miner_addr)
        priv = int.from_bytes(bytes.fromhex("11" * 32), "big")
        fund_script = p2pkh_script_for(priv)
        fund_addr = B.p2pkh_address(B.hash160(B.pubkey_from_priv(priv)), B.TESTNET_P2PKH)
        fund_txid = rpc.sendtoaddress(fund_addr, FUND_BSV)
        rpc.generatetoaddress(1, miner_addr)
        funded = rpc.getrawtransaction(fund_txid, True)
        vout = next(o for o in funded["vout"]
                    if o["scriptPubKey"]["hex"].lower() == fund_script.hex())
        utxo = {"txid": fund_txid, "vout": vout["n"],
                "value": int(round(vout["value"] * 1e8)), "script": fund_script}
        log(f"     funded {utxo['value']} sats; deposit {DEPOSIT_VALUE}, "
            f"fee {DEPOSIT_FEE}, change {utxo['value'] - DEPOSIT_VALUE - DEPOSIT_FEE}")
        deposit_tx = build_deposit(utxo, RECIPIENT, amount=DEPOSIT_VALUE,
                                   fee=DEPOSIT_FEE, priv=priv)
        deposit_raw = B.serialise_tx(deposit_tx)
        deposit_txid = rpc.sendrawtransaction(deposit_raw.hex())
        node_txid = deposit_txid
        rpc.generatetoaddress(MIN_CONFIRMATIONS, miner_addr)
        raw_from_node = rpc.getrawtransaction(deposit_txid, False)
    else:
        chain = rpc.chain
        priv = int.from_bytes(bytes.fromhex("22" * 32), "big")
        miner_script = p2pkh_script_for(priv)
        mature = [u for u in chain.mature_coinbases() if u["script"] == miner_script]

        # Mirror the live path exactly: an intermediate funding transaction that
        # pays FUND_VALUE to the deposit owner, which the deposit then spends.
        fund_src = mature[0]
        fund_tx = B.new_tx()
        B.add_input(fund_tx, B.le(fund_src["txid"]), fund_src["vout"])
        B.add_output(fund_tx, FUND_VALUE, miner_script)
        fund_tx["vin"][0]["script"] = B.sign_input(
            fund_tx, 0, priv, fund_src["value"], fund_src["script"])
        fund_raw = B.serialise_tx(fund_tx)
        chain.mine_block([fund_raw], coinbase_script=miner_script)
        utxo = {"txid": B.txid_of(fund_raw), "vout": 0,
                "value": FUND_VALUE, "script": miner_script}
        log(f"     funded {utxo['value']} sats (mirrors the live path); "
            f"change {utxo['value'] - DEPOSIT_VALUE - DEPOSIT_FEE}")

        deposit_tx = build_deposit(utxo, RECIPIENT, amount=DEPOSIT_VALUE,
                                   fee=DEPOSIT_FEE, priv=priv)
        deposit_raw = B.serialise_tx(deposit_tx)
        deposit_txid = B.txid_of(deposit_raw)
        chain.mine_block([deposit_raw], coinbase_script=miner_script)
        chain.mine_empty(MIN_CONFIRMATIONS - 1, coinbase_script=miner_script)
        node_txid = deposit_txid

    # -- 1. our txid matches the node's -----------------------------------
    log("1. transaction identity")
    check("our txid equals the node's txid",
          B.txid_of(deposit_raw) == deposit_txid == node_txid,
          f"ours {B.txid_of(deposit_raw)[:16]} vs node {node_txid[:16]}")
    if raw_from_node:
        check("the node returns the same transaction bytes we broadcast",
              bytes.fromhex(raw_from_node) == deposit_raw)

    # -- 2. our transaction codec matches decoderawtransaction -------------
    log("2. transaction codec vs decoderawtransaction")
    decoded = rpc.decoderawtransaction(deposit_raw.hex())
    ours = B.parse_tx(deposit_raw)
    check("version matches", int(decoded["version"]) == int(ours["version"]))
    check("input count matches", len(decoded["vin"]) == len(ours["vin"]))
    check("output count matches", len(decoded["vout"]) == len(ours["vout"]))
    check("every output value matches",
          all(int(round(o["value"] * 1e8)) == m["value"]
              for o, m in zip(decoded["vout"], ours["vout"])))
    check("every output script matches byte for byte",
          all(o["scriptPubKey"]["hex"].lower() == m["script"].hex()
              for o, m in zip(decoded["vout"], ours["vout"])))

    # -- 3. find the block, and pin the header ----------------------------
    log("3. header serialisation")
    height = rpc.getblockcount() - (MIN_CONFIRMATIONS - 1)
    block_hash = rpc.getblockhash(height)
    block = rpc.getblock(block_hash, 1)
    check("the deposit is in the block at that height", deposit_txid in block["tx"])

    header_dict = rpc.getblockheader(block_hash, True)
    header_raw = rpc.getblockheader(block_hash, False)
    if isinstance(header_raw, str):
        check("our 80-byte header serialisation equals the node's raw header",
              B.header_bytes(header_dict).hex() == header_raw.lower(),
              f"ours {B.header_bytes(header_dict).hex()[:32]}... "
              f"node {header_raw.lower()[:32]}...")
    check("our block hash equals the node's",
          B.block_hash(header_dict) == block_hash, f"{B.block_hash(header_dict)} vs {block_hash}")

    # -- 4. Merkle root and branch versus the node ------------------------
    log("4. Merkle root and branch vs getmerkleproof2")
    txids = list(block["tx"])
    index = txids.index(deposit_txid)
    ours_root = B.merkle_root(txids)
    check("our Merkle root equals the block's merkleroot",
          ours_root[::-1].hex() == block["merkleroot"],
          f"{ours_root[::-1].hex()} vs {block['merkleroot']}")

    proof = rpc.getmerkleproof2(deposit_txid)
    if isinstance(rpc, NodeRPC):
        os.makedirs(FIXTURE_DIR, exist_ok=True)
        path = os.path.join(FIXTURE_DIR, "node_merkleproof_raw.json")
        with open(path, "w") as f:
            json.dump(proof, f, indent=2, sort_keys=True)
            f.write("\n")
        log(f"     raw getmerkleproof2 response recorded to {path}")

    node_branch, node_index, raw_keys = extract_branch(proof)
    log(f"     node returned keys: {raw_keys}")
    ours_branch = B.merkle_branch(txids, index)

    check("index convention agrees", node_index == index,
          f"node {node_index} vs ours {index}")
    check("branch length agrees", len(node_branch) == len(ours_branch),
          f"node {len(node_branch)} vs ours {len(ours_branch)}")
    same = (len(node_branch) == len(ours_branch)
            and all(a == b for a, b in zip(node_branch, ours_branch)))
    check("EVERY branch element matches byte for byte", same,
          "ours " + ours_branch[0].hex()[:16] + " node " + node_branch[0].hex()[:16]
          if ours_branch and node_branch else "empty branch")
    check("the node's branch folds to the block root",
          B.fold_branch(deposit_txid, node_index, node_branch) == B.le(block["merkleroot"]))
    check("our branch folds to the block root",
          B.fold_branch(deposit_txid, index, ours_branch) == B.le(block["merkleroot"]))

    # -- 5. the instruction, from node-supplied primitives ----------------
    log("5. the mint instruction, built from the node's own data")
    view = {
        "tip_height": rpc.getblockcount(),
        "checkpoint": {"height": height, "hash": block_hash},
        "headers": {height: header_dict},
    }
    node_proof = {
        "tx_raw": deposit_raw,
        "height": height,
        "txid": deposit_txid,
        "vout": 0,
        "amount": DEPOSIT_VALUE,
        "recipient": RECIPIENT,
        "index": node_index,
        "branch": node_branch,
    }
    ours_proof = dict(node_proof, index=index, branch=ours_branch)

    ins_node = serialise_instruction(verify_deposit(node_proof, view))
    ins_ours = serialise_instruction(verify_deposit(ours_proof, view))
    check("our verifier accepts a proof built from the node's primitives", True)
    check("the instruction is identical whichever branch we use",
          ins_node == ins_ours)
    check("the instruction embeds the node's header",
          ins_node[5:85] == B.header_bytes(header_dict))


# ---------------------------------------------------------------------------

def main() -> int:
    selftest = "--selftest" in sys.argv

    if not selftest and not RPC_URL:
        print("SOLBEAM / Phase 1B — SV Node format pin\n")
        print("  SKIP  SOLBEAM_RPC is not set, so there is no node to pin against.")
        print("        Start one with poc/scripts/regtest-up.sh, then re-run.")
        if REQUIRE_NODE:
            print("        SOLBEAM_REQUIRE_NODE=1 is set, so this is a failure.")
            return 1
        return 0

    print("SOLBEAM / Phase 1B — SV Node format pin"
          + (" (selftest, no node)\n" if selftest else "\n"))

    checks, failures = 0, []

    def check(name, ok, detail=""):
        nonlocal checks
        checks += 1
        print(f"  {'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f"  {detail}"))
        if not ok:
            failures.append(name)

    def log(msg):
        print(msg)

    if selftest:
        chain = C.Chain()
        chain.mine_empty(115, coinbase_script=p2pkh_script_for(
            int.from_bytes(bytes.fromhex("22" * 32), "big")))
        rpc = StubRPC(chain)
        log("     using an in-process stub that imitates the node's JSON shape")
        log("     NOTE: this pins our code paths, NOT the node's real response\n")
    else:
        rpc = NodeRPC(RPC_URL, RPC_USER, RPC_PASS)
        try:
            info = rpc.getblockchaininfo() if hasattr(rpc, "getblockchaininfo") \
                else {"chain": "unknown"}
            log(f"     node at {RPC_URL}, chain {info.get('chain')}\n")
        except NodeUnavailable as e:
            print(f"  FAIL  node unreachable at {RPC_URL}: {e}")
            return 1

    try:
        run_pin(rpc, log, check)
    except Exception as e:  # noqa: BLE001
        # Print the traceback. This harness runs on a remote box: a bare
        # exception name costs a whole round trip to diagnose, and that is
        # exactly what the first live run cost.
        traceback.print_exc()
        check(f"pin run completed ({type(e).__name__})", False, str(e))

    print(f"\n{checks - len(failures)}/{checks} checks passed")
    if failures:
        print("failed: " + ", ".join(failures))
        return 1
    if selftest:
        print("all good — the pin harness agrees with itself. ONLY a live SV Node")
        print("can settle the real getmerkleproof2 shape; run without --selftest.")
    else:
        print("all good — our byte formats agree with SV Node")
    return 0


if __name__ == "__main__":
    sys.exit(main())
