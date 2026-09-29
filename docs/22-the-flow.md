# 22. The flow — three actors, step by step

Three actors instead of five. The old cast (Depositor, Holder, Relayer, Advancer, Challenger)
split one job across four parties: a relayer's float, an unpaid header-pusher, a challenger
nobody funded. Under the federation model they are **roles inside one piece of node software
run by a bonded member**, and the flows are short enough to read in one pass.

Written as journeys, so that **the failure cases sit next to the step that can fail, with who
bears the loss** — that is the part of the old document worth keeping, and it is kept
everywhere below.

## Where it stands

| | |
|---|---|
| **Light client** | **Built.** Checkpoint plus a rolling window of **192** BSV headers — **32 hours** — in one account of **10,107 bytes**, with a **52-byte** record per header (block hash, cumulative chainwork, timestamp). cw-144 verified against **324/324** real mainnet headers |
| **Token and mint** | **Built.** 27 on-chain tests, with negative controls. `verify_deposit` mints gross, straight to the depositor |
| **BSV-side peg-in** | **Built.** 51/51 synthetic, 21/21 against a live SV Node |
| **Vault, federation, threshold custody, slashing, governance, peg-out** | **Designed, not built.** Everything in this document after the mint step |
| **Order book** | **Removed.** A governed 30 bp fee replaces it |

---

## The cast

| Actor | Wants | Is trusted with | Its worst case |
|---|---|---|---|
| **User** | `solBSV` for BSV, and BSV for `solBSV` | Nothing | A deposit that never gets verified inside the window — the one loss with no on-chain remedy |
| **Federation member** | Fees, pro rata to stake | **A share of the threshold key over the whole reserve**, and its own signed attestations | Its bond, and ejection |
| **Governance** | — (it is a process, not a party) | The upgrade authority, under 85% / 30 days / live signal | Nothing; a proposal that harms holders empties the bridge before it lands |

**The one trust assumption: a threshold of members do not collude.** Everything else is
verified or evidenced. That assumption is not eliminated — it is bounded, by bonds larger than
what a member could take and by proofs anyone can submit. The residual is stated in
§Where this leaves the trust statement.

---

## The instruction set

**Built — 11:** `initialize` `[built]`, `push_header` `[built]`, `verify_deposit` `[built]`,
`init_staging` `[built]`, `push_fork_header` `[built]`, `abandon_staging` `[built]`,
`commit_fork` `[built]`, `initialize_token` `[built]`, `initialize_bridge` `[built]`,
`set_checkpoint` `[built]`, `set_paused` `[built]`.

**Designed — vault:** `release_mint`, `burn_staged`, `prune_marker`, `request_redeem`,
`attest_payout` *(provisional)*, `finalize_redeem`, `cancel_redeem`.

**Designed — federation:** `stake`, `announce_unbond`, `withdraw_bond`, `slash_equivocation`,
`propose`, `vote`, `execute`, `pause_mints`.

`verify_deposit` is built, but **not as the vault needs it**: today it mints the **gross**
amount straight to the depositor's token account, with no escrow, no marker, no maturity, no
fee and no federation. Everything downstream of it in this document is a specification.

---

## 1 · The User — BSV in

```
1  SEND      BSV to the FEDERATION's deposit script
             OP_RETURN = version ‖ cluster_id ‖ program_hash ‖ recipient
2  DEPTH     FLOOR = 12 confirmations
3  MINT      anyone calls verify_deposit(header, branch)              [built]
               → verifies PoW (cw-144) and Merkle inclusion
               → today: GROSS to the depositor
               → designed: NET into a PegInEscrow owned by a new PegIn,
                 the 30 bp fee into the fee account, and a Marker
4  MATURE    MATURITY_BLOCKS of BSV — designed, not built, and the parameter is
             unset; what is built is the 192-record window itself
5  RELEASE   anyone calls release_mint                                [designed]
               requires: the client is FRESH (a header was pushed recently)
               requires: the stored hash at that height STILL matches,
                         or the height has left the window entirely
               → escrow to recipient
               if the hash DIFFERS, the deposit was reorged:
                 burn_staged burns the staged tokens, and the BSV the reorg
                 returned stays with the sender
```

