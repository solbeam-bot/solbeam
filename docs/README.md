# SOLBEAM

**A wrapped-BSV token on Solana.**

SOLBEAM brings BSV to Solana as `solBSV` — a 1:1 wrapper you can hold, trade, and redeem back to real
BSV. It is pegged, not traded: there is no order book, no leverage, and no exchange mechanism inside
it.

[`13-summary.md`](13-summary.md) is the canonical model. Where any other document disagrees with it,
that one is right.

---

## The three parts

| | |
|---|---|
| **A light client on Solana** | Verifies BSV proof of work against the real difficulty rule (**cw-144**) and Merkle inclusion of a transaction in a block |
| **A vault** | Every mint lands here rather than in your wallet, and leaves only when the program is satisfied — or is burned if it isn't |
| **A bonded federation** | Members run nodes, hold the reserve under a **2-of-2 `OP_CHECKMULTISIG`** — the gateway's threshold ECDSA key plus the **Greycore**'s, both required — relay headers, sign payouts, and challenge theft. Membership is **admitted by the Greycore**, and **two-sided bonds** (the float) — the BSV side held under the same collective key and seized by the members |

---

## Putting BSV in

```
1  SEND      BSV to the federation's registered deposit script
             OP_RETURN carries your Solana address
2  DEPTH     12 confirmations
3  STAGE     solBSV is minted INTO THE VAULT, not to you — and a record stores
             the block hash your deposit was proven against
4  MATURE    144 blocks
5  RELEASE   anyone may call it, and it requires that the chain has ADVANCED past
             your deposit and that the stored hash at that height STILL MATCHES
             → the vault releases to you
             if the hash DIFFERS, the deposit was reorged: the staged tokens BURN,
               and you keep the BSV the reorg returned to you
```

**Step 3 is what makes a fraudulent mint unsellable** — a staged token is not in anyone's wallet, so
there is nothing to dump and no innocent buyer to inherit the loss.

## Taking BSV out

```
1  ESCROW    solBSV into the vault; a BSV destination and a deadline are set
2  ATTEST    members sign payout intents individually, on Solana
3  PAY       the threshold ECDSA key signs the BSV payment
4  SETTLE    the payout is proved against the light client; the escrow burns
   or
5  CANCEL    permissionless after the deadline: the escrow returns to you
```

**Failure returns; it never mints.** Supply is unchanged and you are whole without asking anyone.

> **Built or designed?** The light client, the `solBSV` token, the mint, and fork staging exist. **The
> vault, the federation, threshold custody, governance, slashing and all of peg-out are designed and
> not built.** The shipped program mints straight to the depositor's token account, so the vault and
> maturity steps above are a specification today, not shipped behaviour.

---

## Why this exists

BSV is fast to mine but slow to *move*. Exchanges hold deposits and withdrawals for long confirmation
windows because reorgs are expensive to them, and moving sizeable BSV between venues is a manual,
hours-to-days process. That friction is the biggest practical obstacle to BSV being used as money and
as a trading asset.

`solBSV` removes it. It is a Solana SPL token, so it moves at Solana speed, trades on Solana venues
like any other token, and can be used in Solana DeFi.

---

## What SOLBEAM is — and is not

| | |
|---|---|
| **The deposit is verified; the backing is reported** | The program verifies BSV proof of work and Merkle inclusion itself. **The program verifies deposits; the federation reports backing** — it reports spent deposit outpoints, because Solana cannot read the BSV UTXO set |
| **Reversal is trustless** | The program compares its own stored header hashes. A reorg is a fact about headers, not a report from anyone |
| **The reserve is trusted, and bounded** | The BSV sits under a **2-of-2 `OP_CHECKMULTISIG`** — the federation's **threshold ECDSA** gateway key (never assembled in one place) **and** the **Greycore**'s key, both required — so no gateway majority and no Greycore can move it alone. What protects a holder is a **bond anyone can seize by proving misbehaviour on-chain**: the program seizes the `solBSV` side, the members seize the BSV side collectively |
| **There is a federation** | Bonded members, **admitted by the Greycore**. They are paid to carry the risk; the bonds are the **float** and are held under the collective key, not each member's own |
| **There is governance** | 85% of pledged coins, 30 days, signalled live. It holds the upgrade authority — see *the floor* below |
| **No price oracle** | No external price or data feed gates anything. Minting is gated by a verified proof plus a coverage floor; peg-outs by the gateway threshold key and the **Greycore co-signature** |
| **Market layer is external** | Liquidity comes from Solana venues. SOLBEAM is the wrapper, not the market |
| **FOSS** | The light client, programs and node software are open source |

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
risk; **the mitigation is transparency**, which is promoted to an early deliverable: publishing the
reserve and supply continuously converts a hidden theft into a visible one, and for collusion and an
unchallenged outpoint spend that is the defence that remains. See [`13-summary.md`](13-summary.md)
and [`07-roadmap.md`](07-roadmap.md).

---

## Start here

- [The problem](01-problem.md)
- [How it works](02-how-it-works.md)
- [Architecture](03-architecture.md)
- [Trust model](04-trust-model.md)
- [The federation](05-federation.md)
- [Parameters & governance](06-parameters.md)
- [Roadmap](07-roadmap.md)
- [FAQ](08-faq.md)
- [Glossary](09-glossary.md)
- [Brand](10-brand.md)
- [Peg mechanism](12-peg-mechanism.md)
- [**The model — canonical**](13-summary.md)
- [Decisions](14-decisions.md)
- [Audit history](15-audit-2.md)
- [Costs](17-costs.md)
- [Pre-code checklist](18-pre-code-checklist.md)
- [The vault](21-vault-structural.md)
- [The flow](22-the-flow.md)
- [The federation, in detail](23-federation.md)
- [Parameters reference](24-parameters.md)
- [**Audit findings**](25-audit-federation.md)
- [Documentation refresh — historical](16-docs-refresh.md)
