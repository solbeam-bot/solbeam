# 3. Architecture

> **Built or designed?** The built set is exactly four things: the **light client** (cw-144
> difficulty verification and Merkle inclusion), the **`solBSV` token**, the **mint**, and **fork
> staging with chainwork** — 27 passing on-chain tests. **The vault, the federation, threshold
> custody, governance, slashing and all of peg-out are designed and not built.** The shipped
> program mints straight to the depositor's token account. The
> [trust model](04-trust-model.md) records who bears the difference, and
> [`13-summary.md`](13-summary.md) is authoritative where this document disagrees with it.

## Components

```
        BSV CHAIN                              SOLANA
 ┌────────────────────────┐       ┌──────────────────────────────────┐
 │  One deposit script     │       │  solBSV (SPL token, 8 dp)        │
 │  ─ the federation's     │       │  ─ mint authority: a program PDA │
 │    threshold address    │       │  ─ no freeze authority           │
 │  ─ the reserve is BSV   │       │                                  │
 │    under a THRESHOLD    │       │  Light client (built)            │
 │    KEY: no single       │       │  ─ checkpoint + 192-record window│
 │    member can move it   │       │  ─ cw-144 PoW + Merkle inclusion │
 └───────────┬─────────────┘       │                                  │
             │                     │  Vault (designed)                │
             │  threshold-signed   │  ─ every mint lands here first   │
             │  payouts            │  ─ released after MATURITY, or   │
             └──────────► BSV users│    burned if a reorg is followed │
                                   │                                  │
                                   │  Federation (designed)           │
                                   │  ─ bonded members running nodes  │
                                   │  ─ relay headers, sign payout    │
                                   │    intents, challenge theft      │
                                   │  ─ governance: 85% / 30 days     │
                                   └───────────────┬──────────────────┘
                                                   │
                                    ┌──────────────▼──────────────┐
                                    │ Website — no consensus role │
                                    │ parameter display, status,  │
                                    │ external metrics            │
                                    └─────────────────────────────┘
```

| Component | Status | What it does |
|---|---|---|
| **BSV light client** (Solana program) | **Built** | Holds a checkpoint plus a rolling window of **192 BSV headers**. It answers one question — *is this transaction in this block, and is that block still canonical?* — by verifying proof-of-work and Merkle inclusion. Difficulty is implemented as **cw-144**, the rule the SV Node's `src/pow.cpp` uses, replayed against real mainnet headers at **324/324 exact**. Each of the 52-byte records carries the block hash, its cumulative chainwork and its timestamp, which is what the rule consumes. **This is what makes minting permissionless: the proof is the authorisation** |
| **The vault** (program-owned token account + a record per pending item) | **Designed, not built** | **Every mint lands here first, never with the depositor.** The program releases the staged `solBSV` once `MATURITY` passes with the deposit still canonical, or **burns** it if a reorg is followed. The release decision is made from the program's own stored headers — it compares the hash stored when the deposit was proven against the hash it holds now — so it needs no reporter. Because the tokens are in an account the program owns, releasing or burning is disposing of what it holds, which is what makes a mint reversible **without a freeze authority**. The current design carries unfixed audit findings and is being re-audited against this model; see [`21-vault-structural.md`](21-vault-structural.md) |
| **The federation** — bonding, Greycore co-signature, governance, slashing | **Designed, not built** | A set of **Greycore-admitted, bonded members** (**two-sided bonds, the float**, 1,000 BSV per side) who run software rather than exercising judgement. Each runs its own light client, relays headers, **signs payout intents individually** (which is what makes misbehaviour self-proving), and challenges theft. The reserve is held under a **2-of-2 `OP_CHECKMULTISIG`** — the gateway's threshold ECDSA key plus the **Greycore**'s — so **no gateway majority and no Greycore can move it alone**. **A bond can be seized by proving misbehaviour on-chain** — the program seizes the `solBSV` side, the members seize the BSV side collectively — and that, not the absence of trust, is what protects the reserve. See [`23-federation.md`](23-federation.md) |
| **The website** | Not built | Parameter display, status and the external metrics. **No consensus role at all** — it can be replaced or ignored without the program noticing |

`solBSV` itself is a classic SPL token: 8 decimals, **no freeze authority**, its mint authority a
program PDA, so **no external key can mint**.

**The removed component.** An earlier architecture carried an **order book** of staked bids with a
discovered fee. It is **removed**: it solved fee discovery and capacity allocation, and a governed
**30 bp** fee plus a bond cap solves both more simply — while deleting the one subsystem that never
received an adversarial review. [`12-peg-mechanism.md`](12-peg-mechanism.md) still describes the
book and is **superseded** on that point.

## The header state problem

A full BSV header chain cannot live on Solana economically. There are roughly **968,000 BSV
headers**; at current Solana rent that is tens of megabytes and hundreds of SOL, against a 10 MiB
per-account cap. The chain therefore lives on-chain as:

- a **checkpoint** (a recent, well-buried header), plus
- a **rolling window** of **192 subsequent headers** — 32 hours at BSV's ten-minute target — held in
  a single account of **10,107 bytes**, inside Solana's 10,240-byte account cap. A competing branch
  is staged in batches and committed only if **strictly heavier** in accumulated chainwork; ties
  keep the incumbent, so an equal-length branch cannot churn the tip.