**Nothing here needs a member to sign, and nothing needs the User to act again.** Steps 3 and
5 are permissionless, so the app can do them and so can anyone else. That is what leaves the
deposit path free of the consent requirement that used to strand it.

### What can go wrong

| Failure | What happens | Who bears it |
|---|---|---|
| **Nobody pushes a header, and the deposit's block leaves the 32-hour window** | The proof can never verify again. **No tokens, and no on-chain remedy** | **The User.** The one hard cliff, and it is not dressed up |
| The client's view stalls short of the deposit | Nothing releases — minting stalls, but nothing is falsely released | Nobody loses tokens; the deposit waits |
| The deposit is reorged out **and the reorg is noticed** | `burn_staged` burns the staged tokens; the reorg returned the BSV to the sender | Nobody — the User is where they started |
| The deposit is reorged out **and the window passes unnoticed** | Once the height leaves the window there is no hash left to check, so `release_mint` succeeds unchecked and the tokens are handed over against a block that no longer exists | **The reserve and every other User.** It is detectable only inside `WINDOW − MATURITY` — 48 blocks at the proposed values, and **zero** at the maximum committed depth. This is V5, carried forward |
| The reorg is shallower than `FLOOR` | Any reorg deep enough to erase a deposit is necessarily deeper than the 12-confirmation floor, so depth does not spare it | The User, unless the burn happens in time |
| A forged deposit proof | Rejected by the built checks: header in window, canonical hash, Merkle branch, output value and script, recipient committed in the `OP_RETURN` | The submitter, plus the fee |
| The fee's physical location is unstated (`OP_RETURN` carries no amount) | Possibly an **unbacked** accrual: the program mints `net` and credits a fee against a pool it does not read | **Users**, if it ever diverges. Open defect **N1**, critical |
| The deposit output is later spent by the members themselves | The proof still verifies, because the program cannot see BSV-side spends: an unbacked mint | **Users.** Open defect **N5**, the most serious in the re-audit |

**The 32-hour window is a real deadline and it exists for a reason** — the light client can only
verify a header it still holds. Two failure rows above are not accidents: **N1 and N5 are
unresolved**, and until they are, the deposit path's honesty rests on the federation's signing
policy rather than on the program.

---

## 2 · The User — `solBSV` in, BSV out

```
1  ESCROW     request_redeem(amount, bsv_destination, deadline_slot)   [designed]
                → solBSV moves into a NEW PegOutEscrow; a PegOut is created
                → a minimum deadline must be required, or see the last row
2  ATTEST     each member calls attest_payout(item, preimage, signature)  [designed]
                → the member signs THIS redemption individually:
                  id, amount, destination, deadline
                → recorded as PayoutIntent[id, member]; NOT acted on
3  PAY        once enough attributed intents exist, the threshold key
              signs the BSV payment out of the reserve
4  FINALIZE   finalize_redeem(item, proof)                            [designed]
                → payout pays exactly `amount` to `bsv_destination`
                → its OP_RETURN carries THIS redemption's id
                → payout depth >= C_payout
                → burns the escrow; fee accrues; closes the item
   or
4' CANCEL     cancel_redeem — permissionless, after deadline_slot       [designed]
                → escrow returns to the User. Supply unchanged
```

**Failure returns; it never mints.** And there is no `claimed` latch, so there is no state that
cannot be resolved: `finalize` and `cancel` are mutually exclusive by time.

### What can go wrong

