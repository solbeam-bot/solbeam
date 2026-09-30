# 01. What SOLBEAM is

**SOLBEAM issues `solBSV` on Solana, backed 1:1 by BSV held by a bonded federation.** Peg only: no
order book, no leverage, no exchange mechanism. Eight decimals, MIT, proof of concept. This document
is the orientation: the problem, what the wrapper fixes, the brand, the vocabulary, and the
questions people actually ask.

---

## 1. The problem: BSV is fast to mine, slow to move

A BSV block arrives roughly every ten minutes. That is not the bottleneck. The bottleneck is
**confirmation policy**: because a chain can reorganise, every custodian — exchanges especially —
chooses how many confirmations a block requires before treating it as final, and how long a
withdrawal must sit before it is broadcast. For BSV those windows are long.

The result is friction at exactly the wrong moment:

- **Moving sizeable BSV between venues** takes hours to days, and often a support ticket.
- **Trading BSV** means parking it on an exchange, with all the custody and counterparty risk that implies.
- **New users** — particularly where obtaining BSV is genuinely difficult (e.g. the USA) — face a slow, gated on-ramp.
- **Arbitrage** between venues is slow, so price dislocations persist longer than they should.

The asset itself is fine. The problem is plumbing.

## 2. What a wrapper fixes

A wrapped representation on a fast chain solves the movement problem without touching BSV itself:

1. **Speed.** Once `solBSV` exists, it moves at Solana speed — seconds, not hours.
2. **Permissions.** Minting and redeeming require no exchange account, jurisdiction or approval.
3. **Composability.** `solBSV` is an ordinary SPL token and can be used across Solana DeFi.
4. **An exit that cannot be closed.** Redemptions can never be paused, so the way back to BSV stays
   open even while governance is changing the rules.

## 3. Why not just use an existing bridge

Existing Bitcoin-family bridges to Solana are **federated or custodial**: guardian sets, notary
committees, or a custodian holding the coins. They work, but they ask you to trust a group of
operators, and their track record is mixed.

SOLBEAM's answer is a **bonded federation** — a deliberate change from earlier drafts, which
described an operator-less design. The honest division is:

- **The deposit is verified; the backing is reported.** A Solana program verifies BSV proof of work
  (under BSV's real rule, **cw-144**) and Merkle inclusion itself. Solana cannot read the BSV UTXO
  set, so the federation's software reports **spent deposit outpoints** and the program checks mints
  against that record. That is **not a new trust assumption** — the federation is already trusted
  with the reserve. The **upgrade authority** is the other exception: it can re-anchor the
  checkpoint, which is why production must hold it under a threshold and a timelock.
- **Reversal is trustless.** The program compares its own stored header hash against the one a
  deposit was proven with. A reorg is a fact about headers, not a report from anyone.
- **The reserve is trusted, and bounded.** The BSV sits under a **2-of-2 `OP_CHECKMULTISIG`** — the
  federation's **threshold ECDSA** gateway key (never assembled in one place) **and** the
  **Greycore**'s key. **Both must sign**, so neither the gateway majority nor the Greycore can move
  funds alone, and the Greycore polices every reserve spend. What protects a holder is a **bond
  anyone can seize by proving misbehaviour on-chain** — the program seizes the `solBSV` side, the
  members seize the BSV side collectively — not the absence of trust.

That leaves **one trust assumption: a threshold of federation members, together with the Greycore,
do not collude.** It is not eliminated. It is bounded by the **Greycore co-signature**, by two-sided
bonds, by a 30-day governed exit window during which redemptions never pause, and by proofs anyone
can submit. **Collusion is unprevented**, and the maximum loss is the entire non-member supply.

The bond is **the float** — working capital for transfers — and it is **not the scale limit**. The
claim that total value locked is capped by bonds pledged is **withdrawn**, along with its `~$180k`
figure: the reserve is constrained by the **Greycore co-signature**, not the bond. That is a proof of
concept, and it says so.

## 4. What SOLBEAM is — and is not

| | |
|---|---|
| **The deposit is verified; the backing is reported** | The program verifies BSV proof of work and Merkle inclusion itself. **The program verifies deposits; the federation reports backing** — it reports spent deposit outpoints, because Solana cannot read the BSV UTXO set |
| **Reversal is trustless** | The program compares its own stored header hashes. A reorg is a fact about headers, not a report from anyone. **The mechanism is built; its protective window ships at 0 (see 08)** |
| **The reserve is trusted, and bounded** | The BSV sits under a **2-of-2 `OP_CHECKMULTISIG`** — the federation's threshold ECDSA gateway key **and** the Greycore's, both required — so no gateway majority and no Greycore can move it alone. What protects a holder is a **bond anyone can seize by proving misbehaviour on-chain** |
| **There is a federation** | Bonded members, **admitted by the Greycore**. They are paid to carry the risk; the bonds are the float, held under the collective key, not each member's own |
| **There is governance** | 85% of pledged coins, 30 days, signalled live. It holds the upgrade authority — see *the floor is the exit* below |
| **No price oracle** | No external price or data feed gates anything. Minting is gated by a verified proof plus a coverage floor; peg-outs by the gateway threshold key and the **Greycore co-signature** |
| **Market layer is external** | Liquidity comes from Solana venues. SOLBEAM deploys no pool, quotes no price and provides no liquidity |
| **Not a general-purpose bridge** | It wraps BSV and nothing else |
| **Not a single-party custodian** | No one entity holds the reserve; the reserve needs a 2-of-2 of two quorums |
| **FOSS** | The light client, programs and node software are open source (MIT / ISC / Apache-2.0) |

### The floor is the exit, not a constitution

Governance can change rules, including the upgrade authority. **That is safe because redemptions can
never be paused.** A hostile proposal is visible for 30 days — and signalled live from the moment it
is raised — so anyone who dislikes it redeems and leaves. By the time it takes effect, the bridge is
empty.

**The residual, stated plainly:** a holder who does not watch and does not act within the delay is
exposed. That is a disclosure obligation, not a mechanism.

### What is not protected

**A colluding threshold can take the reserve, and nothing prevents it.** There is no on-chain
predicate that proves which members signed an off-chain threshold signature, so there is nothing to
slash on. The maximum loss is the entire **non-member supply**. The bonds are outside the reserve, so
they do not enlarge the prize — but they do not reduce the loss either. This is a stated, accepted
risk; **the mitigation is transparency**, promoted to an early deliverable: publishing the reserve and
supply continuously converts a hidden theft into a visible one, and for collusion and an unchallenged
outpoint spend that is the defence that remains.

---

## 5. Brand and visual language

**Name.** **SOLBEAM** — *Atomic wrapper on Solana for BSV.* The name plays on "beam" as both a bridge
and a transfer of value, and on "Sol" as the destination chain.

**The mark.** A single circular badge, centred, carrying **both** chains. A filled "beam disc" on a
dark background; the **Solana** mark centred on the front face; the **BSV** mark centred on the
reverse; a thin ring around the disc, slightly brighter than the fill, as an "orbit" cue.

**Motion.** The disc is still by default. Periodically (every ~8 seconds) it performs a single
**Y-axis rotation of 180°**, revealing the reverse face — Solana becomes BSV — holds the BSV face
briefly (~2 seconds), then rotates back. Easing is slow-out (`cubic-bezier(0.22, 1, 0.36, 1)`),
~900 ms per half-turn, no bounce and no overshoot. An optional faint horizontal light sweep crosses
the disc at the moment of the flip, reinforcing "beam".

**Reduced motion.** With `prefers-reduced-motion`, the disc cross-fades between the two marks at the
same interval, or stays on a static split-face mark (Solana left, BSV right).

**Sizes.** Favicon / app icon: the static split-face mark only — motion is not legible below 32 px.
Header: the animated disc at 40–64 px. Hero: the animated disc at 160–240 px, centred.

```css
.beam { width: 200px; height: 200px; perspective: 800px; }
.beam__disc {
  position: relative; width: 100%; height: 100%;
  transform-style: preserve-3d;
  animation: beam-flip 8s cubic-bezier(0.22, 1, 0.36, 1) infinite;
}
.beam__face {
  position: absolute; inset: 0; display: grid; place-items: center;
  border-radius: 50%; backface-visibility: hidden;
}
.beam__face--bsv { transform: rotateY(180deg); }

@keyframes beam-flip {
  0%, 62%   { transform: rotateY(0deg); }    /* hold: Solana  */
  72%, 84%  { transform: rotateY(180deg); }  /* hold: BSV     */
  94%, 100% { transform: rotateY(360deg); }  /* return        */
}
@media (prefers-reduced-motion: reduce) { .beam__disc { animation: none; } }
```

**The two-step graphic.** The clearest expression of the product is three panels:

```
 ┌──────────────┐      ┌──────────────┐      ┌──────────────┐
 │   STEP 1     │ ───► │   STEP 2     │ ───► │    solBSV    │
 │   SEND       │      │   WAIT       │      │    on Solana │
 │   BSV        │      │   12 blocks, │      │              │
 │              │      │   then 144   │      │              │
 └──────────────┘      └──────────────┘      └──────────────┘
    BSV wallet          12 confirmations,       Solana wallet
                        then 144 blocks
                        before release
```

Left panel: BSV mark. Middle panel: a progress ring or the beam disc mid-rotation, with the
confirmation depth beneath — state the depth as **`FLOOR`, 12 blocks**, then the **144-block
maturity** before release. There is no bid and no market-priced depth, so never put a variable number
or a clock here. Right panel: Solana mark with the `solBSV` ticker. *(The shipped program's vault
maturity is currently 0 — see 08; the graphic shows the designed values.)*

**Voice.**

- **Plain, not promotional.** State what is trustless and what is trusted-and-bounded, every time.
- **No overclaiming.** Avoid "fully trustless", "risk-free", "instant" for redemption, or
  "guaranteed" yields. **Never imply the reserve is trustless:** the deposit proof is verified and
  reversal is verified, but **the federation reports backing** (the spent-outpoint record), and the
  reserve is held under a **2-of-2 `OP_CHECKMULTISIG`**. What bounds it is the Greycore co-signature
  and **two-sided bonds** — plus an exit window that never closes and continuous publication of the
  reserve and supply.
- **Say the federation, not "no operator".** The design has a federation, **Greycore-admitted
  membership**, **two-sided bonds**, and governance at **85% of pledged coins over 30 days**. Do not
  describe it as an operator-less system; that was an earlier model. **"Threshold script" is not
  banned any more:** with the **Greycore co-signing**, the deposit script genuinely is a 2-of-2
  multisig, and audit **F10 is reversed**.
- **No price oracle.** The program reacts only to BSV headers and Solana slots. External metrics —
  price, hashrate, reorg cost — are published on the website and never consulted by the program. **One
  thing is reported:** the federation's **spent deposit outpoints** — say *"the program verifies
  deposits, the federation reports backing"*.
- **Precise words:** mint, redeem, bond, **2-of-2 reserve script / Greycore**, threshold **key** for
  the gateway leg, maturity, deadline, proof, equivocation.
- **Say the numbers.** The fee (**30 bp, governed**), the confirmation depth (**12 blocks**), maturity
  (**144 blocks designed; 0 shipped**), the bonds (**1,000 BSV per side, the float**), the gateway
  threshold (**4-of-N, `N` open**), the **Greycore** (size and threshold `open`), and the governance
  defaults (**85% / 30 days**). Do not present a confirmation time as a promise.

**Naming note.** The word **"relayer"** belongs to an earlier, per-relayer model and should not be
used in user-facing copy. Say **"federation member"**, or **"member"** where the context is clear —
members run software rather than approving transactions.

**Colour and type (directional).** Near-black background, high contrast; a single beam colour (a cold
cyan-violet for the Solana side transitioning to a warm amber for the BSV side); one geometric sans
for headings, one humanist sans for body; tabular figures for all amounts.

**Asset checklist.** Animated beam disc (SVG + CSS, and Lottie for app use); static split-face app
icon (512 / 192 / 32 px); two-step graphic (light + dark); token logo for Raydium/Orca listings
(512 px transparent PNG/SVG); social banner and OpenGraph image (1200×630); federation member node
app icons.

---

## 6. Questions people ask

**Is SOLBEAM trustless?** Not all of it. **The deposit is verified** — a BSV light client on Solana
verifies deposits under BSV's real difficulty rule (cw-144) — but **the backing is reported**. Solana
cannot read the BSV UTXO set, so the federation reports **spent deposit outpoints** and the program
checks mints against that record. That is not a new trust assumption, because the federation is
already trusted with the reserve: *the program verifies deposits, the federation reports backing.*
**Reversal is trustless too** — the program compares its own stored header hash. **The reserve is
trusted, and bounded:** the BSV is held under a **2-of-2 `OP_CHECKMULTISIG`** — the federation's
threshold ECDSA gateway key (never assembled in one place) **and** the Greycore's key — so both must
sign and neither the gateway majority nor the Greycore can move funds alone. The one trust assumption
is that **the gateway threshold and the Greycore do not collude.**

**What does "atomic" mean in the name?** Two things: the wrapper is **1:1**, and **minting is atomic
in the proof sense** — the deposit proof either verifies on Solana or it doesn't; there is no
discretionary approval. Redemption is not atomic: it is settled by a federation threshold signature
and proved back to the program.

**Where does my `solBSV` go after I deposit?** As designed, into a **program-owned vault**, never
straight to your wallet; the vault releases to you after a **maturity window of 144 blocks** passes
with the deposit still canonical. If BSV reorgs and the program follows the heavier branch, the
still-staged tokens are **burned**, and your BSV returned with the reorg, so you end where you
started. **The vault is built, but the shipped maturity is 0**, so the reversal is currently available
and racy rather than a protective window; `MIN_CONFIRMATIONS = 12` is what protects a deposit today.
**One real cliff, not dressed up:** the proof must be submitted while the deposit's block is still
inside the light client's **32-hour window** (192 records); after that it can never verify, and the
BSV is with the reserve.

**Why can't redemption be trustless too?** Because releasing real BSV requires a BSV signature, and
BSV Script cannot verify Solana's ed25519 consensus. Solana can check BSV; BSV cannot check Solana.
Nor can an opaque threshold signature tell you *which* member misbehaved — which is why members sign
payout intents **individually**, so misbehaviour produces its own evidence.

**What happens if a redemption fails, or a member disappears?** The escrow comes back. If the payout
does not settle, **anyone may cancel after the deadline** and the escrow returns to the holder.
Supply is unchanged, and **the bond is not slashed for a failed redemption** — the returned escrow
already makes the holder whole, and paying the bond too would compensate twice.

**What is the bond actually for, then?** **Self-proving equivocation.** A member who signs **two
conflicting payout intents** has produced its own proof of guilt; anyone may submit it and be paid
the bounty. It does **not** catch "an intent matching no authorised redemption" — that predicate is
undecidable (a closed `PegOut` looks like one that never existed) and would false-positive against an
honest member who attested before a cancel. A threshold signing something invalid is attributable
from the record but is a governance matter, not a cryptographic one.

**What if a threshold of federation members collude?** That is the trust assumption, and it is not
eliminated. It is **bounded**: two-sided bonds (the float, with a coverage floor so a member cannot
leave while owing); a **2-of-2 reserve script** so the gateway majority cannot move funds alone and
the **Greycore co-signs every reserve spend**; misbehaviour leaves on-chain evidence; and the reserve
and supply are published continuously, so a theft cannot be hidden. The maximum loss is the entire
non-member supply, and that is stated rather than minimised.

**What if BSV's price rises sharply?** Nothing in the mechanism breaks. **Each bond is denominated in
the asset its side holds** — the mint side in BSV held outside the reserve, the redeem side in
`solBSV` — so a price move scales each bond with the exposure it covers. There is no oracle to feed
and no price governor to react. **There is no capacity cap to move:** the bond is the float, and the
reserve is constrained by the Greycore co-signature.

**Who controls SOLBEAM? Is there an operator?** There is no single operator, but there **is** a
federation and there **is** governance. **The Greycore admits members** and members post the two-sided
bonds. Governance passes changes at **85% of pledged coins with a 30-day delay**, signalled live, and
it **holds the upgrade authority**. It **cannot touch redemptions**: they are never pausable. A pause
stops **mints only**. On `solBSV` itself there is **no freeze authority**.

**What are the fees?** **30 bp, governed**, each way. An earlier draft had an order book quote the
fee; the order book is **removed**. There is no book, no matching and no market-making.

**Why does a deposit wait?** Twelve blocks is **`FLOOR`**, the minimum confirmation depth; maturity is
then **144 blocks designed**, chosen so the program has time to see a reorg and burn the staged tokens
instead of releasing them. The deposit's whole proof must land inside the light client's **32-hour
window**. Redemption deadlines are measured in **Solana slots**, so a cluster halt freezes the clock.

**Is it open source? Do I need KYC?** Yes; and no — a BSV wallet and a Solana wallet are all you need.
Compliance posture around any *market* layer is a matter for the venues where `solBSV` trades.

**Can I run a federation member?** Yes. Post the **two-sided bonds** — a BSV-side bond held outside
the reserve under the collective key, and a `solBSV`-side bond seizable on Solana — run the node
software, and earn fees pro rata to stake. No committee approves you and there is no whitelist.
**Genesis is decided:** members post a BSV-side bond at genesis, so no `solBSV` needs to exist first.

**How do I verify the reserve?** The reserve is native BSV held by the federation under a threshold
key, and **the Solana program cannot read it**. The `solBSV` supply is on-chain. **Publishing the
reserve and the supply continuously is an early deliverable, not a late monitoring task** — because
for collusion and an unchallenged outpoint spend, visibility is the defence that remains. Until it is
live, treat any figure shown outside the chain as an assertion, not a proof.

**What if the bridge program has a bug?** Caps start conservative, and the **pause** is the intended
stop for new minting while holders redeem. Two honest qualifications: the vault's *design* failed two
audits before the current build, and there has been no external audit. The upgrade authority is held
by **governance** at 85% / 30 days.

**Is `solBSV` the same as BSV?** No — it is a claim on BSV, redeemable 1:1 through the peg, carrying
the trust assumptions in [05. Trust model](05-trust-model.md).

**Where can I trade `solBSV`? Do I have to wait?** Wherever SPL tokens trade — pools, apps, wallets,
market makers, and eventually centralised exchanges. SOLBEAM runs none of that. Only the peg has
latency; a market trade settles in seconds, and a market maker can pay you immediately while it
absorbs the peg delay.

**Do I need to run a BSV node?** Users do not — a wallet and an RPC endpoint are enough, because the
proofs are verified on Solana and data sources need not be trusted. Federation members run the node
software, and each verifies independently with its own light client.

---

## 7. Vocabulary

Plain-English definitions of the terms this set uses.

**Atomic (in "atomic wrapper")** — the wrapper is 1:1 and minting is authorised purely by a proof: the
deposit either verifies on Solana or it doesn't, with no discretionary approval. It does **not** mean
redemption is instantaneous.

**Bond** — the collateral a federation member lodges to join. **Two bonds, one per direction, and
neither inside the reserve:** a BSV-side bond held outside the reserve, and a `solBSV`-side bond, the
leg the program seizes on-chain. **The bond is the float** — working capital that lets the federation
serve redemptions — not a capital requirement sized against the reserve, and it does not cap capacity.
The `k × (held)` line is a **coverage floor** (`k = 1`), which stops a member leaving while it still
owes. The **BSV-side bond is held under the collective key, not the member's own, and is seized by the
members collectively** with a threshold-signed transaction, the slashers being paid from it — a
collective action by the majority, not an automatic rule.

**Burn** — destroying `solBSV`. A followed reorg makes the vault burn tokens that were minted but
never released; a settled redemption burns the escrow.

**Challenger** — the party that proves misbehaviour on-chain. Under the federation model the **node
software is the challenger**; **anyone** may still submit self-proving evidence and be paid the
bounty.

**Checkpoint (light client)** — a recent, deeply buried BSV header the on-chain client treats as its
starting point. The chainwork baseline is checkpoint-relative.

**DAA (Difficulty Adjustment Algorithm)** — BSV's rule for adjusting mining difficulty, recalculated
**every block** over a 144-block window. The light client verifies it per block: the rule is
**cw-144**, taken from the SV Node's `src/pow.cpp`, verified against real mainnet headers at
**324/324 exact**. It is **hard-coded**, and that is an open item (X3).

**Equivocation** — a member signing **two conflicting payout intents**. Because each member signs
individually and the intents are recorded on Solana, the two signatures are the entire proof.

**Exit window** — the floor of the system, and deliberately not a constitution. Because redemptions
**can never be paused**, a governance change takes **30 days** with live signal, so a proposal that
would harm holders empties the bridge before it lands. A holder who does not watch is exposed.

**Federation member** — an operator running software, not a person exercising judgement. Each node
watches both chains, verifies independently with its own light client, signs payout intents
individually, and challenges theft automatically. Closest analogue: running a staked validator.

**`FLOOR`** — the minimum deposit confirmation depth, **12 BSV blocks**. A governable parameter in the
design; `MIN_CONFIRMATIONS = 12`, fixed in the built code.

**Governance delay** — the **30 days** between a proposal passing at **85% of pledged coins** and
taking effect, signalled live from the moment it is raised.

**Light client** — a program that verifies a chain's headers and transaction inclusion without
downloading the whole chain. SOLBEAM runs a BSV light client **on Solana**, holding a checkpoint and a
rolling window of **192 records** (52 bytes each, `SPACE` **10,107** of 10,240) carrying a hash,
cumulative chainwork and time per header.

**Maturity** — how long a staged mint must stay unreorged before the vault releases it. Designed at
**144 blocks**; **shipped at 0**. Depth sets the cost of attacking; maturity sets the time available
to detect.

**Merkle proof** — a short cryptographic path showing a transaction is included in a block.

**Mint** — creating `solBSV` against a proven BSV deposit. The tokens land in the **vault**, not the
depositor's wallet.

**`OP_RETURN`** — a BSV output carrying arbitrary data. A SOLBEAM deposit is designed to attach
`version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient`; what is built checks only that the
recipient's 32 bytes appear in an `OP_RETURN`.

**Pause** — an emergency stop for **new mints only**. Redemptions continue, always, and the pause
lifts automatically unless renewed.

**Payout intent** — a member's individual signature, cast on Solana, approving a specific BSV payout
for a redemption. Intents are recorded, so every approval is **attributed**.

**Peg-in / peg-out** — moving value into the wrapper (BSV → `solBSV`) and back out.

**Pledged coins** — the `solBSV` members have pledged as bonds. Governance thresholds are counted
against these.

**Proof-of-reserves** — a published, independently checkable statement that the BSV held matches the
`solBSV` supply. **An early deliverable**, since the program cannot read the reserve.

**Reserve** — the native BSV backing `solBSV`, held by the federation under a **2-of-2
`OP_CHECKMULTISIG`**: the gateway's threshold ECDSA key plus the Greycore's, and both must sign.

**`solBSV`** — the wrapped BSV token on Solana: a classic SPL token, 8 decimals, no freeze authority.

**SPV** — verifying a transaction is in a chain by checking block headers and Merkle proofs.

**Threshold key** — the key over the reserve, requiring a threshold of federation members to sign.
**The threshold value (`4-of-N`) and whether the key is sharded are undecided.**

**Trustless** — correct without trusting any participant. **Minting and reversal qualify.** The
reserve does not: it is trusted, and bounded.

**Unbonding period** — the notice a member must serve before its bond is released. **Undecided**; both
bonds must still cover what their sides hold at withdrawal.

**Upgrade authority** — the program's upgrade key. Under the federation model it is **held by
governance** — 85% of pledged coins and a 30-day delay — rather than being an unowned gap. It cannot
pause redemptions. In the built code the checkpoint/pause path is **timelocked (32 slots)** but still
one key.

**Vault** — a **program-owned token account that every mint lands in first, never with the user**.
Because the tokens sit in the program's own account, the program can **burn them if a reorg is
followed** or release them on maturity, without a freeze authority. Peg-out escrows into the vault.
**Built**; its protective window ships at 0.

### Terms of earlier models — removed or superseded

Kept only so older documents can be read. **None is part of the current design.**

- **Order book / "the book"** — **removed.** The fee is now a governed 30 bp.
- **Discovered fee** — **removed** with the book.
- **Bonded relayer** — **superseded** by *federation member*.
- **"Threshold script"** — **no longer superseded wording.** With the Greycore adopted as a co-signer
  the deposit script genuinely **is** a 2-of-2 multisig; audit **F10 is reversed**.
- **`owed_R` / exposure / relayer consent** — **superseded** by the two-sided bonds.
- **Hot wallet / naked spend** — **superseded.** There is no hot float; the analysis now leads to a
  BSV-side bond held outside the reserve.
- **The two gates** — **superseded** by the vault's own release-and-burn rule.
- **Cold reserve / covenant, tranche, veto-only cosigner** — **superseded.**
