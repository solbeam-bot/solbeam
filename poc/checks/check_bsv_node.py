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
        except urllib.error.HTTPError as e:
            # SV Node returns HTTP 500 for JSON-RPC errors, and the useful part
            # is in the BODY: {"error":{"code":-32601,"message":"Method not
            # found"}}. Raising on the status alone throws that away, which cost
            # a round trip when getmerkleproof2 was called with the wrong
            # argument order.
            detail = ""
            try:
                detail = e.read().decode("utf-8", "replace")[:400]
            except Exception:  # noqa: BLE001
                pass
            raise NodeUnavailable(
                f"{method}: HTTP {e.code} {e.reason}"
                + (f" — {detail}" if detail else "")) from e
        except Exception as e:  # noqa: BLE001
            raise NodeUnavailable(f"{method}: {e}") from e
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

    def getmerkleproof2(self, blockhash, txid):
        """SV Node v1.1.1 signature: getmerkleproof2 "blockhash" "txid"
        ( includeFullTx targetType format ). Note the argument ORDER — block
        hash first. Passing only the txid makes the node treat it as a block
        hash and fail with an HTTP 500."""
        return self.call("getmerkleproof2", blockhash, txid)


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

    def getmerkleproof2(self, blockhash, txid):
        """Emits SV Node's real response shape.

        Confirmed from the v1.1.1 source (src/rpc/rawtransaction.cpp): the
        signature is `getmerkleproof2 "blockhash" "txid" ( includeFullTx
        targetType format )`, and the result is the TSC Merkle-proof form:

            { "flags": 2, "index": n, "txOrId": "<txid>",
              "target": {block header}, "nodes": ["hash", "*", ...] }

        `"*"` means "a copy of the node being calculated" — the odd-level
        duplication case. Our own merkle_branch() emits the duplicated hash
        itself, so the two encodings differ while proving the same thing. The
        stub emits the node's encoding so the parser is exercised faithfully.
        """
        blk, index = self.chain.find_tx(txid)
        if blockhash and blk["hash"] != blockhash:
            raise RuntimeError(f"tx {txid} is not in block {blockhash}")

        # Build the branch in the TSC encoding: walk the tree, and where our
        # sibling is the node itself, emit "*". Hashes go out in display order,
        # which is what the live node returned — the harness determines this
        # rather than trusting it (see interpret_nodes).
        level = [B.le(t) for t in blk["txids"]]
        nodes, i = [], index
        while len(level) > 1:
            if len(level) % 2:
                level.append(level[-1])
            sibling = level[i ^ 1]
            nodes.append("*" if sibling == level[i] else sibling[::-1].hex())
            level = [B.merkle_hash(level[j], level[j + 1]) for j in range(0, len(level), 2)]
            i //= 2

        # The live node returned a string here, not the header object the help
        # text describes, and omitted `flags` entirely.
        return {"index": index, "txOrId": txid,
                "target": blk["hash"], "nodes": nodes}

    # not needed by the pin, present so the surface matches
    def generatetoaddress(self, n, addr): raise NotImplementedError
    def getnewaddress(self):              raise NotImplementedError
    def sendtoaddress(self, a, v):        raise NotImplementedError
    def getnetworkhashps(self):           return 0


# The TSC proof's "*": "a copy of the node being calculated", i.e. the
# odd-level duplication case. Our merkle_branch() emits the duplicated hash
# itself, so the two encodings differ while proving the same thing.
DUPLICATE = None


def fold_nodes(txid_display: str, index: int, nodes: list) -> bytes:
    """Fold a branch of internal-order hashes (or DUPLICATE), leaf to root."""
    current, i = B.le(txid_display), index
    for node in nodes:
        if node is DUPLICATE:
            current = B.merkle_hash(current, current)
        elif i % 2 == 0:
            current = B.merkle_hash(current, node)
        else:
            current = B.merkle_hash(node, current)
        i //= 2
    return current


