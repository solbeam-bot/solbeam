#!/usr/bin/env python3
"""
SOLBEAM — Phase 1A: peg-in on the BSV side, against a synthetic regtest chain.

Proves that a BSV deposit yields a **portable mint instruction** that anyone can
re-verify without trusting the producer. No Solana, no BSV node — this is the
half of the peg that can be proven with nothing installed.

The artefact is `fixtures/deposit_1.json`: the exact bytes Phase 2 will submit
to the Solana program. Phase 1B re-runs these identical cases against a real SV
Node and asserts the instruction is byte-identical.

Trust shape, which the code is careful to respect:
  * the **verifier** owns the headers (`view`) — they come from the advancer and
    are checked for PoW and linkage;
  * the **producer** supplies the proof (`proof`) — the raw transaction, the
    Merkle branch, the claimed output — and none of it is believed.

Usage:  python3 check_bsv_pegin.py
"""

import json
import os
import sys

import bsvlib as B
import bsvchain as C

MIN_CONFIRMATIONS = C.CONFIRMATIONS_REQUIRED      # 12
DEPOSIT_VALUE = 100_000_000                       # 1 BSV in satoshis
FEE = 1_000

# Fixed keys, so the fixture is reproducible byte for byte.
BRIDGE_PRIV = 0x5E11B0A200000000000000000000000000000000000000000000000000000001
USER_PRIV = 0x5E11B0A200000000000000000000000000000000000000000000000000000002
FAUCET_PRIV = 0x5E11B0A200000000000000000000000000000000000000000000000000000003

RECIPIENT = bytes.fromhex("11" * 32)              # the user's Solana pubkey
R2 = bytes.fromhex("33" * 32)
R3 = bytes.fromhex("44" * 32)
R_ORPHAN = bytes.fromhex("55" * 32)

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURE_PATH = os.path.normpath(os.path.join(HERE, "..", "fixtures", "deposit_1.json"))


def p2pkh_script_for(priv: int) -> bytes:
    return B.p2pkh_script(B.hash160(B.pubkey_from_priv(priv)))


DEPOSIT_SCRIPT = p2pkh_script_for(BRIDGE_PRIV)
USER_SCRIPT = p2pkh_script_for(USER_PRIV)
FAUCET_SCRIPT = p2pkh_script_for(FAUCET_PRIV)


# ---------------------------------------------------------------------------
# rejection codes — distinct and named, because "invalid deposit" helps nobody
# ---------------------------------------------------------------------------

class DepositRejected(Exception):
    def __init__(self, code: str, detail: str = ""):
        super().__init__(f"{code}: {detail}" if detail else code)
        self.code = code


class MintRegistry:
    """The used-(txid, vout) set a bridge must keep to stop replays."""

    def __init__(self):
        self.seen = set()

    def submit(self, instruction: dict) -> None:
        key = (instruction["txid"], instruction["vout"])
        if key in self.seen:
            raise DepositRejected("ALREADY_MINTED", f"{key[0]}:{key[1]} was already minted")
        self.seen.add(key)


# ---------------------------------------------------------------------------
# building
# ---------------------------------------------------------------------------

def build_deposit(utxo, recipient, amount=DEPOSIT_VALUE, fee=FEE,
                  deposit_script=DEPOSIT_SCRIPT, change_script=USER_SCRIPT,
                  include_payload=True, priv=USER_PRIV):
    """The user's payment: deposit output, OP_RETURN payload, change."""
    tx = B.new_tx()
    B.add_input(tx, B.le(utxo["txid"]), utxo["vout"])
    B.add_output(tx, amount, deposit_script)
    if include_payload:
        B.add_output(tx, 0, B.op_return_script(recipient))
    B.add_output(tx, utxo["value"] - amount - fee, change_script)
    tx["vin"][0]["script"] = B.sign_input(tx, 0, priv, utxo["value"], utxo["script"])
    return tx


def make_proof(chain, tx_raw: bytes, txid: str, vout: int, amount: int, recipient: bytes) -> dict:
    """Everything the producer supplies. None of it is trusted."""
    found = chain.find_tx(txid)
    if not found:
        raise ValueError("tx not in chain")
    block, index = found
    return {
        "tx_raw": tx_raw,
        "height": block["height"],
        "txid": txid,
        "vout": vout,
        "amount": amount,
        "recipient": recipient,
        "index": index,
        "branch": B.merkle_branch(block["txids"], index),
    }


