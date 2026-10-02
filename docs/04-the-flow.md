# 04. The flow — every actor's journey

Three actors instead of five. The old cast (Depositor, Holder, Relayer, Advancer, Challenger) split
one job across four parties: a relayer's float, an unpaid header-pusher, a challenger nobody funded.
Under the federation model they are **roles inside one piece of node software run by a bonded
member**, and the flows are short enough to read in one pass.

Written as journeys, so that **the failure cases sit next to the step that can fail, with who bears
the loss.**

---

## Where it stands

| | |
|---|---|
| **Light client** | **Built.** Checkpoint plus a rolling window of **192** BSV headers — **32 hours** — in one account of **10,107 bytes**, with a **52-byte** record per header (block hash, cumulative chainwork, timestamp). cw-144 verified against **324/324** real mainnet headers, and 160 real mainnet headers through `push_header` itself |
| **Token and mint** | **Built.** 79 on-chain tests, with negative controls |
| **Vault** | **Built**, with a **144-block (~24 hour)** maturity window — see [02. How it works §4](02-how-it-works.md#why-maturity-was-raised-from-0-to-144) |
| **Federation, Greycore, governance, peg-out** | **Designed, not built.** Everything after the mint step |
| **Order book** | **Removed.** A governed 30 bp fee replaces it |

## The cast

| Actor | Wants | Is trusted with | Its worst case |
|---|---|---|---|
| **User** | `solBSV` for BSV, and BSV for `solBSV` | Nothing | A deposit that never gets verified inside the 32-hour window — the one loss with no on-chain remedy |
| **Federation member** | Fees, pro rata to stake | **A share of the threshold ECDSA key over the whole reserve**, a share of the **collective key over the mint-side bonds**, and their own signed attestations | Its bond, and ejection |
| **Governance** | — (it is a process, not a party) | The upgrade authority, under 85% / 30 days / live signal | Nothing; a proposal that harms holders empties the bridge before it lands |

**The one trust assumption: a threshold of members do not collude.** Everything else is verified or
evidenced. That assumption is not eliminated — it is bounded, by **two-sided bonds, neither inside the
reserve** (the program seizes the `solBSV` side; the members seize the BSV side collectively), by
proofs anyone can submit, and by **continuous publication of the reserve and supply.**

## The instruction set

**Built — 16.** Counted from `#[program]` in `lib.rs`:

| Role | Instructions |
|---|---|
| Light client | `initialize`, `push_header`, `seed_headers` |
| Fork staging | `init_staging`, `push_fork_header`, `abandon_staging`, `commit_fork` |
| Authority | `propose_authority_change`, `execute_authority_change`, `cancel_authority_change` |
| Token and bridge | `initialize_token`, `initialize_bridge` |
| Mint and vault | `verify_deposit`, `release_mint`, `burn_staged`, `prune_nullifier` |

The immediate setters (`set_checkpoint`, `set_paused`) have been **removed**: checkpoint and pause
changes now go through the **timelocked authority path** (`TIMELOCK_SLOTS = 32`).

**Designed, not built:**

| Role | Instructions |
|---|---|
| Federation | `stake`, `announce_unbond`, `withdraw_bond`, `attest_payout`, `slash_equivocation`, `propose`, `vote`, `execute`, `pause_mints` |
| Peg-out | `request_redeem`, `finalize_redeem`, `cancel_redeem` |

`verify_deposit` **is** built as the vault needs it: it creates the nullifier, stages the mint in the
vault with `maturity_at_deposit`, and mints into the vault rather than to the depositor. What is missing
downstream is the federation and peg-out.

---

## 1 · The User — BSV in

```
1  SEND      BSV to the FEDERATION's deposit script
             OP_RETURN = version ‖ cluster_id ‖ program_hash ‖ recipient
             (designed; built checks only that the recipient's 32 bytes appear)
2  DEPTH     FLOOR = 12 confirmations (MIN_CONFIRMATIONS in the built code)
3  MINT      anyone calls verify_deposit(header, branch)              [built]
               → verifies PoW (cw-144), linkage and Merkle inclusion
               → creates a nullifier for (txid, vout)
               → mints NET into the vault, owned by the program,
                 recording deposit_hash, deposit_height and
                 maturity_at_deposit, and the recipient
4  MATURE    maturity_at_deposit of BSV — a STORED parameter, shipped at 144
5  RELEASE   anyone calls release_mint                                [built]
               requires: the client is FRESH (a header was pushed
                         within the staleness bound — StaleClient)
               requires: tip >= deposit_height + maturity_at_deposit
               requires: the stored hash at that height STILL MATCHES
                         and the height is still in the window
               → the vault pays the recipient
               if the hash DIFFERS, anyone calls burn_staged:
                 the staged tokens burn, and the BSV the reorg
                 returned stays with the sender
```

**Nothing here needs a member to sign, and nothing needs the User to act again.** Both `release_mint`
and `burn_staged` are permissionless — the caller is only a fee payer — so the app can do them and so
can anyone else.

### What can go wrong

| Failure | What happens | Who bears it |
|---|---|---|
| **Nobody pushes a header, and the deposit's block leaves the 32-hour window** | The proof can never verify again. **No tokens, and no on-chain remedy** | **The User.** The one hard cliff, and it is not dressed up |
| The client's view stalls short of the deposit | `release_mint` fails `StaleClient`; nothing releases | Nobody loses tokens; the deposit waits |
| The deposit is reorged out **and the reorg is noticed** | `burn_staged` burns the staged tokens; the reorg returned the BSV to the sender | Nobody — the User is where they started |
| The deposit is reorged out **and the window passes unnoticed** | Once the height leaves the window there is no hash left to check, so `release_mint` succeeds unchecked and the tokens are handed over against a block that no longer exists | **The reserve and every other User.** Detectable only inside `WINDOW − MATURITY` — 48 blocks at the designed values, and **zero at the maximum committed depth** |
| The reorg is shallower than `FLOOR` | Any reorg deep enough to erase a deposit is necessarily deeper than the 12-confirmation floor, so depth does not spare it | The User, unless the burn happens in time |
| A forged deposit proof | Rejected by the built checks: header in window, canonical hash, Merkle branch, output value and script, recipient committed in the `OP_RETURN` | The submitter, plus the fee |
| **The vault's maturity is lowered again** | At 0, release and burn are both permitted from the moment of staging — a **race**, and release normally wins. At the shipped 144 blocks a followed reorg is caught and burned instead | **The User and the reserve.** The window is a stored parameter; `MIN_CONFIRMATIONS = 12` is still the prevention, and the 144-block window is the reversal |
| The deposit output is later spent by the members themselves | The proof still verifies, because the program cannot see BSV-side spends: an unbacked mint. The federation's **reported spent-outpoint record** is the answer, and it is **not built** | **Users** |
| The peg-in fee's physical location is unspecified (`OP_RETURN` carries no amount) | The program can mint `net` and credit an accrual against a pool the members already hold; the 30 bp is decided by the federation's signing policy | **Users**, if the accrual and the reserve diverge |

**The 32-hour window is a real deadline and it exists for a reason** — the light client can only verify
a header it still holds.

---

## 2 · The User — `solBSV` in, BSV out

```
1  ESCROW     request_redeem(amount, bsv_destination, deadline_slot)   [designed]
                → solBSV moves into a NEW PegOutEscrow; a PegOut is created
                → a minimum deadline must be required, or see the last row
2  ATTEST     each member calls attest_payout(item, preimage, signature)  [designed]
                → the member signs THIS redemption individually:
                  id, amount, destination, deadline
                → recorded as a PayoutIntent[id, member]; NOT acted on
3  PAY        once enough attributed intents exist, the gateway threshold
              ECDSA key signs the BSV payment and the GREYCORE co-signs
4  FINALIZE   finalize_redeem(item, proof)                            [designed]
                → the payout pays exactly `amount` to `bsv_destination`
                → its OP_RETURN carries THIS redemption's id
                → payout depth >= C_payout
                → burns the escrow; closes the item
   or
4' CANCEL     cancel_redeem — permissionless, after deadline_slot        [designed]
                → escrow returns to the User. Supply unchanged
```

**Failure returns; it never mints.** There is no `claimed` latch, so there is no state that cannot be
resolved: `finalize` and `cancel` are mutually exclusive by time.

### What can go wrong

| Failure | What happens | Who bears it |
|---|---|---|
| A member never attests | Nothing stalls the others | Nobody |
| **A threshold of members never attests** | After the deadline, `cancel_redeem` returns the escrow. The User is **denied peg-out for the duration of the deadline**, though not robbed | **The User** — latency, not loss. Refusal leaves no signed artifact, so nothing is slashable |
| A member signs two conflicting intents | It produced its own proof of guilt. Anyone submits both; the bonds are seized — the `solBSV` side by the program, the BSV side by the members collectively | **The member's bonds** |
| ~~A member signs an intent matching no redemption~~ | **Deleted.** A closed `PegOut` is indistinguishable from one that never existed, so the predicate is undecidable and would false-positive against an honest member who attested before a cancel | — |
| The payout proof is replayed across two redemptions to one exchange address | It cannot be: the payment carries the redemption's id | Whoever tried it — rejected |
| The payout is reorged before it is `C_payout` deep | It does not finalize. The reserve can pay again; if not, the deadline returns the escrow | **Nobody.** The reserve does not depend on one transaction surviving |
| The escrow is burned on a proof that later vanishes | `C_payout` blocks must sit behind the payout **before** the burn, so this is not reachable through `finalize_redeem` | — |
| The user picks a past or very short `deadline_slot` | Potentially paid **and** refunded. The fix is a minimum deadline at step 1 | **The federation's reserve** until the minimum is enforced |

---

## 3 · The Federation member — the node software

A member is **an operator running software**, closest to a staked validator. There is no manual
approval of any transaction: the node watches, verifies, signs and challenges on its own, and the bond
prices the risk of that software being modified.

```
0  JOIN      stake(amount, script)                                    [designed]
               → the two-sided bonds: BSV on the mint side, outside the
                 reserve, under the COLLECTIVE key; solBSV on the redeem
                 side, seizable on Solana
1  WATCH     run its own light client; push headers              [push_header: built]
               → pushing is permissionless, but it is the member's job,
                 so it is a member's node that keeps the view current
               on a reorg: init_staging → push_fork_header → commit_fork
               → commits only if strictly HEAVIER (accumulated chainwork),
                 and only if the fork point has not moved (ForkPointMoved)
2  CUSTODY   hold a share of the gateway threshold ECDSA key over the reserve
               → no gateway majority can move funds; the GREYCORE co-signs
                 every spend
               and a share of the COLLECTIVE key over the mint-side bonds
               → a member cannot move its own bond; the members seize it
                 together, and the slashers are paid from it
3  ATTEST    sign each payout intent individually                     [designed]
4  CHALLENGE slash_equivocation(member, id, two signed intents)       [designed]
               → two signatures, one member, conflicting statements:
                 the entire proof. Anyone submits it
5  EARN      fees pro rata to stake
6  EXIT      announce_unbond → withdraw_bond after the unbonding period [designed]
               → requires both bonds still to cover what their sides hold
                 afterwards, so a member with outstanding obligations
                 CANNOT leave
```

**Making the challenger part of the node is the important change.** It was previously an unpaid chore
nobody owned; it is now a funded job done by the parties with the most to lose.

### What can go wrong

| Failure | What happens | Who bears it |
|---|---|---|
| A member equivocates on two intents | `slash_equivocation` — the member's own two signatures are the whole proof | **The member's bonds** |
| A member's key is stolen | The bonds are seizable, and its share of the threshold is not enough to move funds alone | **The member** |
| A member stops running the node | Minting stalls if too few push headers; redemptions stall if too few attest. Fees are not earned | **Everyone, until governance ejects it** |
| A member wants out while owing | `withdraw_bond` refuses while either bond fails to cover its side | — |
| **A threshold of members colludes** | Every signature is on record, so it is attributable — but there is no cryptographic proof against a valid-signing majority. **Continuous publication of the reserve and supply is the mitigation** | **The reserve.** This is the one trust assumption, and it is bounded, not removed |
| The bonds lose value in the incident | The redeem-side bond is `solBSV` and the liability is `solBSV`, so a reserve loss devalues that bond as it is seized. The mint-side bond is BSV, held outside the reserve | **Users**, to the extent of the shortfall |
| Attribution costs one account per member per redemption | Rent and per-redemption instructions scale as members × concurrent redemptions | The federation, in rent |
| Genesis — **decided** | Members post a **BSV-side bond at genesis**, so no `solBSV` needs to exist first. A capped, explicitly-unbonded first mint is a documented later option, not chosen | — |

**`owed` is deliberately not a single counter.** Under the federation the liability is per-redemption
and is attributed by each member's own signature, so there is nothing to double-count — and the
**two-sided bonds replace the single `bond >= k × owed` check**, so no shared `owed` counter needs
defining at all.

---

## 4 · Governance — 85%, 30 days, live signal

Governance is a process over members, not a fourth party. It holds **the upgrade authority**, which
means it can in principle change anything — including this document's parameters.

```
propose(payload)     any member;                                  [designed]
                     signal is LIVE from the moment it is raised
vote(proposal)       passes at 85% of pledged coins               [designed]
execute(proposal)    only at effective_slot = passed + delay      [designed]
pause_mints()        stops NEW mints only; a lower threshold,
                     auto-lifting                                 [designed]
```

**Mints can be paused. Redemptions cannot — so there is no instruction to pause them.** Pausing
inbound is a safety valve; pausing outbound is taking hostages. Because the power is bounded, pause
carries a lower threshold than a governance change.

### What can go wrong

| Failure | What happens | Who bears it |
|---|---|---|
| A hostile proposal passes | It is visible and live for **30 days**, during which redemptions run and anyone can leave. A change that harms holders empties the bridge before it lands | **Nobody who watches.** The floor is the exit |
| A holder does not watch for 30 days | The change lands against them | **The holder.** A disclosure obligation, not a mechanism |
| Governance loosens a safety parameter (`FLOOR`, maturity, the DAA rule) | 85% and 30 days is the only barrier; there is no immutable floor by design | **Users who stay** |
| Governance changes the deposit script | It changes which deposits a future mint will accept, which is a mint gate | **Depositors** |
| A member quorum silently refuses redemptions | Not a governance action and not an instruction — but it achieves what a redemption pause would | **The User.** Denied exit-to-BSV, not robbed |

**The floor is the exit window, not a constitution.** The protection was never that the rules are
frozen; it is that you can always leave before they change.

---

## 5 · Where this leaves the trust statement

| | |
|---|---|
| **A User's worst case in** | A deposit that never verifies inside the 32-hour window. **No on-chain remedy** — real, disclosed, and the strongest argument for the app automating the mint |
| **A User's worst case out** | Peg-out stalled by member inaction, recovered only by waiting out the deadline and taking `solBSV` back. **Denied service, not loss** — but the floor argument rests on redemptions being the exit |
| **A member's worst case** | Its bonds, seized on its own two conflicting signatures — the `solBSV` side by the program, the BSV side by the members collectively |
| **The system's worst case** | A threshold of members colluding. **Bounded by two-sided bonds, neither inside the reserve — and only if they are large enough and the price holds.** The maximum loss is the entire non-member supply |

**Three things this walk exposes that matter more than the old five-actor version:**

1. **The deposit path is the weakest link, not the redemption path.** Redemptions are permissionless to
   cancel and the escrow always returns. Deposits depend on the window being pushed and on the
   reported spent-outpoint record.
2. **Refusal is the unattributable failure.** Every escalation the design has needs a signed artifact.
   A member that does nothing signs nothing, so nothing can be proved and nothing can be seized. That
   is a hole in the middle of "the node software is the challenger."
3. **The bonds bound attribution, not loss.** They answer "who did it, provably." They do not answer
   "is the loss covered," because the redeem-side bond is denominated in the asset the loss devalues —
   although the mint-side bond is BSV held outside the reserve.

**The trust statement, plainly:**

> You trust that a **threshold of bonded members do not collude**, and that enough members are running
> the software to keep the BSV view current and to pay redemptions. Minting itself is trustless. What
> protects you is **two-sided bonds** that can be seized — the program seizes the `solBSV` side
> automatically, the members seize the BSV side collectively by a threshold-signed transaction — an
> exit that **cannot be paused**, continuous publication of the reserve and supply, and a reserve that
> **no gateway majority and no Greycore can move alone**.

That is a trust assumption, stated and bounded — and it **is** custody of the reserve, unlike the
per-relayer model it replaced. The trade is deliberate: one **2-of-2 reserve script** — the gateway's
threshold key plus the Greycore's — instead of many floats, attribution that actually works, and an
honest statement of what the threshold can still do.
