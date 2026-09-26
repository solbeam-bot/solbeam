#!/usr/bin/env python3
"""
SOLBEAM — adversarial play runner.

For a human who wants to be the man in the middle. Each "play" is an attack,
attempted against a fresh synthetic chain, with the expected rejection stated up
front so you can judge the system rather than take its word for it.

    python3 attack.py --list              # every play, what it tries, its status
    python3 attack.py tamper-branch       # run one
    python3 attack.py --all               # run everything runnable now

Plays marked "Phase 3" need the relayer and bond machinery, which does not exist
yet. They are listed anyway, so the whole threat model is visible in one place
rather than being discovered as we go.

Nothing here touches a live chain. Every play runs in-process against a
generated chain and takes under a second.

The full script, including what each play means and the adversary roles a person
can take, is in ADVERSARY_PLAYBOOK.md.
"""

import os
import sys
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
CHECKS = os.path.normpath(os.path.join(HERE, "..", "checks"))
sys.path.insert(0, CHECKS)

import bsvlib as B                      # noqa: E402
import bsvchain as C                    # noqa: E402
from check_bsv_pegin import (           # noqa: E402
    DepositRejected, MintRegistry, build_deposit, make_proof, verify_deposit,
    serialise_instruction, DEPOSIT_VALUE, FEE, RECIPIENT, p2pkh_script_for,
)

MINER_PRIV = int.from_bytes(bytes.fromhex("22" * 32), "big")
MINER_SCRIPT = p2pkh_script_for(MINER_PRIV)


# ---------------------------------------------------------------------------
# a fresh world per play, so no play can be affected by another
# ---------------------------------------------------------------------------

class World:
    def __init__(self, confirmations=C.CONFIRMATIONS_REQUIRED):
        self.chain = C.Chain()
        self.chain.mine_empty(115, coinbase_script=MINER_SCRIPT)
        self.utxos = [u for u in self.chain.mature_coinbases()
                      if u["script"] == MINER_SCRIPT]
        self.deposit_raw, self.deposit_txid, self.deposit_block = self._deposit(self.utxos[0])
        self.chain.mine_empty(confirmations - 1, coinbase_script=MINER_SCRIPT)
        self.registry = MintRegistry()

    def _deposit(self, utxo, recipient=RECIPIENT, **kw):
        tx = build_deposit(utxo, recipient, priv=MINER_PRIV, **kw)
        raw = B.serialise_tx(tx)
        blk = self.chain.mine_block([raw], coinbase_script=MINER_SCRIPT)
        return raw, B.txid_of(raw), blk

    @property
    def view(self):
        return {
            "tip_height": self.chain.height,
            "checkpoint": {"height": 0, "hash": self.chain.at(0)["hash"]},
            "headers": self.chain.headers(0, self.chain.height),
        }

    @property
    def proof(self):
        return make_proof(self.chain, self.deposit_raw, self.deposit_txid, 0,
                          DEPOSIT_VALUE, RECIPIENT)


# ---------------------------------------------------------------------------
# the plays
# ---------------------------------------------------------------------------

def p_honest(w):
    """Control: the same proof, untampered. Proves the harness can succeed."""
    ins = verify_deposit(w.proof, w.view)
    return f"ACCEPTED — {len(serialise_instruction(ins))}-byte mint instruction for " \
           f"{ins['amount']} sats"


def p_no_payload(w):
    """Strip the OP_RETURN. The most likely real-world user error."""
    raw, txid, _ = w._deposit(w.utxos[1], include_payload=False)
    w.chain.mine_empty(C.CONFIRMATIONS_REQUIRED - 1, coinbase_script=MINER_SCRIPT)
    return verify_deposit(make_proof(w.chain, raw, txid, 0, DEPOSIT_VALUE, RECIPIENT), w.view)


def p_truncated_payload(w):
    """A 20-byte payload where 32 is required — a wallet quietly truncating."""
    raw, txid, _ = w._deposit(w.utxos[1], b"\x11" * 20)
    w.chain.mine_empty(C.CONFIRMATIONS_REQUIRED - 1, coinbase_script=MINER_SCRIPT)
    return verify_deposit(make_proof(w.chain, raw, txid, 0, DEPOSIT_VALUE, RECIPIENT), w.view)