The window is fixed by arithmetic rather than taste: the record must carry hash, chainwork and time
(52 bytes) because cw-144 subtracts two cumulative chainworks and two times 144 blocks apart, and it
must reach back **147 records** for the median-of-three "suitable blocks" at each end. 192 is the
largest window with real margin under the cap.

Verification itself is cheap: an 80-byte header double-SHA-256 costs **226 CU**, a 12-level Merkle
branch **2,616 CU** — a **full SPV deposit proof is about 2,842 CU**, negligible against Solana's
per-transaction limit. The expense is *state*, not computation.

This design is deliberately the **simplest** option: a checkpointed, optimistic header chain.
Zero-knowledge proof verification (Groth16/SP1-class) is a later hardening step — it is faster to
verify than to run, but the tooling is unaudited and, in the cheapest cases, restrictively licensed.

## Where the BSV sits

**One reserve, under a 2-of-2 `OP_CHECKMULTISIG`.** The BSV lives at the federation's reserve script —
the gateway's **threshold ECDSA** key plus the **Greycore**'s key — spendable only when **both** sign.
The gateway key is **never assembled in one place**. **The deposit script genuinely is a multisig, which
reverses audit F10:** `is_p2pkh` must change and `DepositScript::SPACE` must grow to ~71 bytes (from
38). Three properties matter, and they are the whole of the custody story:

- **No gateway majority — and no Greycore — can move it alone.** That is a property of the 2-of-2
  script and the shared gateway key, not a promise about behaviour.
- **It is trusted, and that is stated.** The gateway quorum **acting with the Greycore** can take the
  reserve, and the maximum loss is the entire non-member supply. The design does not pretend otherwise;
  it **bounds** the assumption with the Greycore co-signature, two-sided bonds and proofs anyone can
  submit, and it makes the
  theft **visible** by publishing the reserve and supply continuously (doc 07).
- **The bonds are two-sided and outside the reserve.** The mint side is native BSV held outside the
  reserve **under the collective key, not the member's own**; the redeem side is `solBSV`, seized by
  the program on Solana. Each is denominated in the asset its side holds, so no BSV price move shrinks
  it relative to what it protects, and the program can compare each pair on-chain with no oracle. The
  BSV-side bond is **seized by the members collectively** — a threshold-signed transaction, with the
  slashers paid from it — so it is a mechanism, though it is a **collective action by the majority**
  rather than an automatic rule.

The published invariant is `custodied BSV ≥ outstanding solBSV`. **The program cannot enforce it** —
the reserve is off-chain BSV it cannot read — so the website shows the ratio and the program does
not check it. What the program *can* check is that each bond covers its side, because those are
quantities it holds or measures. **Publishing that ratio is an early deliverable, not a late one.**

See [Parameters & governance](06-parameters.md) for the sizing of the bonds and the governance
threshold, and [The federation](05-federation.md) for the role.

## Governance and the upgrade authority

Governance is a **component**, not an afterthought, and it holds the **program upgrade authority**.
A change requires **85% of pledged coins** and takes effect after **30 days**, with the proposal
signalled **live from the moment it is raised**. The reason that is safe is architectural rather
than political:

**Redemptions can never be paused.** Pause stops **mints only**. So a proposal that would harm
holders cannot trap them: the 30-day signal is an exit window, and a hostile change **empties the
bridge before it lands**. There is no immutable floor, deliberately — **the floor is the exit
window.** *Designed, not built*, like everything else in this section.

## What the program consults

**No oracle.** The program reacts only to **BSV block headers** and **Solana slots**, and to nothing
else. Depth and block time are read from the headers; deadlines are measured in slots, so a cluster
halt **freezes** the clock rather than punishing anyone who could not act. External metrics — price,
hashrate, reorg cost — are published on the website and **never consulted by the program**. Nothing
the website says can change what the program accepts.

## Technology and licensing

SOLBEAM is **FOSS** (MIT), and dependencies are chosen for permissive licensing:

| Need | Choice | Licence |
|---|---|---|
| BSV headers, legacy transactions, Merkle | `btcsuite/btcd` | ISC |
| BSV signature hashing (SIGHASH_FORKID) | implemented in-house | — |
| BSV covenants (`OP_PUSH_TX` introspection) — **not used in the current design** | Rúnar, retained for the deferred signerless track | MIT |
| Solana programs | Anchor + SPL Token | Apache-2.0 |
| ZK verification (future) | `groth16-solana` | Apache-2.0 |
| Solana escrow/hashlock patterns | `kobby-pentangeli/atomic-swap` | MIT / Apache-2.0 |

Note: `scryptlib`'s *SDKs* are MIT, but the sCrypt compiler/stdlib that implements preimage
introspection is **not** permissively licensed, so Rúnar is used instead. The BSV Go SDK is under
the Open BSV License and is avoided for the same reason.

## What SOLBEAM does not require

- **No BSV full node.** A header source needs only chain data; the proof is verified on Solana, so
  the data source need not be trusted.
- **No Solana infrastructure.** A public or private RPC endpoint is enough.
- **No single operator.** Membership is open at two-sided 1,000 BSV bonds, and the website has no consensus
  role: no privileged party signs anything the program trusts, and the reserve needs a threshold
  rather than a key.

---

Next: [Trust model & security](04-trust-model.md)
