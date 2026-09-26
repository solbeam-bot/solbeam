# SOLBEAM — adversary playbook

For whoever wants to be the man in the middle. You do not need to read the code to use this.

The point is not to confirm that the tests pass. The point is to **try to break the thing yourself**, and to write down what happened. A play that behaves as documented is a confirmation; a play that does not is a **finding**, which is far more valuable.

---

## 1. Ground rules

1. **Regtest and local validators only.** Every play either runs in-process against a generated chain or against a local regtest node. Nothing here touches a live chain, and nothing here should.
2. **Each play gets a fresh world.** No play can be affected by another, so a finding is reproducible in isolation.
3. **You are allowed to win.** If you manage to make the system part with money or accept a false proof, that is the most useful thing that can happen this week. Record it, do not work around it.
4. **The record is the deliverable.** Expected / observed / verdict, in the table at the end, committed. A green tick with no record is worth nothing.

---

## 2. The roles you can take

| Role | What it can attack | When it exists |
|---|---|---|
| **Proof producer** | The verifier: forge an inclusion proof, inflate an amount, replay a deposit | **Now** |
| **Man in the middle (against the user)** | The address or payload the user is told to use | **Now** — and see §4, where two of these cannot be prevented |
| **Reorg attacker** | The chain itself: undo a deposit after it is mintable | **Now** |
| **Hostile advancer** | The header chain: feed fabricated, reordered or low-work headers | Phase 2 |
| **Rogue relayer** | The float, the bond, the deadline, the unbonding period | Phase 3 |
| **Colluding user + relayer** | The refund path: claim both the payout and the refund | Phase 3 |

---

## 3. Plays you can run right now

```bash
python3 poc/adversary/attack.py --list        # everything, including what is not built yet
python3 poc/adversary/attack.py --all         # run all 13
python3 poc/adversary/attack.py tamper-branch # one play, with the reasoning printed
```

Each play prints what you are trying to do, what the design claims should happen, and what actually happened.

| Play | What you are trying | Expected |
|---|---|---|
| `honest` | Submit an untampered proof — the control | **ACCEPTED** |
| `no-payload` | Strip the `OP_RETURN`. The likely real-world user error | `MISSING_PAYLOAD` |
| `truncated-payload` | A 20-byte payload where 32 is required | `MISSING_PAYLOAD` |
| `wrong-address` | Pay an address the bridge does not control | `WRONG_OUTPUT_SCRIPT` |
| `inflate-amount` | Claim 100× more than the output carries | `AMOUNT_MISMATCH` |
| `tamper-branch` | Rewrite the Merkle path to another transaction | `BAD_MERKLE_PROOF` |
| `swap-txid` | Present different bytes under the real txid | `TXID_MISMATCH` |
| `coinbase-claim` | Claim a whole block's coinbase as a deposit | `COINBASE_DEPOSIT` |
| `wrong-height` | Point at a block the verifier does not hold | `UNKNOWN_HEADER` |
| `bad-checkpoint` | Offer a checkpoint hash that does not match | `BAD_CHECKPOINT` |
| `early` | Mint before the confirmation depth | `INSUFFICIENT_CONFIRMATIONS` |
| `replay` | Mint the same deposit twice | `ALREADY_MINTED` |
| `orphan` | Get a deposit minted, then reorg the chain out from under it | `BAD_MERKLE_PROOF` |

**Start with `orphan`.** It is the most interesting one that already works: it shows the difference between *"the proof was valid"* and *"the proof is still valid"*, which is the whole reason confirmation depth exists.

The `early` play is worth running by hand with the constant changed, to see how the depth parameter behaves rather than trusting that it is enforced.

---

## 4. The plays you can attempt that the protocol **cannot** win

These are in `--list` as `PENDING`, but they are not waiting for code — they are waiting for a *decision*, because they are not protocol problems.

**`swap-deposit-address`.** You are the middleman. You hand the user a deposit address you control instead of the real one. The payload still names the user's Solana address, so:

- the user's funds go to **you**, and
- the user is **still minted** `solBSV`, because the payload is intact and the deposit is valid.

The protocol is behaving correctly. It has no way to know the user was told the wrong address. **This is the single most likely way a real user loses money, and no amount of on-chain verification prevents it.**