def p_wrong_address(w):
    """Pay an address the bridge does not control, payload intact."""
    raw, txid, _ = w._deposit(w.utxos[1], deposit_script=MINER_SCRIPT)
    w.chain.mine_empty(C.CONFIRMATIONS_REQUIRED - 1, coinbase_script=MINER_SCRIPT)
    return verify_deposit(make_proof(w.chain, raw, txid, 0, DEPOSIT_VALUE, RECIPIENT), w.view)


def p_inflate_amount(w):
    """Claim more than the output carries."""
    return verify_deposit(dict(w.proof, amount=DEPOSIT_VALUE * 100), w.view)


def p_tamper_branch(w):
    """Rewrite the Merkle path to a different transaction."""
    bad = dict(w.proof, branch=list(w.proof["branch"]))
    bad["branch"][0] = bytes(32)
    return verify_deposit(bad, w.view)


def p_swap_txid(w):
    """Present a different transaction's bytes under the real txid."""
    corrupt = bytearray(w.deposit_raw)
    corrupt[-6] ^= 0x01
    return verify_deposit(dict(w.proof, tx_raw=bytes(corrupt)), w.view)


def p_coinbase_claim(w):
    """Claim an entire block's coinbase as a deposit."""
    cb = w.chain.at(w.deposit_block["height"])
    return verify_deposit({
        "tx_raw": cb["raw"][0], "height": cb["height"], "txid": cb["txids"][0],
        "vout": 0, "amount": C.COINBASE_SUBSIDY, "recipient": RECIPIENT, "index": 0,
        "branch": B.merkle_branch(cb["txids"], 0),
    }, w.view)


def p_wrong_height(w):
    """Point the proof at a block the verifier does not hold."""
    return verify_deposit(dict(w.proof, height=w.chain.height + 500), w.view)


def p_bad_checkpoint(w):
    """Offer a checkpoint hash that does not match the stored header."""
    view = w.view
    view["checkpoint"] = {"height": 0, "hash": "ab" * 32}
    return verify_deposit(w.proof, view)


def p_early(w):
    """Mint before the confirmation depth is reached."""
    w2 = World(confirmations=2)          # deposit is only 2 blocks deep
    return verify_deposit(w2.proof, w2.view)


def p_replay(w):
    """Submit a proof that has already been minted."""
    w.registry.submit(verify_deposit(w.proof, w.view))
    w.registry.submit(verify_deposit(w.proof, w.view))


def p_orphan(w):
    """Get a deposit minted, then reorg the chain out from under it."""
    proof = w.proof
    w.chain.invalidate_from(w.deposit_block["height"])
    w.chain.mine_empty(C.CONFIRMATIONS_REQUIRED, coinbase_script=MINER_SCRIPT)
    view = {"tip_height": w.chain.height,
            "checkpoint": {"height": 0, "hash": w.chain.at(0)["hash"]},
            "headers": w.chain.headers(0, w.chain.height)}
    return verify_deposit(proof, view)


# name -> (phase, what you are trying to do, expected outcome, fn)
PLAYS = {
    "honest":         ("1a", "Control: submit an untampered proof",
                       "ACCEPTED", p_honest),
    "no-payload":     ("1a", "Strip the OP_RETURN — the likely user error",
                       "MISSING_PAYLOAD", p_no_payload),
    "truncated-payload": ("1a", "Send a 20-byte payload where 32 is required",
                          "MISSING_PAYLOAD", p_truncated_payload),
    "wrong-address":  ("1a", "Pay an address the bridge does not control",
                       "WRONG_OUTPUT_SCRIPT", p_wrong_address),
    "inflate-amount": ("1a", "Claim 100x more than the output carries",
                       "AMOUNT_MISMATCH", p_inflate_amount),
    "tamper-branch":  ("1a", "Rewrite the Merkle path",
                       "BAD_MERKLE_PROOF", p_tamper_branch),
    "swap-txid":      ("1a", "Present different bytes under the real txid",
                       "TXID_MISMATCH", p_swap_txid),
    "coinbase-claim": ("1a", "Claim a whole block's coinbase",
                       "COINBASE_DEPOSIT", p_coinbase_claim),
    "wrong-height":   ("1a", "Point at a block the verifier does not hold",
                       "UNKNOWN_HEADER", p_wrong_height),
    "bad-checkpoint": ("1a", "Offer a checkpoint that does not match",
                       "BAD_CHECKPOINT", p_bad_checkpoint),
    "early":          ("1a", "Mint before the confirmation depth",
                       "INSUFFICIENT_CONFIRMATIONS", p_early),
    "replay":         ("1a", "Mint the same deposit twice",
                       "ALREADY_MINTED", p_replay),
    "orphan":         ("1a", "Reorg the deposit out after it is mintable",
                       "BAD_MERKLE_PROOF", p_orphan),
}