# ---------------------------------------------------------------------------
# verifying — the whole point: anyone can run this and get the same answer
# ---------------------------------------------------------------------------

def verify_deposit(proof: dict, view: dict, deposit_script: bytes = DEPOSIT_SCRIPT,
                   min_conf: int = MIN_CONFIRMATIONS) -> dict:
    """Raise DepositRejected(code), or return the mint instruction.

    `view` is what the verifier owns — the light client's stored headers:
        {"tip_height": int, "checkpoint": {"height", "hash"}, "headers": {h: hdr}}
    """
    height = proof["height"]
    headers = view["headers"]

    # 1. the header must be one the verifier already holds
    if height not in headers:
        raise DepositRejected("UNKNOWN_HEADER", f"no header at height {height}")
    header = headers[height]

    # 2. it must be real work
    if B.hash_as_int(header) > B.target_from_bits(header["bits"]):
        raise DepositRejected("BAD_POW", f"header at {height} misses its target")

    # 3. the chain must link back to the trusted checkpoint
    cp = view["checkpoint"]
    if cp["height"] not in headers:
        raise DepositRejected("BAD_CHECKPOINT", "checkpoint header absent")
    if B.block_hash(headers[cp["height"]]) != cp["hash"]:
        raise DepositRejected("BAD_CHECKPOINT", "checkpoint hash mismatch")
    for h in range(cp["height"] + 1, height + 1):
        if h not in headers:
            raise DepositRejected("BROKEN_LINKAGE", f"header {h} is missing from the window")
        if headers[h]["previousblockhash"] != B.block_hash(headers[h - 1]):
            raise DepositRejected("BROKEN_LINKAGE", f"header {h} does not link to {h - 1}")

    # 4. the transaction must be in that block
    try:
        tx = B.parse_tx(proof["tx_raw"])
    except Exception as e:  # noqa: BLE001 - any malformed encoding is a rejection
        raise DepositRejected("MALFORMED_TX", str(e)) from e
    if B.txid_of(proof["tx_raw"]) != proof["txid"]:
        raise DepositRejected("TXID_MISMATCH", "raw tx does not hash to the claimed txid")
    if B.fold_branch(proof["txid"], proof["index"], proof["branch"]) != B.le(header["merkleroot"]):
        raise DepositRejected("BAD_MERKLE_PROOF", "branch does not fold to the block root")

    # 5. it must not be a coinbase
    if len(tx["vin"]) == 1 and tx["vin"][0]["txid"] == bytes(32) \
            and tx["vin"][0]["vout"] == 0xFFFFFFFF:
        raise DepositRejected("COINBASE_DEPOSIT", "coinbase outputs are not deposits")

    # 6. the output must be exactly what is claimed
    if proof["vout"] >= len(tx["vout"]):
        raise DepositRejected("NO_SUCH_OUTPUT", f"vout {proof['vout']} does not exist")
    out = tx["vout"][proof["vout"]]
    if out["value"] != proof["amount"]:
        raise DepositRejected("AMOUNT_MISMATCH",
                              f"claimed {proof['amount']}, actual {out['value']}")
    if out["value"] <= 0:
        raise DepositRejected("ZERO_VALUE", "deposit output carries no value")
    if out["script"] != deposit_script:
        raise DepositRejected("WRONG_OUTPUT_SCRIPT", "output does not pay the deposit address")

    # 7. the recipient must be committed somewhere in the same transaction
    if not any(B.parse_op_return(o["script"]) == proof["recipient"] for o in tx["vout"]):
        raise DepositRejected("MISSING_PAYLOAD",
                              "no OP_RETURN carrying this recipient — did the wallet attach it?")

    # 8. buried deep enough
    confirmations = view["tip_height"] - height + 1
    if confirmations < min_conf:
        raise DepositRejected("INSUFFICIENT_CONFIRMATIONS", f"{confirmations} of {min_conf}")

    return {
        "version": 1,
        "height": height,
        "header": B.header_bytes(header),
        "txid": proof["txid"],
        "vout": proof["vout"],
        "amount": proof["amount"],
        "recipient": proof["recipient"],
        "branch": proof["branch"],
        "index": proof["index"],
        "confirmations": confirmations,
    }


