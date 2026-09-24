# solbeam.me — website

A single-page static site for SOLBEAM. **No build step, no framework, no dependencies.** Open `index.html` and it works.

---

## What's here

```
website/
├── index.html              ← the landing page (edit copy here)
├── styles.css              ← all styling, incl. the coin animation
├── 404.html
├── favicon.svg
├── robots.txt
├── sitemap.xml
├── _headers                ← security + cache headers (Cloudflare)
├── _redirects              ← /litepaper/ → /about/ (kept for old links)
├── assets/
│   ├── solana.webp         ← Solana coin face
│   └── bsv.jpg             ← BSV coin face
└── about/
    └── index.html          ← About page ("WHAT" + links)
```

---

## Preview locally

```bash
cd website
python3 -m http.server 8080
# open http://localhost:8080
```

(Any static server works — `npx serve`, `php -S localhost:8080`, etc.)

---

## Editing

| Want to change | Where |
|---|---|
| Headline / tagline | `index.html` `.title`, `.tagline` |
| Step 1 / Step 2 text | `index.html` inside `<section class="steps">` |
| The GitHub button | `index.html` — the `.btn` href (currently `github.com/solbeam-bot/solbeam`) |
| About page copy | `about/index.html` — the "WHAT" list and links |
| Coin size | `styles.css` `.coin { width: clamp(140px, 38vw, 208px) }` |
| Spin timing | `styles.css` `@keyframes coin-flip` and the `9s` duration on `.coin__inner` |
| Colours | `styles.css` `:root` variables (Solana purple/green/blue, BSV gold) |
| **BSV face cropping** | `--bsv-zoom` in `:root`. If white corners show around the gold disc, raise it (2.05 → 2.2). If the disc looks too cropped, lower it |

The coin animation is pure CSS (`rotateY`), with a cross-fade fallback for `prefers-reduced-motion`. No JavaScript is required for it; the only JS on the page sets the footer year.

> **Do not add `filter`, `opacity`, `backdrop-filter` or `will-change` to `.coin__inner`.**
> Any of those forces `transform-style: flat`, which collapses the 3D context and the BSV back face stops rendering — the coin then just "flips the Solana logo around". Shadows belong on `.coin__face` instead. (This was a real bug in the first deployment.)

---

## Deploy — Cloudflare Pages (recommended)

**Why:** free, global CDN, unlimited static bandwidth on the free tier, automatic HTTPS, custom domain, and a clear upgrade path (Workers/Functions/R2) when it grows. Static hosting scales to practically any traffic without any server to manage.

### 1. Get the code into a repo

```bash
cd website
git init
git add .
git commit -m "SOLBEAM website"
git branch -M main
git remote add origin git@github.com:<you>/solbeam-site.git
git push -u origin main
```

### 2. Create the Pages project

1. Go to <https://dash.cloudflare.com> → **Workers & Pages** → **Create** → **Pages** → **Connect to Git**.
2. Pick the repo.
3. Settings:
   - **Framework preset:** `None`
   - **Build command:** *(leave blank)*
   - **Build output directory:** `/` (or `website` if the repo root is the parent folder)
4. **Save and Deploy.** You'll get a `*.pages.dev` URL within a minute.

**Or deploy straight from the CLI, no repo needed:**

```bash
cd website
npx wrangler pages deploy . --project-name solbeam
```

### 3. Point solbeam.me at it

**Option A — move DNS to Cloudflare (recommended, free, simplest):**

1. In Cloudflare: **Add a site** → `solbeam.me` → Free plan. Cloudflare shows two nameservers.
2. At your registrar, replace the nameservers with those two. Wait for propagation (minutes to a few hours).
3. In the Pages project: **Custom domains** → **Set up a domain** → add `solbeam.me` and `www.solbeam.me`. Cloudflare creates the DNS records for you.
4. SSL is issued automatically.

**Option B — keep DNS at your registrar:** add the `CNAME` (for `www`) and `A`/`AAAA` records that the Pages custom-domain screen shows you. Cloudflare will verify and issue SSL.

### Alternatives (all fine)

| Host | How | Notes |
|---|---|---|
| **Netlify Drop** | Drag the `website` folder onto <https://app.netlify.com/drop> | Fastest possible deploy; then add the custom domain in settings |
| **Vercel** | `cd website && npx vercel --prod` | Then add `solbeam.me` as a domain |
| **GitHub Pages** | Push to a repo → Settings → Pages → deploy from branch → root. Add a `CNAME` file containing `solbeam.me` | Free; add the CNAME/A records at your registrar |
| **Cloudflare R2 + CDN** | Upload the folder | Use later if you want object storage semantics |

---

## Updating the live site (git)

The repo is <https://github.com/solbeam-bot/solbeam>. If the Cloudflare project is connected to it, a push redeploys automatically.

```bash
# from your local clone
cd solbeam
git pull
# ...copy the updated files in (rsync, unzip, or edit directly)...
git add -A
git commit -m "About page, GitHub button, coin flip fix"
git push
```

If the Worker is deployed with Wrangler instead, deploy straight from the folder:

```bash
cd solbeam
npx wrangler deploy
```

### Checking a deploy

```bash
curl -s -o /dev/null -w "%{http_code}\n" https://solbeam.actftx.workers.dev/
curl -s -o /dev/null -w "%{http_code}\n" https://solbeam.actftx.workers.dev/about/
curl -s -o /dev/null -w "%{http_code}\n" https://solbeam.actftx.workers.dev/assets/bsv.jpg   # must be 200
```

That last one matters: **if `assets/bsv.jpg` returns 404, the BSV face has nothing to show.** Confirm both images are committed (`git ls-files assets/`).

---

## Scaling later

Nothing here needs replacing to grow:

- **Traffic:** a static site on a CDN has no realistic upper bound; no server, no scaling work.
- **Dynamic features** (relayer stats, reserve dashboard, live supply): add **Cloudflare Pages Functions** (a `/functions` folder) or a Worker — same deploy, no new infrastructure.
- **Big assets** (videos, litepaper PDFs, proofs): **R2** storage.
- **The docs site:** build the GitBook and either add it as a new folder (e.g. `litepaper/index.html`), or host it on `docs.solbeam.me` and link it from `index.html` and `about/index.html`.

## Cost

| Item | Cost |
|---|---|
| Hosting (Cloudflare Pages free tier) | $0 |
| SSL | $0 |
| CDN bandwidth | $0 (fair use) |
| The `solbeam.me` domain | already purchased |

---

## Handoff checklist

- [ ] Domain registered and access to its DNS controls
- [ ] Cloudflare account (or another host) created
- [ ] Repo created and this folder pushed
- [ ] Pages project connected, first deploy green
- [ ] `solbeam.me` + `www` added as custom domains, SSL active
- [ ] Landing page loads on mobile and desktop
- [ ] Coin animates (and respects reduced-motion)
- [ ] `robots.txt` / `sitemap.xml` reachable
- [ ] `/about/` loads and the GitHub link works
- [ ] `assets/bsv.jpg` returns 200 (the BSV coin face depends on it)
- [ ] TODO: add `assets/og.png` (1200×630) and uncomment the `og:image` meta tag in `index.html`

## Design notes

- **Palette:** Solana purple `#9945FF`, blue `#00D1FF`, green `#14F195`; BSV gold `#E4A11B`; near-black base `#05060A`.
- **The mark:** a coin that shows the Solana logo, spins on the Y axis, and reveals the BSV logo — the "beam".
- **Copy rule:** never claim more than the trust model supports. Minting is trustless; redemption is trust-minimised. If you add marketing lines, keep that distinction.