**`swap-recipient`.** You take a legitimate deposit and change the `OP_RETURN` recipient to your own Solana address. The mint goes to you. Again the protocol is operating exactly as designed: the payload *is* the instruction, and a deposit is a bearer instrument.

**What follows from this, and what should be built because of it:**

- The deposit address must be **verifiable out of band** — published as a signed, versioned list, with the signature checkable in a wallet or on the site, not just rendered in a page that a middleman controls.
- The UI should show **the payload the user is about to attach**, and the user should be able to check the recipient is their own address. That is a copy-and-compare step the protocol cannot do for them.
- The recovery path for a **payload-less deposit** matters more than it looks: those funds sit at the deposit address with no valid instruction. If the bridge holds that key, they can be returned — the playbook should end with a *tested* return path, not a promise. Right now `no-payload` proves the mint is correctly refused and says nothing about getting the money back. **That is an open gap.**

---

## 5. Plays waiting on Phase 2 — you are the advancer

| Play | What you try | Expected |
|---|---|---|
| `hostile-advancer` | Submit fabricated headers; reorder them; submit duplicates; offer a *valid but lower-work* competing chain | All rejected. The low-work case is the one to watch: linkage alone is not enough, the client must compare accumulated work |
| `stall-advancer` | Simply stop advancing | No funds at risk. Mints stop. A second advancer recovers the tip. This is the honest liveness dependency: the advancer cannot steal, it can only delay |

## 6. Plays waiting on Phase 3 — you are the relayer

This is where the trust model actually gets tested, and where the interesting findings will be.

| Play | What you try | Expected |
|---|---|---|
| `rogue-earmarked` | Take the float while a redemption you accepted is outstanding | Deadline refunds the holder; your bond is slashed. **Self-reporting** — no watcher needed |
| `rogue-naked` | Take an **idle** float with no redemption outstanding | Bounded by the float cap; the bond is seized **only if a challenger submits it**. Run it twice — once challenger active, once not — and confirm the honest residual: bounded but unpunished. This is the play that tests the least comfortable claim in [`docs/04-trust-model.md`](../docs/04-trust-model.md#the-naked-option-attack) |
| `rogue-vanish` | Accept a job, then disappear | Holder re-minted, bond slashed |
| `rogue-underpay` | Pay less than owed | Proof cannot match; deadline refund |
| `rogue-refuse-unbond` | Exit mid-commitment to dodge a slash | Blocked by the unbonding period |
| `rogue-self-deal` | Burn your own `solBSV`, pay yourself, then claim the refund too | Exactly one terminal state; the double-claim must be impossible |
| `pump-the-bond` | Simulate a BSV price move and profit by forfeiting the bond | Impossible: the bond is `solBSV`, so `bond ≥ k × exposure` is unchanged by any price. This play exists specifically to re-test the attack that was found in design review |
| `reorg-payout` | Reorg a payout after it settled | The settlement window catches it; document what happens to the already-closed redemption |

The relayer needs a **`--attack <name>` flag** to make these deterministic rather than hand-driven. That flag is a requirement of Phase 3, and this table is its specification.

---

## 7. The record

Copy this into a new file per session, `poc/adversary/YYYY-MM-DD-results.md`, and commit it.

| Play | Phase | Ran on | Expected | Observed | Verdict | Issue |
|---|---|---|---|---|---|---|
| | | | | | | |

**Verdict is one of:** `as documented` · `FINDING` · `blocked (needs Phase N)`.

For every `FINDING`, open an issue rather than fixing it quietly in the same commit — the point of the record is that the reasoning behind a change is visible later, which is the same reason the test plan and the docs live in this repo rather than on someone's laptop.

---

## 8. What we want out of this

Not coverage. Three things:

1. **A written, dated record** of a human trying to break the peg, with what happened. That is the artefact a reviewer will actually trust, more than any suite of green ticks.
2. **Findings about the boundary**, not the middle — the four plays in §4 are the ones most likely to matter, and they are not code bugs. They are UX and process gaps, and they are only visible when a person plays the middleman.
3. **Evidence that the rejections are specific.** `MISSING_PAYLOAD` tells a user what to do. `invalid deposit` does not. If you ever see that second thing, that is a finding too.

---

Next: [Test plan](TEST_PLAN.md) · [Phase 5 monitoring](PHASE5_MONITORING.md) · [Trust model](../docs/04-trust-model.md)
