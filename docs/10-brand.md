# 10. Brand & visual language

> **Built or specified?** The light client with cw-144, the `solBSV` token, the mint and fork
> staging exist today and pass **27 on-chain tests**. **The vault, the bonded federation,
> threshold custody, governance, slashing and all of peg-out are designed, not built.**
> [`13-summary.md`](13-summary.md) is the authoritative account.

## Name

**SOLBEAM** — *Atomic wrapper on Solana for BSV.*

The name plays on "beam" as both a bridge and a transfer of value, and on "Sol" as the destination chain.

## The mark

A single circular badge, centred, that carries **both** chains.

**Composition**

- A filled circle (the "beam disc") on a dark background.
- The **Solana** mark sits centred on the front face.
- The **BSV** mark sits centred on the reverse face.
- A thin ring around the disc, slightly brighter than the fill, as a subtle "orbit" cue.

**Motion**

- The disc is still by default.
- **Periodically** (every ~8 seconds) it performs a single **Y-axis rotation of 180°**, revealing the reverse face — Solana becomes BSV.
- It holds the BSV face briefly (~2 seconds), then rotates back.
- Easing: slow-out (`cubic-bezier(0.22, 1, 0.36, 1)`), ~900 ms per half-turn. No bounce, no overshoot.
- Optional micro-detail: a faint horizontal light sweep across the disc at the moment of the flip, reinforcing "beam".

**Reduced motion**

- If the user prefers reduced motion, the disc does not rotate. Instead it cross-fades between the two marks at the same interval, or stays on a static split-face mark (Solana on the left half, BSV on the right).

**Sizes**

- Favicon / app icon: the static split-face mark only — motion is not legible below 32 px.
- Header: the animated disc at 40–64 px.
- Hero: the animated disc at 160–240 px, centred.

**Reference implementation (CSS, illustrative)**

```css
.beam {
  width: 200px; height: 200px;
  perspective: 800px;
}
.beam__disc {
  position: relative;
  width: 100%; height: 100%;
  transform-style: preserve-3d;
  animation: beam-flip 8s cubic-bezier(0.22, 1, 0.36, 1) infinite;
}
.beam__face {
  position: absolute; inset: 0;
  display: grid; place-items: center;
  border-radius: 50%;
  backface-visibility: hidden;
}
.beam__face--bsv { transform: rotateY(180deg); }

@keyframes beam-flip {
  0%, 62%   { transform: rotateY(0deg); }    /* hold: Solana  */
  72%, 84%  { transform: rotateY(180deg); }  /* hold: BSV     */
  94%, 100% { transform: rotateY(360deg); }  /* return        */
}

@media (prefers-reduced-motion: reduce) {
  .beam__disc { animation: none; }
}
```

## The two-step graphic

The clearest expression of the product is two panels:

```
 ┌──────────────────┐        ┌──────────────────┐        ┌──────────────────┐
 │                  │        │                  │        │                  │
 │   STEP 1         │  ───►  │   STEP 2         │  ───►  │    solBSV        │
 │   SEND           │        │   WAIT           │        │    on Solana     │
 │   BSV            │        │   12 blocks,     │        │                  │
 │                  │        │   then maturity  │        │                  │
 └──────────────────┘        └──────────────────┘        └──────────────────┘
      BSV wallet              12 confirmations,             Solana wallet
                              then 144 blocks
                              before release
```

- Left panel: BSV mark.
- Middle panel: a simple progress ring or the beam disc mid-rotation, with the confirmation depth beneath. **State the depth as `FLOOR`, 12 blocks**, then the **144-block maturity** before release. There is no bid and no market-priced depth, so never put a variable number or a clock here.
- Right panel: Solana mark with the `solBSV` ticker.

This graphic is the social-friendly summary and should appear on the landing page, in the litepaper header, and on node-app onboarding.

## Voice

- **Plain, not promotional.** State what is trustless and what is trusted-and-bounded, every time.
- **No overclaiming.** Avoid "fully trustless", "risk-free", "instant" for redemption, or "guaranteed" yields. **Never imply the reserve is trustless:** minting and reversal are verified, the reserve is held by a bonded federation under a **threshold ECDSA key** (an ordinary P2PKH address, never assembled in one place), and what bounds it is **two-sided bonds** — the program seizes the `solBSV` side, the members seize the BSV side collectively — plus an exit window that never closes and continuous publication of the reserve and supply.
- **Say the federation, not "no operator".** The design has a federation, open membership, **two-sided bonds**, and governance at **85% of pledged coins over 30 days**. Do not describe it as an operator-less system; that was an earlier model. **Never say "threshold script"** — it is a threshold **key** (audit F10).
- **No oracle.** The program reacts only to BSV headers and Solana slots. External metrics — price, hashrate, reorg cost — are published on the website and never consulted by the program, so copy must not imply it watches a market.
- **Precise words:** mint, redeem, bond, **threshold key** (never "threshold script"), maturity, deadline, proof, equivocation.
- **Say the numbers.** The fee (**30 bp, governed**), the confirmation depth (**12 blocks**), maturity (**144 blocks**), the bonds (**1,000 BSV per side**), the signing threshold (**3-of-5 provisional, `open`**), and the governance defaults (**85% / 30 days**). Do not present a confirmation time as a promise.

## Naming note

The word **"relayer"** belongs to an earlier, per-relayer model and should not be used in user-facing copy. Say **"federation member"**, or **"member"** where the context is clear — the role is open to anyone with the bond, membership is not a committee seat, and **members run software rather than approving transactions**. Reserve the technical detail for the federation documentation.

## Colour and type (directional)

- **Base:** near-black background, high contrast.
- **Accent:** a single beam colour (suggested: a cold cyan-violet for the Solana side transitioning to a warm amber for the BSV side).
- **Type:** one geometric sans for headings, one humanist sans for body; tabular figures for all amounts.

## Asset checklist

- [ ] Animated beam disc (SVG + CSS, and Lottie for app use)
- [ ] Static split-face app icon (512 px, 192 px, 32 px)
- [ ] Two-step graphic (light + dark)
- [ ] Token logo for Raydium/Orca listings (`solBSV`, 512 px transparent PNG/SVG)
- [ ] Social banner and OpenGraph image (1200×630)
- [ ] Federation member node app icons (desktop + mobile)