| Failure | What happens | Who bears it |
|---|---|---|
| A member never attests | Nothing stalls the others. The other members attest, the threshold is reached, the payment goes out | Nobody |
| **A threshold of members never attests** | After the deadline, `cancel_redeem` returns the escrow. The User is **denied peg-out for the duration of the deadline**, though not robbed | **The User** — latency, not loss. Open defect **N2**: refusal leaves no signed artifact, so nothing is slashable |
| A member signs two conflicting intents | It produced its own proof of guilt. Anyone submits both; the bond is seized and the challenger takes the bounty | **The member's bond** |
| ~~A member signs an intent matching no redemption~~ | **Deleted (audit F8).** A closed `PegOut` is indistinguishable from one that never existed, so the predicate is undecidable and would false-positive against an honest member who attested before a cancel | — |
| The payout proof is replayed across two redemptions to one exchange address | It cannot be: the payment carries the redemption's id | Whoever tried it — rejected |
| The payout is reorged before it is `C_payout` deep | It does not finalize. The reserve can pay again; if not, the deadline returns the escrow | **Nobody.** The reserve does not depend on one transaction surviving |
| The escrow is burned on a proof that later vanishes | `C_payout` blocks must sit behind the payout **before** the burn, so this is not reachable through `finalize_redeem` | — |
| The user picks a past or very short `deadline_slot` | Potentially paid **and** refunded. The fix is a minimum deadline at step 1 | **The federation's reserve** until the minimum is enforced. Open item, **T6** |

---

## 3 · The Federation member — the node software

A member is **an operator running software**, closest to a staked validator. There is no manual
approval of any transaction: the node watches, verifies, signs and challenges on its own, and
the bond prices the risk of that software being modified.

```
0  JOIN      stake(amount, script)                                    [designed]
               → 1,000 BSV bond, posted as solBSV so it is seizable on Solana
1  WATCH     run its own light client; push headers                   [push_header: built]
               → pushing is permissionless, but it is the member's job,
                 so it is a member's node that keeps the view current
               on a reorg: init_staging → push_fork_header → commit_fork
               → commits only if strictly HEAVIER (accumulated chainwork)
2  CUSTODY   hold a share of the threshold key over the reserve
               → no single member can move funds
3  ATTEST    sign each payout intent individually                     [designed]
4  CHALLENGE slash_equivocation(member, id, two signed intents)       [designed]
               → two signatures, one member, conflicting statements:
                 the entire proof. Anyone submits it
5  EARN      fees pro rata to stake
6  EXIT      announce_unbond → withdraw_bond after the unbonding period [designed]
               → requires bond >= k × owed still holds afterwards,
                 so a member with outstanding obligations CANNOT leave
```

**Making the challenger part of the node is the important change.** It was previously an
unpaid chore nobody owned; it is now a funded job done by the parties with the most to lose.

### What can go wrong

| Failure | What happens | Who bears it |
|---|---|---|
| A member equivocates on two intents | `slash_equivocation` — the member's own two signatures are the whole proof | **The member's bond** |
| A member's key is stolen | The bond is seizable, and its share of the threshold is not enough to move funds alone | **The member** |
| A member stops running the node | Minting stalls if too few push headers; redemptions stall if too few attest. Fees are not earned | **Everyone, until governance ejects it** |
| A member wants out while owing | `withdraw_bond` refuses while `bond >= k × owed` fails | — |
| **A threshold of members colludes** | Every signature is on record, so it is attributable — but there is no cryptographic proof against a valid-signing majority | **The reserve.** This is the one trust assumption, and it is bounded, not removed |
| The bonded asset loses value in the incident | `solBSV` is the bond **and** the liability, so a reserve loss devalues the bond being seized | **Users**, to the extent of the shortfall. Open defect **N6** |
| Attribution costs one account per member per redemption | Rent and per-redemption instructions scale as members × concurrent redemptions | The federation, in rent. Open defect **N4** |
| Genesis: a member must bond `solBSV`, and none exists until a mint happens | There is no path for the first members | **Open. No answer in the design** |

