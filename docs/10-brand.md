# 10. Brand & visual language

> **Built or specified?** The light client, the `solBSV` token and the mint exist today and pass
> 17 on-chain tests. **The vault, the order book, per-relayer deposits and all of peg-out are
> designed, not built.** [`13-summary.md`](13-summary.md) is the authoritative account.


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
 │   CHOOSE         │        │   WAIT           │        │    on Solana     │
 │   TERMS          │        │   the bid names  │        │                  │
 └──────────────────┘        └──────────────────┘        └──────────────────┘
      BSV wallet              depth is a term of           Solana wallet
                              the bid, priced by
                              the market
```

- Left panel: BSV mark.
- Middle panel: a simple progress ring or the beam disc mid-rotation, with the confirmation depth beneath. **Depth is a term of the bid** — a staker names the depth it will accept, and the market prices it. `FLOOR` (12 blocks) is only a backstop, never a promise about any particular deposit, so never put a clock or a fixed number here.
- Right panel: Solana mark with the `solBSV` ticker.

This graphic is the social-friendly summary and should appear on the landing page, in the litepaper header, and on relayer-app onboarding.


## Voice

- **Plain, not promotional.** State what is trustless and what is trust-minimised, every time.
- **No overclaiming.** Avoid "fully trustless", "risk-free", "instant" for redemption, or "guaranteed" yields. **Never present a confirmation time as a guarantee:** depth is a term of the bid, the market prices it, and `FLOOR` is only a backstop.
- **No oracle.** The program reacts only to BSV headers and Solana slots. External metrics — price, hashrate, reorg cost — are published on the website and never consulted by the program, so copy must not imply it watches a market.
- **Precise words:** mint, redeem, bond, liquidity, depth, bid, deadline, proof.
- **Say the numbers.** The bid's liquidity, fee and confirmation depth; published caps. Depth is a term of the bid rather than a fixed constant, so quote it as a term, not a promise.

## Naming note

The word **"relayer"** can read as a trusted intermediary. In user-facing copy prefer **"bonder"** or **"redemption fulfiller"**, with "relayer" reserved for technical documentation. The role is bonded and permissionless — **anyone may run a relayer, and there is no privileged operator** — and the language should say so.

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
- [ ] Relayer app icons (desktop + mobile)
