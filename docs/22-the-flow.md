# 22. The flow — every actor's journey

Five actors, and what each of them actually does. Written as journeys so that the failure
cases can be read in place rather than gathered in an appendix.

**Status of what is described here:** the light client, the mint and fork staging are **built**.
The vault, the book, bonds and peg-out are **designed, not built**. Instructions are marked
`[built]` or `[designed]` throughout, and no journey below describes running code as though it
were designed or the reverse.

---

## The cast

| Actor | Wants | Is trusted with |
|---|---|---|
| **Depositor** | `solBSV` for BSV | Nothing. Anyone can mint their proof |
| **Holder** | BSV for `solBSV` | Nothing, beyond waiting out a deadline |
| **Relayer** | Fees | **Its own float of BSV.** This is the trust assumption |
| **Advancer** | Nothing | Nothing. It can only submit headers the program checks |
| **Challenger** | The bounty | Nothing. It proves theft or fails |

**The one trust assumption in the system is that a relayer holds BSV it does not
immediately spend.** Everything else is verified. That assumption is not eliminated — it is
**bounded**, by a bond that can be seized on proof, and the proof is on-chain.

---

## The instruction set

**Built:** `initialize`, `push_header`, `verify_deposit`, `init_staging`, `push_fork_header`,
`abandon_staging`, `commit_fork`, `initialize_token`, `initialize_bridge`, `set_checkpoint`,
`set_paused`.

**Designed:** `register_relayer`, `release_mint`, `burn_staged`, `prune`, `request_redeem`,
`accept_redeem`, `settle_redeem`, `cancel_redeem`, `slash`, `stake`, `announce_unbond`,
`withdraw_bond`.

---

## 1 · The Depositor — BSV in, `solBSV` out

```
1  PICK      terms from the book: which relayer, what fee, how many confirmations
2  SEND      BSV to that relayer's REGISTERED script
             OP_RETURN = version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient
3  WAIT      FLOOR = 12 confirmations
4  MINT      anyone calls verify_deposit(header, branch)     [built]
               → verifies PoW (cw-144) and Merkle inclusion
               → mints the NET amount INTO THE VAULT, not to the depositor
               → creates PendingMint{ recipient, relayer, amount,
                                      deposit_hash, deposit_height }
5  MATURE    wait MATURITY_BLOCKS
6  RELEASE   anyone calls release_mint                       [designed]
               requires: tip advanced past the deposit
               requires: hash at that height STILL matches deposit_hash
               → vault to recipient
```

**Nothing here requires the depositor to act again, and nothing requires the relayer's
cooperation.** Steps 4 and 6 are permissionless; the depositor can always do them, and so can
anyone else. That is deliberate — it is what stops a relayer from holding a deposit hostage.

### What can go wrong

| Failure | What happens | Who bears it |
|---|---|---|
| Nobody mints within **32 hours** | The block leaves the window and the proof can never verify again. **The BSV is with the relayer and there are no tokens** | **The depositor.** No on-chain remedy |
| The block is reorged out | `burn_staged` burns the staged token. The BSV came back with the reorg | Nobody — the depositor is where they started |
| The relayer never cooperates | Irrelevant. Steps 4 and 6 are permissionless | — |
| The relayer spends the deposit to an unauthorised destination | Provable within 32 hours; see the Challenger | The relayer's bond |

**The 32-hour deadline is a real cliff and it is not dressed up.** It exists because the
light client can only verify a header that is still in its window. The app mints
automatically, but a depositor who acts entirely alone and waits has no recourse.

---

## 2 · The Holder — `solBSV` in, BSV out

```
1  ESCROW    request_redeem(amount, fee, bsv_destination, deadline)   [designed]
               → solBSV moves into a PegOut account owned by the program
2  ACCEPT    a relayer calls accept_redeem                            [designed]
               → its consent; owed_R increases by amount
               → requires bond_R ≥ k × owed_R afterwards
3  PAY       the relayer broadcasts BSV to bsv_destination
4  SETTLE    settle_redeem(proof)                                     [designed]
               → verifies payout value == amount − fee
               → verifies payout script == bsv_destination
               → verifies the payout's OP_RETURN carries THIS item's id
               → after W confirmations: burns the escrow, pays the fee
   or
4' CANCEL    cancel_redeem — permissionless, after the deadline        [designed]
               → escrow returns to the holder
```

**Failure returns; it never mints.** `cancel_redeem` gives the holder their `solBSV` back,
supply unchanged, **without needing anyone's cooperation**. That is why the bond is not
needed for a failed redemption — and paying both would compensate twice.

### What can go wrong

| Failure | What happens | Who bears it |
|---|---|---|
| The relayer accepts and never pays | After the deadline, `cancel_redeem` returns the escrow | **Nobody.** The holder is whole |
| The relayer pays, but the payout is reorged before settling | It must pay again. If it does not, cancel returns the escrow | The relayer's float |
| The payout settles, **then** is reorged away | **Currently unresolved.** The escrow is burned and the item closed, so neither instruction can run | **The holder.** Needs an unclaim path — see *Open* below |
| Nothing settles and the deadline is far away | The escrow sits. `deadline_slot` must be required to be `>= now + D` | The holder, briefly |