def materialise_branch(txid_display: str, index: int, nodes: list) -> list:
    """Resolve "*" into concrete 32-byte hashes.

    verify_deposit() takes concrete hashes, because those are what fold
    on-chain; "*" is only a wire-format shorthand for "the node being
    calculated".
    """
    current, i, out = B.le(txid_display), index, []
    for node in nodes:
        if node is DUPLICATE:
            out.append(current)
            current = B.merkle_hash(current, current)
        else:
            out.append(node)
            current = B.merkle_hash(current, node) if i % 2 == 0 \
                else B.merkle_hash(node, current)
        i //= 2
    return out


# Every plausible way SV Node might encode the branch. Rather than assume one,
# try them all and report which actually folds to the block root.
NODE_ENCODINGS = (
    ("display-order hashes, leaf-to-root", True, False),
    ("internal-order hashes, leaf-to-root", False, False),
    ("display-order hashes, root-to-leaf", True, True),
    ("internal-order hashes, root-to-leaf", False, True),
)


def extract_nodes(proof: dict):
    """Pull (raw nodes, index, keys) out of a getmerkleproof2 response.

    Confirmed shape (SV Node v1.1.1, src/rpc/rawtransaction.cpp):
        { "index": n, "txOrId": "<txid>", "target": ..., "nodes": [...] }

    The branch key is `nodes`, not `proof`; a node may be the string "*"; and
    `flags` is absent from the real response even though `getmerkleproof`'s
    help text implies otherwise.
    """
    raw_keys = sorted(proof.keys())
    nodes = None
    for key in ("nodes", "proof", "branches", "merkle", "merklebranch", "branch"):
        if key in proof and isinstance(proof[key], list):
            nodes = proof[key]
            break
    if nodes is None:
        raise AssertionError(f"no branch found in {raw_keys}")
    for key in ("index", "pos", "txpos", "position"):
        if key in proof:
            return list(nodes), int(proof[key]), raw_keys
    raise AssertionError(f"no index found in {raw_keys}")


