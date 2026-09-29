# 30. BLS, Alpenglow, and what could be proven back the other way

**Status: a note and a possible improvement. Not a dependency, and not buildable today.**

Raised for consideration: Solana validator workflows support creating **BLS public keys on-chain**
using **BLS12-381** primitives, which could let a chain validate **Solana's consensus**. This records
what is actually live as of the time of writing, what is not, what we could test **now**, and why it
matters for SOLBEAM.

**Verified against the sources rather than recalled** — which is the lesson of the last three errors:

- [BLS Pubkey and the Validator Admission Ticket](https://solana.com/upgrades/bls-pubkey-vat) — Solana Foundation, July 2026
- [SIMD-0388: BLS12-381 Elliptic Curve Syscalls](https://raw.githubusercontent.com/samkim-crypto/solana-improvement-documents/fa6b9685023d593858671c631d4a8fcba679784e/proposals/0388-bls12-381-syscalls.md) — status **Review**

---

## 1. What is actually live — the half that is true

| | |
|---|---|
| **BLS pubkey registration** | **Live on mainnet since 8 July 2026** (SIMD-0387), and live on devnet and testnet |
| **Validator Admission Ticket (VAT)** | **Live since 22 July 2026** (SIMD-0357). A validator without a registered BLS pubkey **stops participating in consensus** |
| **How it is registered** | A normal on-chain transaction, `VoterWithBLS` — a **48-byte compressed BLS pubkey** and a **96-byte proof of possession** |
| **CLI requirement** | **4.1.0 or higher** |

**So yes — validators can and do create BLS public keys on-chain.** That part of the claim is correct,
and it is a plain on-chain data field: **anyone can read a validator's BLS pubkey from its vote
account.**

**And the proof-of-possession is there for a specific reason worth knowing:** without it, an attacker
could register a *rogue* key built algebraically from their own key and a victim's, and then **forge
aggregate signatures that appear to come from the whole group**. Requiring a PoP at registration
prevents that.

## 2. What is NOT live — the half that is not

**SIMD-0388, the BLS12-381 syscalls, is status `Review` — not activated.** And the existing pairing
syscall is not wired up for this curve:

> *"There is an existing definition for a `sol_curve_pairing_map` syscall… **This function is not
> actually instantiated at the moment.**"*

**So a general BPF program cannot perform BLS12-381 operations today.** No group operations, no
pairing, no decompression for BLS12-381. What exists is `alt_bn128` (BN254) — **a different curve**,
and one the proposal explicitly says *"does not provide a 128-bit security level."*

**And the syscalls are not what Alpenglow needs for voting anyway** — the proposal says so directly:

> *"Alpenglow consensus votes themselves **do not require a BLS12-381 syscall** as they are processed
> differently than regular transactions."*

The syscalls exist so that **a BPF program** can verify a proof of possession once the vote program
moves to BPF.

**The precise position, then:**

| | |
|---|---|
| **Read a validator's BLS pubkey** | **Yes — it is on-chain data, today** |
| **Verify a BLS signature or aggregate in our program** | **No.** The syscalls are not live |
| **Verify a pairing / a BLS aggregate on BSV** | **No, and not planned.** BSV script has no pairing operation |

## 3. What we could test now — and it is a small, honest experiment

**Our toolchain is `solana-cli 4.1.2`, which meets the 4.1.0 requirement.** So the registration
workflow is testable locally:

```bash
solana-keygen bls_pubkey <AUTHORIZED_VOTER_KEYPAIR>          # derive the BLS pubkey
solana vote-authorize-voter-checked <VOTE_ACCOUNT> <KEYPAIR> <KEYPAIR>
solana vote-account <VOTE_ACCOUNT> | grep "BLS Public Key"   # confirm
```

**What that would establish:** that a BLS pubkey can be created and **read** on a local validator, and
what the account layout looks like. **What it would not establish:** anything about verifying BLS
cryptography in a program, because that is not available.

**Worth doing** because it is cheap and it is a fact rather than a claim — and because it tells us
whether the *local* validator activates the feature gate. It is a probe, not a feature.

## 4. Why it matters to us — and this is the interesting part

**Two separate consequences, and only one is about bridging.**

**(a) Alpenglow changes the peg-out economics.** Alpenglow ([SIMD-0326](https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0326-alpenglow.md))
targets **~150 ms finality** and moves voting off-chain, landing in **Agave 4.3, expected Q4 2026**.
Our peg-out settlement is gated on Solana finality. **Going from seconds to 150 ms materially
improves redemption**, and the BLS key is the prerequisite — which is why both are already live.

**(b) A BLS aggregate is a compact proof of Solana consensus — which is the ingredient for proving
Solana *back* to BSV.** Today our bridge is **one-way verified**: Solana verifies BSV, and peg-out
relies on the federation. A succinct certificate that a supermajority of Solana stake approved
something would be the primitive for a **Solana light client**, closing the other direction.

**But we could not consume it, and should be plain about why:**

- **A pairing check cannot be done in BSV script.** No such opcode exists and none is planned.
- So verification would need either an **optimistic** scheme (assume valid, allow a fraud proof) or a
  **covenant** — which is the same research track the reserve bond already depends on.

**So the honest note:** BLS pubkeys are live and Alpenglow will speed up settlement — **that part
matters and is real**. Proving Solana consensus on another chain is **possible in principle and not
reachable from BSV script today**, and it would rest on the covenant work that is already on the
critical path for the reserve.

## 5. What this does not change

**Nothing in the design may depend on this.** BLS12-381 syscalls are `Review` status; Alpenglow is
expected in a future release; and a Solana light client on BSV is not buildable regardless. **Recorded
as an improvement to watch, and the peg-out settlement speedup is the part worth tracking.**