---

## 3 · The Relayer — earns the fee, carries the risk

```
1  REGISTER  register_relayer(script, bond)     [designed]
               → stakes solBSV; publishes the BSV script that deposits must pay
               → registering the script IS the standing consent to be liable for
                 deposits that pay it
2  SERVE     accepts redemptions; pays BSV from its own float
3  CLAIM     settle_redeem pays the fee
4  EXIT      announce_unbond → withdraw_bond after UNBOND_SLOTS     [designed]
               → requires bond_R ≥ k × owed_R still holds
               → so a relayer with outstanding liability CANNOT leave
```

**A relayer's float is its own capital and nobody else's.** It is not a pooled reserve. A key
compromise loses that relayer's funds and no one else's — **except** that its `owed_R` remains
bonded, so holders are still covered.

### What can go wrong

| Failure | What happens | Who bears it |
|---|---|---|
| It spends a deposit to an unauthorised destination | A challenger proves it on-chain within 32 hours; the bond is seized | The relayer |
| It abandons a redemption | The escrow returns to the holder; the relayer loses the fee, not its bond | The relayer, mildly |
| Its key is compromised | The float is gone. `owed_R` is still bonded and still owed | The relayer |
| It wants out while owing | `withdraw_bond` refuses while `bond_R ≥ k × owed_R` fails | — |

---

## 4 · The Advancer — keeps Solana's copy of BSV current

```
per BSV block      push_header(header)                              [built]
                     → verifies linkage, cw-144 difficulty, proof of work
on a reorg         init_staging → push_fork_header → commit_fork    [built]
                     → commits only if strictly HEAVIER (accumulated chainwork)
```

**Unpaid, unstaked, and permissionless — and it cannot lie.** Every header is checked against
cw-144 and the chain's own linkage; a fabricated header is rejected and the submitter pays the
fee.

**Why anyone does it:** `release_mint` requires the tip to have *advanced* past the deposit. If
the chain stalls, nothing releases — **including the advancer's own mint, if they have one.**
Anyone waiting on a deposit has a direct reason to push headers.

| Failure | What happens | Who bears it |
|---|---|---|
| An attacker pushes their own branch | It must be valid and strictly heavier. A tie keeps the incumbent | — |
| Nobody advances | Nothing releases. Minting stalls, but **nothing is falsely released** | Everyone, equally |
| BSV changes its difficulty rule (X3) | The client rejects every header and the bridge **halts** | Everyone. Recoverable, not a theft |

---

## 5 · The Challenger — proves theft, takes the bounty

```
1  WATCH     a relayer's deposit outpoint is spent
2  PROVE     slash(relayer, spending_tx, merkle_branch)             [designed]
               → verifies the spending tx IS IN A CANONICAL BLOCK
                 (same Merkle machinery verify_deposit already uses)
               → parses its outputs
               → requires they match NO registered payout for that relayer
               → seizes bond_R; the challenger is paid
```

**This instruction is what makes the bond real rather than decorative.** It is also the fix
for "who watches?" — the bounty is the incentive, so detection stops being an unpaid chore.

**The window is 32 hours from the spend landing in a block.** That is the header window, and it
is the honest weak point: a thief who spends and is unwatched for a day keeps the money.

| Failure | What happens | Who bears it |
|---|---|---|
| A challenger submits a fabricated spend | It is not in a block; rejected | The challenger, plus the fee |
| A challenger targets an honest relayer consolidating its own UTXO | **This is the over-broad case.** The predicate must distinguish an unauthorised spend from a mere re-spend of its own money | Needs specification |
| Nobody challenges within 32 hours | The theft stands | Holders |

---

## Does it hold?

Walking the five journeys against the failure tables:

| | |
|---|---|
| **A depositor's worst case** | Losing the deposit to a relayer who never gets minted — **no on-chain remedy.** Real, disclosed, and the strongest argument for the app automating the mint |
| **A holder's worst case** | A post-settle reorg with no resolver. **Fixable** — it needs a path, not a redesign |
| **A relayer's worst case** | Its own float, which is its own capital |
| **The system's worst case** | A theft no one challenges inside 32 hours. **Bounded by the bond, and only if the bond is large enough** |

**Three things this walk exposes that the design did not:**

1. **The depositor's cliff has no remedy.** Everything else returns; this does not.
2. **The post-settle reorg has no resolver** — an item no instruction can move.
3. **The challenge window is the real security parameter**, not the bond size. A bond is worth
   nothing if nobody is watching in the 32 hours it can be seized.

**The trust statement, plainly:**

> You trust each relayer with its own float. You are protected by a bond that anyone can seize
> **by proving theft on-chain, within 32 hours**, with a bounty for whoever proves it.

That is a trust assumption, stated and bounded — **not custody.** No single party holds the
reserve; no one can move funds without leaving on-chain evidence; anyone may act on that
evidence without asking permission.