def serialise_instruction(ins: dict) -> bytes:
    """The exact bytes Phase 2 submits. Phase 1 owns this layout."""
    raw = bytes([ins["version"]])
    raw += ins["height"].to_bytes(4, "little")
    raw += ins["header"]                                   # 80
    raw += B.le(ins["txid"])                               # 32
    raw += ins["vout"].to_bytes(4, "little")
    raw += ins["amount"].to_bytes(8, "little")             # satoshis == solBSV base units
    raw += ins["recipient"]                                # 32
    raw += bytes([len(ins["branch"])])
    raw += b"".join(ins["branch"])
    raw += ins["index"].to_bytes(4, "little")
    raw += ins["confirmations"].to_bytes(4, "little")
    return raw


# ---------------------------------------------------------------------------

def main() -> int:
    checks, failures = 0, []

    def check(name, ok, detail=""):
        nonlocal checks
        checks += 1
        print(f"  {'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f"  {detail}"))
        if not ok:
            failures.append(name)

    def expect_reject(name, code, fn):
        try:
            fn()
        except DepositRejected as e:
            check(name, e.code == code, f"expected {code}, got {e.code}")
        else:
            check(name, False, f"expected {code}, nothing raised")

    print("SOLBEAM / Phase 1A — peg-in on BSV alone\n")

    # -- chain and funding -------------------------------------------------
    print("[1] chain, maturity, and the deposit transaction")
    chain = C.Chain()
    chain.mine_empty(115, coinbase_script=FAUCET_SCRIPT)
    mature = chain.mature_coinbases()
    faucet_mature = [u for u in mature if u["script"] == FAUCET_SCRIPT]
    check("chain mines at the regtest target", chain.height == 115, f"tip {chain.height}")
    check("coinbases become spendable after 100 confirmations", len(faucet_mature) == 15,
          f"{len(faucet_mature)} mature")
    check("an immature coinbase is not spendable",
          not any(u["height"] == 110 for u in mature))
    check("the genesis coinbase is unspendable by construction",
          not any(u["script"] == FAUCET_SCRIPT for u in mature if u["height"] == 0))

    # One faucet transaction with eight outputs: eight independent user UTXOs, so
    # no two deposits below ever double-spend the same outpoint.
    faucet = B.new_tx()
    for u in faucet_mature[:8]:
        B.add_input(faucet, B.le(u["txid"]), u["vout"])
    for _ in range(8):
        B.add_output(faucet, 8 * 100_000_000, USER_SCRIPT)
    for i, u in enumerate(faucet_mature[:8]):
        faucet["vin"][i]["script"] = B.sign_input(faucet, i, FAUCET_PRIV, u["value"], u["script"])
    faucet_txid = B.txid_of(B.serialise_tx(faucet))
    chain.mine_block([B.serialise_tx(faucet)], coinbase_script=FAUCET_SCRIPT)
    user_utxos = [{"txid": faucet_txid, "vout": i, "value": 8 * 100_000_000,
                   "script": USER_SCRIPT} for i in range(8)]
    check("the funding transaction signs every input",
          all(B.parse_pushes(v["script"]) for v in faucet["vin"]))

    # The deposit under test.
    deposit_tx = build_deposit(user_utxos[0], RECIPIENT)
    deposit_raw = B.serialise_tx(deposit_tx)
    deposit_txid = B.txid_of(deposit_raw)
    deposit_block = chain.mine_block([deposit_raw], coinbase_script=FAUCET_SCRIPT)

    pushes = B.parse_pushes(deposit_tx["vin"][0]["script"])
    sig, pub = pushes[0][:-1], pushes[1]
    digest = B.sighash_forkid(deposit_tx, 0, user_utxos[0]["value"], user_utxos[0]["script"])
    r, s = B.der_decode(sig)
    check("deposit tx is signed by the user and the signature verifies",
          B.ecdsa_verify(B.parse_pubkey(pub), int.from_bytes(digest, "big"), r, s))
    check("deposit tx carries the recipient in an OP_RETURN",
          B.parse_op_return(deposit_tx["vout"][1]["script"]) == RECIPIENT)
    check("deposit output pays the bridge deposit address",
          deposit_tx["vout"][0]["script"] == DEPOSIT_SCRIPT)
    check("deposit output carries 1 BSV", deposit_tx["vout"][0]["value"] == DEPOSIT_VALUE)

    def view_for(tip=None, checkpoint_height=0, txs=()):
        v = {
            "tip_height": chain.height if tip is None else tip,
            "checkpoint": {"height": checkpoint_height,
                           "hash": chain.at(checkpoint_height)["hash"]},
            "headers": chain.headers(0, chain.height),
        }
        return v

    def proof_for(tx_raw, recipient, vout=0, amount=DEPOSIT_VALUE):
        return make_proof(chain, tx_raw, B.txid_of(tx_raw), vout, amount, recipient)

    # -- confirmations -----------------------------------------------------
    print("[2] confirmation depth")
    expect_reject("1 confirmation is rejected", "INSUFFICIENT_CONFIRMATIONS",
                  lambda: verify_deposit(proof_for(deposit_raw, RECIPIENT), view_for()))

    chain.mine_empty(11, coinbase_script=FAUCET_SCRIPT)
    proof = proof_for(deposit_raw, RECIPIENT)
    view = view_for()

    expect_reject("11 confirmations is rejected", "INSUFFICIENT_CONFIRMATIONS",
                  lambda: verify_deposit(proof, view_for(tip=deposit_block["height"] + 10)))

    instruction = verify_deposit(proof, view)
    check(f"{MIN_CONFIRMATIONS} confirmations is accepted",
          instruction["confirmations"] == MIN_CONFIRMATIONS,
          f"got {instruction['confirmations']}")

    # -- the instruction ---------------------------------------------------
    print("[3] the mint instruction")
    raw_instruction = serialise_instruction(instruction)
    expected_len = 1 + 4 + 80 + 32 + 4 + 8 + 32 + 1 + 32 * len(instruction["branch"]) + 4 + 4
    check("instruction serialises to the expected length",
          len(raw_instruction) == expected_len,
          f"{len(raw_instruction)} != {expected_len}")
    check("instruction carries the 80-byte block header", len(instruction["header"]) == 80)
    check("instruction amount equals the deposit, in satoshis",
          instruction["amount"] == DEPOSIT_VALUE)
    check("instruction recipient is the user's Solana key",
          instruction["recipient"] == RECIPIENT)
    check("instruction is deterministic",
          serialise_instruction(verify_deposit(proof, view)) == raw_instruction)

    # -- tampering ---------------------------------------------------------
    print("[4] tampered proofs")
    bad = dict(proof, branch=list(proof["branch"]))
    bad["branch"][0] = bytes(32)
    expect_reject("tampered Merkle branch", "BAD_MERKLE_PROOF",
                  lambda: verify_deposit(bad, view))

    expect_reject("inflated amount", "AMOUNT_MISMATCH",
                  lambda: verify_deposit(dict(proof, amount=DEPOSIT_VALUE + 1), view))

    expect_reject("output index that does not exist", "NO_SUCH_OUTPUT",
                  lambda: verify_deposit(dict(proof, vout=99), view))

    expect_reject("proof for a block not in the window", "UNKNOWN_HEADER",
                  lambda: verify_deposit(dict(proof, height=deposit_block["height"] + 10_000), view))

    expect_reject("payload naming a different recipient", "MISSING_PAYLOAD",
                  lambda: verify_deposit(dict(proof, recipient=R2), view))

    corrupt = bytearray(deposit_raw)
    corrupt[-6] ^= 0x01                      # inside the change output's value
    expect_reject("raw tx that does not hash to the claimed txid", "TXID_MISMATCH",
                  lambda: verify_deposit(dict(proof, tx_raw=bytes(corrupt)), view))

    alt_cp = view_for(checkpoint_height=deposit_block["height"] - 1)
    alt_cp["checkpoint"] = {"height": deposit_block["height"] - 1, "hash": "ab" * 32}
    expect_reject("checkpoint that does not match the stored header", "BAD_CHECKPOINT",
                  lambda: verify_deposit(proof, alt_cp))

    holed = view_for()
    holed["headers"] = {k: v for k, v in holed["headers"].items() if k != deposit_block["height"] - 1}
    expect_reject("a hole in the header chain", "BROKEN_LINKAGE",
                  lambda: verify_deposit(proof, holed))

    # -- deposits a wallet can get wrong ----------------------------------
    print("[5] deposits a wallet can get wrong")
    cases = [
        ("deposit with no OP_RETURN — the likely user error", "MISSING_PAYLOAD",
         build_deposit(user_utxos[1], RECIPIENT, include_payload=False), RECIPIENT),
        ("truncated payload (20 bytes, not 32)", "MISSING_PAYLOAD",
         build_deposit(user_utxos[2], b"\x11" * 20), RECIPIENT),
        ("output paying the wrong address", "WRONG_OUTPUT_SCRIPT",
         build_deposit(user_utxos[3], RECIPIENT, deposit_script=USER_SCRIPT), RECIPIENT),
    ]
    for name, code, tx, recip in cases:
        raw = B.serialise_tx(tx)
        chain.mine_block([raw], coinbase_script=FAUCET_SCRIPT)
        chain.mine_empty(11, coinbase_script=FAUCET_SCRIPT)
        expect_reject(name, code, lambda raw=raw, recip=recip:
                      verify_deposit(proof_for(raw, recip), view_for()))

    cb = chain.at(deposit_block["height"])
    cb_proof = {"tx_raw": cb["raw"][0], "height": deposit_block["height"],
                "txid": cb["txids"][0], "vout": 0, "amount": C.COINBASE_SUBSIDY,
                "recipient": RECIPIENT, "index": 0,
                "branch": B.merkle_branch(cb["txids"], 0)}
    expect_reject("coinbase output claimed as a deposit", "COINBASE_DEPOSIT",
                  lambda: verify_deposit(cb_proof, view_for()))

    # -- replay ------------------------------------------------------------
    print("[6] replay protection")
    registry = MintRegistry()
    registry.submit(instruction)
    check("first submission is accepted", (instruction["txid"], instruction["vout"]) in registry.seen)
    expect_reject("the same (txid, vout) cannot be minted twice", "ALREADY_MINTED",
                  lambda: registry.submit(verify_deposit(proof, view)))

    # -- several deposits in one block (odd Merkle level) -----------------
    print("[7] multiple deposits in one block")
    d2 = build_deposit(user_utxos[4], R2)
    d3 = build_deposit(user_utxos[5], R3)
    d2_raw, d3_raw = B.serialise_tx(d2), B.serialise_tx(d3)
    block = chain.mine_block([d2_raw, d3_raw], coinbase_script=FAUCET_SCRIPT)
    chain.mine_empty(11, coinbase_script=FAUCET_SCRIPT)
    check("the block has an odd transaction count (duplication exercised)",
          len(block["txids"]) == 3, f"{len(block['txids'])}")
    outs = [verify_deposit(proof_for(d2_raw, R2), view_for()),
            verify_deposit(proof_for(d3_raw, R3), view_for())]
    check("both deposits yield their own instruction", len(outs) == 2)
    check("the two instructions differ",
          serialise_instruction(outs[0]) != serialise_instruction(outs[1]))
    check("each instruction names its own recipient",
          {o["recipient"] for o in outs} == {R2, R3})
    check("each instruction is separately replay-protected",
          len({(o["txid"], o["vout"]) for o in outs}) == 2)

    # -- reorg -------------------------------------------------------------
    print("[8] reorg")
    orphan_raw = B.serialise_tx(build_deposit(user_utxos[6], R_ORPHAN))
    orphan_block = chain.mine_block([orphan_raw], coinbase_script=FAUCET_SCRIPT)
    chain.mine_empty(11, coinbase_script=FAUCET_SCRIPT)
    orphan_proof = proof_for(orphan_raw, R_ORPHAN)
    check("deposit is mintable on the branch that holds it",
          verify_deposit(orphan_proof, view_for())["confirmations"] == MIN_CONFIRMATIONS)

    orphaned = chain.invalidate_from(orphan_block["height"])
    chain.mine_empty(12, coinbase_script=FAUCET_SCRIPT)
    check("the chain reorged away from the deposit",
          chain.at(orphan_block["height"])["hash"] != orphan_block["hash"]
          and len(orphaned) == 12)
    check("a checkpoint before the fork is still valid after the reorg",
          B.block_hash(chain.at(0)["header"]) == chain.at(0)["hash"])
    expect_reject("a deposit from the orphaned branch is not mintable", "BAD_MERKLE_PROOF",
                  lambda: verify_deposit(orphan_proof, view_for()))

    # -- the fixture -------------------------------------------------------
    print("[9] the fixture Phase 2 consumes")
    cp_height = deposit_block["height"] - 4
    fixture_view = {
        "tip_height": view["tip_height"],
        "checkpoint": {"height": cp_height, "hash": chain.at(cp_height)["hash"]},
        "headers": chain.headers(cp_height, view["tip_height"]),
    }
    instruction_hex = serialise_instruction(instruction).hex()
    fixture = {
        "comment": "SOLBEAM Phase 1A fixture — the exact bytes Phase 2 must verify on-chain",
        "version": 1,
        "checkpoint": fixture_view["checkpoint"],
        "tip_height": fixture_view["tip_height"],
        "headers": [{"height": h, "raw": B.header_bytes(fixture_view["headers"][h]).hex()}
                    for h in sorted(fixture_view["headers"])],
        "proof": {
            "height": instruction["height"],
            "txid": instruction["txid"],
            "vout": instruction["vout"],
            "amount": instruction["amount"],
            "recipient": instruction["recipient"].hex(),
            "index": instruction["index"],
            "branch": [x.hex() for x in instruction["branch"]],
        },
        "deposit_tx_raw": deposit_raw.hex(),
        "instruction_hex": instruction_hex,
        "instruction_commitment_sha256d": B.sha256d(bytes.fromhex(instruction_hex))[::-1].hex(),
    }
    os.makedirs(os.path.dirname(FIXTURE_PATH), exist_ok=True)
    with open(FIXTURE_PATH, "w") as f:
        json.dump(fixture, f, indent=2)
        f.write("\n")
    check("fixture written", os.path.exists(FIXTURE_PATH), FIXTURE_PATH)

    # The fixture must verify standalone: rebuild a view from the file alone.
    rebuilt_headers = {h["height"]: {
        "version": int.from_bytes(bytes.fromhex(h["raw"])[0:4], "little"),
        "previousblockhash": bytes.fromhex(h["raw"])[4:36][::-1].hex(),
        "merkleroot": bytes.fromhex(h["raw"])[36:68][::-1].hex(),
        "time": int.from_bytes(bytes.fromhex(h["raw"])[68:72], "little"),
        "bits": int.from_bytes(bytes.fromhex(h["raw"])[72:76], "little"),
        "nonce": int.from_bytes(bytes.fromhex(h["raw"])[76:80], "little"),
    } for h in fixture["headers"]}
    rebuilt_view = {"tip_height": fixture["tip_height"],
                    "checkpoint": fixture["checkpoint"],
                    "headers": rebuilt_headers}
    rebuilt_proof = {
        "tx_raw": bytes.fromhex(fixture["deposit_tx_raw"]),
        "height": fixture["proof"]["height"],
        "txid": fixture["proof"]["txid"],
        "vout": fixture["proof"]["vout"],
        "amount": fixture["proof"]["amount"],
        "recipient": bytes.fromhex(fixture["proof"]["recipient"]),
        "index": fixture["proof"]["index"],
        "branch": [bytes.fromhex(x) for x in fixture["proof"]["branch"]],
    }
    rederived = serialise_instruction(verify_deposit(rebuilt_proof, rebuilt_view))
    check("the fixture verifies standalone, from the file alone",
          rederived.hex() == instruction_hex)
    check("the fixture's checkpoint is inside its own header list",
          any(h["height"] == fixture["checkpoint"]["height"] for h in fixture["headers"]))
    check("the deposit block is inside the fixture's header list",
          any(h["height"] == fixture["proof"]["height"] for h in fixture["headers"]))

    # -- time scale --------------------------------------------------------
    print("[10] regtest time scale")
    check("peg-in wait is seconds, not hours", C.MINT_WAIT_SECONDS == 2)
    check("peg-out deadline is seconds, not hours", C.REDEMPTION_DEADLINE_SECONDS == 6)
    check("unbonding outlasts deadline + challenge window",
          C.UNBONDING_SECONDS > C.REDEMPTION_DEADLINE_SECONDS + C.CHALLENGE_WINDOW_SECONDS,
          f"{C.UNBONDING_SECONDS} vs "
          f"{C.REDEMPTION_DEADLINE_SECONDS + C.CHALLENGE_WINDOW_SECONDS}")
    check("every deadline derives from TIMESCALE, not from a literal",
          C.MINT_WAIT_SECONDS == (2 * 3600) // C.TIMESCALE
          and C.REDEMPTION_DEADLINE_SECONDS == (6 * 3600) // C.TIMESCALE
          and C.UNBONDING_SECONDS == (7 * 24 * 3600) // C.TIMESCALE)

    print(f"\n{checks - len(failures)}/{checks} checks passed")
    if failures:
        print("failed: " + ", ".join(failures))
        return 1
    print("all good — a deposit yields a portable mint instruction, verifiable by anyone")
    return 0


if __name__ == "__main__":
    sys.exit(main())