def interpret_nodes(raw_nodes: list, txid: str, index: int, root_internal: bytes):
    """Determine how the node encodes its branch, instead of assuming.

    The first live run settled that it is not the naive reading: the node's
    branch did not fold to the block root when taken at face value. So try the
    plausible encodings, use the one that folds, and say which it was — that
    turns a guess into a measurement, and the fixture records it.
    """
    attempts = []
    for label, reverse_bytes, reverse_order in NODE_ENCODINGS:
        nodes = []
        for n in raw_nodes:
            if n == "*":
                nodes.append(DUPLICATE)
                continue
            b = bytes.fromhex(n) if isinstance(n, str) else bytes(n)
            nodes.append(b[::-1] if reverse_bytes else b)
        if reverse_order:
            nodes = nodes[::-1]
        folded = fold_nodes(txid, index, nodes)
        attempts.append((label, folded))
        if folded == root_internal:
            return nodes, label
    detail = "; ".join(f"{lbl} -> {f.hex()[:16]}" for lbl, f in attempts)
    raise AssertionError(
        f"the node's branch does not fold to the block root under any known "
        f"encoding. root={root_internal.hex()[:16]}; {detail}")


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

        # Put a second transaction in the same block, BEFORE the deposit, so the
        # deposit sits at index 2 of a 3-leaf tree. That is what makes the TSC
        # proof emit its "*" sentinel — the odd-level duplication case. Without
        # it the sentinel path never runs offline, and the live node is free to
        # produce it the first time it matters.
        extra_src = mature[1]
        extra_tx = B.new_tx()
        B.add_input(extra_tx, B.le(extra_src["txid"]), extra_src["vout"])
        B.add_output(extra_tx, extra_src["value"] - 1000, miner_script)
        extra_tx["vin"][0]["script"] = B.sign_input(
            extra_tx, 0, priv, extra_src["value"], extra_src["script"])
        extra_raw = B.serialise_tx(extra_tx)

        chain.mine_block([extra_raw, deposit_raw], coinbase_script=miner_script)
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

    proof = rpc.getmerkleproof2(block_hash, deposit_txid)
    if isinstance(rpc, NodeRPC):
        os.makedirs(FIXTURE_DIR, exist_ok=True)
        path = os.path.join(FIXTURE_DIR, "node_merkleproof_raw.json")
        with open(path, "w") as f:
            json.dump(proof, f, indent=2, sort_keys=True)
            f.write("\n")
        log(f"     raw getmerkleproof2 response recorded to {path}")

    raw_nodes, node_index, raw_keys = extract_nodes(proof)
    log(f"     node returned keys: {raw_keys}")
    log(f"     node nodes, raw:  {raw_nodes}")
    ours_branch = B.merkle_branch(txids, index)
    root_internal = B.le(block["merkleroot"])

    # Determine the encoding rather than assume it. The first live run proved
    # the naive reading is wrong: taken at face value the node's branch did not
    # fold to the block root.
    node_nodes, encoding = interpret_nodes(raw_nodes, deposit_txid, node_index,
                                           root_internal)
    log(f"     encoding determined: {encoding}")
    if any(n is DUPLICATE for n in node_nodes):
        log(f"     node used the '*' sentinel "
            f"{sum(1 for n in node_nodes if n is DUPLICATE)} time(s) — "
            f"odd-level duplication, now materialised")

    # Resolve any "*" into concrete hashes: that is the form verify_deposit()
    # takes, and the form the on-chain verifier folds.
    node_branch_concrete = materialise_branch(deposit_txid, node_index, node_nodes)

    check("index convention agrees", node_index == index,
          f"node {node_index} vs ours {index}")
    check("branch length agrees", len(node_nodes) == len(ours_branch),
          f"node {len(node_nodes)} vs ours {len(ours_branch)}")
    check("the materialised node branch equals ours byte for byte",
          node_branch_concrete == ours_branch,
          f"{[x.hex()[:8] for x in node_branch_concrete]} vs "
          f"{[x.hex()[:8] for x in ours_branch]}")
    check("the node's branch folds to the block root",
          fold_nodes(deposit_txid, node_index, node_nodes) == root_internal)
    check("our branch folds to the block root",
          B.fold_branch(deposit_txid, index, ours_branch) == root_internal)

    # `target` identifies the block the proof is against. Its type is NOT what
    # the help text implies: the live node returned a string (and no `flags`
    # field at all), so handle what it can actually be and say what we saw.
    target = proof.get("target")
    if isinstance(target, dict):
        check("the proof's target header hashes to the block we asked about",
              B.block_hash(target) == block_hash,
              f"{B.block_hash(target)[:16]} vs {block_hash[:16]}")
    elif isinstance(target, str) and len(target) == 64:
        check("the proof's target block hash is the block we asked about",
              target.lower() == block_hash.lower()
              or target.lower()[::-1] == block_hash.lower(),
              f"{target[:16]} vs {block_hash[:16]}")
    elif isinstance(target, str) and len(target) == 160:
        raw = header_raw.lower() if isinstance(header_raw, str) else None
        check("the proof's target is the raw header of that block",
              raw is not None and target.lower() in (raw, raw[::-1]),
              f"{target[:16]}... vs {str(raw)[:16]}...")
    else:
        log(f"     target not interpreted: {type(target).__name__} = "
            f"{str(target)[:60]}")
    check("the proof's txOrId is our transaction",
          proof.get("txOrId", deposit_txid) == deposit_txid)

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
        "branch": node_branch_concrete,
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
        print("all good — the pin harness agrees with itself. The response shape")
        print("is taken from the SV Node v1.1.1 source (src/rpc/rawtransaction.cpp);")
        print("only a live call confirms it in practice. Run without --selftest.")
    else:
        print("all good — our byte formats agree with SV Node")
    return 0


if __name__ == "__main__":
    sys.exit(main())