# Not runnable yet. Listed so the whole threat model is visible in one place.
PENDING = {
    "swap-deposit-address": ("1a", "MITM: hand the user an address you control",
                             "NOT PREVENTABLE by the protocol — see the playbook"),
    "swap-recipient":       ("1a", "MITM: change the OP_RETURN recipient",
                             "NOT PREVENTABLE by the protocol — see the playbook"),
    "hostile-advancer":     ("2", "Feed fabricated, out-of-order or low-work headers",
                             "rejected; liveness only"),
    "stall-advancer":       ("2", "Stop advancing the header chain",
                             "no funds at risk; a second advancer recovers"),
    "rogue-earmarked":      ("3", "Take the float with a redemption outstanding",
                             "deadline refunds the holder, bond slashed"),
    "rogue-naked":          ("3", "Take an idle float with no redemption",
                             "bounded by the cap; slashed only if challenged"),
    "rogue-vanish":         ("3", "Accept a job, then disappear",
                             "holder re-minted, bond slashed"),
    "rogue-underpay":       ("3", "Pay less than owed",
                             "proof cannot match; deadline refund"),
    "rogue-refuse-unbond":  ("3", "Exit mid-commitment to dodge a slash",
                             "blocked by the unbonding period"),
    "rogue-self-deal":      ("3", "Burn your own, pay yourself, claim the refund",
                             "exactly one terminal state; no double-claim"),
    "pump-the-bond":        ("3", "Profit from a BSV pump by forfeiting the bond",
                             "impossible: the bond is solBSV, so the ratio is invariant"),
    "reorg-payout":         ("3", "Reorg a payout after it settled",
                             "the settlement window catches it"),
}


# ---------------------------------------------------------------------------

def run(name, verbose=False):
    phase, what, expected, fn = PLAYS[name]
    print(f"\n=== {name}  (Phase {phase})")
    print(f"    you are trying to: {what}")
    print(f"    expected:          {expected}")
    try:
        world = World()
        result = fn(world)
        observed = ("ACCEPTED" if expected == "ACCEPTED"
                    else f"no rejection raised: {result}")
        ok = expected == "ACCEPTED"
    except DepositRejected as e:
        observed = e.code
        ok = (e.code == expected)
    except Exception as e:  # noqa: BLE001
        observed = f"{type(e).__name__}: {e}"
        ok = False
        if verbose:
            traceback.print_exc()
    print(f"    observed:          {observed}")
    print(f"    verdict:           {'as expected' if ok else 'NOT as expected'}")
    return ok


def main() -> int:
    args = sys.argv[1:]
    verbose = "--verbose" in args
    args = [a for a in args if a != "--verbose"]

    if not args or "--list" in args:
        print("SOLBEAM — adversarial plays\n")
        print("  runnable now:")
        for name, (phase, what, expected, _) in PLAYS.items():
            print(f"    {name:<20} {expected:<28} {what}")
        print("\n  need a later phase:")
        for name, (phase, what, expected) in PENDING.items():
            print(f"    {name:<20} Phase {phase}  {what}")
            print(f"    {'':<20} -> {expected}")
        print("\n  python3 attack.py --all        run everything runnable")
        print("  python3 attack.py <name>       run one\n")
        return 0

    names = list(PLAYS) if "--all" in args else [a for a in args if a in PLAYS]
    unknown = [a for a in args if a not in PLAYS and a != "--all"]
    if unknown:
        print(f"unknown play(s): {', '.join(unknown)}")
        print("run with --list to see them")
        return 2

    results = [run(n, verbose) for n in names]
    good = sum(results)
    print(f"\n{good}/{len(results)} plays behaved as the design claims")
    if good != len(results):
        print("A play that does not behave as documented is a FINDING, not a test")
        print("failure. Record it in ADVERSARY_PLAYBOOK.md's results table.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