**`owed` is deliberately not a single counter.** The old model accumulated one number per
relayer at both mint and accept, and it could not be discharged (T5). Under the federation the
liability is per-redemption and is attributed by each member's own signature, so there is
nothing to double-count — but `bond >= k × owed` still needs a definition of `owed` that does
not grow monotonically, and that is an open item.

---

## 4 · Governance — 85%, 30 days, live signal

Governance is a process over members, not a fourth party. It holds **the upgrade authority**,
which means it can in principle change anything — including this document's parameters.

```
propose(payload)     any member;                     [designed]
                     signal is LIVE from the moment it is raised, not when it passes
vote(proposal)       passes at 85% of pledged coins  [designed]
execute(proposal)    only at effective_slot = passed + 30 days   [designed]
pause_mints()        stops NEW mints only; a lower threshold, auto-lifting  [designed]
```

**Mints can be paused. Redemptions cannot — so there is no instruction to pause them.**
Pausing inbound is a safety valve; pausing outbound is taking hostages. Because the power is
bounded, pause carries a lower threshold than a governance change.

### What can go wrong

| Failure | What happens | Who bears it |
|---|---|---|
| A hostile proposal passes | It is visible and live for **30 days**, during which redemptions run and anyone can leave. A change that harms holders empties the bridge before it lands | **Nobody who watches.** The floor is the exit |
| A holder does not watch for 30 days | The change lands against them | **The holder.** A disclosure obligation, not a mechanism |
| Governance loosens a safety parameter (`FLOOR`, `MATURITY`, the DAA rule) | 85% and 30 days is the only barrier; there is no immutable floor by design | **Users who stay** |
| Governance changes the deposit script | It changes which deposits a future mint will accept, which is a mint gate | **Depositors.** Open item, **N5**'s neighbourhood |
| A member quorum silently refuses redemptions | Not a governance action and not an instruction — but it achieves what a redemption pause would | **The User.** Open defect **N2** |

**The floor is the exit window, not a constitution.** The protection was never that the rules
are frozen; it is that you can always leave before they change.

---

## 5 · Where this leaves the trust statement

| | |
|---|---|
| **A User's worst case in** | A deposit that never verifies inside the 32-hour window. **No on-chain remedy** — real, disclosed, and the strongest argument for the app automating the mint |
| **A User's worst case out** | Peg-out stalled by member inaction, recovered only by waiting out the deadline and taking `solBSV` back. **Denied service, not loss** — but doc 13's floor argument rests on redemptions being the exit |
| **A member's worst case** | Its bond, seized on its own two conflicting signatures |
| **The system's worst case** | A threshold of members colluding. **Bounded by bonds that cost more than they could take — and only if the bonds are large enough and the price holds** |

**Three things this walk exposes that matter more than the old five-actor version:**

1. **The deposit path is now the weakest link, not the redemption path.** Redemptions are
   permissionless to cancel and the escrow always returns. Deposits depend on the window being
   pushed and on `N5` — that a deposit output has not been spent by the members who hold the
   script.
2. **Refusal is the unattributable failure.** Every escalation the design has needs a signed
   artifact. A member that does nothing signs nothing, so nothing can be proved and nothing
   can be seized. That is a hole in the middle of "the node software is the challenger."
3. **The bond bounds attribution, not loss.** It answers "who did it, provably." It does not
   answer "is the loss covered," because the bond is denominated in the asset the loss
   devalues.

**The trust statement, plainly:**

> You trust that a **threshold of bonded members do not collude**, and that enough members are
> running the software to keep the BSV view current and to pay redemptions. Minting itself is
> trustless. What protects you is a bond that anyone can seize **by submitting a member's own
> two conflicting signatures**, an exit that **cannot be paused**, and a reserve that **no
> single member can move**.

That is a trust assumption, stated and bounded — and it **is** custody of the reserve, unlike
the per-relayer model it replaced. The trade is deliberate: one threshold key instead of many
floats, attribution that actually works, and an honest statement of what the threshold can
still do.
