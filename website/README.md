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

## Deploy — Cloudflare Workers

**Why:** free, global CDN, unlimited static bandwidth on the free tier, automatic HTTPS, custom domain, and a clear upgrade path (Functions/R2) when it grows. Static hosting scales to practically any traffic without any server to manage.

### Live configuration

`solbeam.me` is served by a **Cloudflare Worker with static assets**. The migration off the old repo is complete.

| | |
|---|---|
| Account | `actftx` (the `workers.dev` subdomain) |
| Worker | **`solbeam-main`** → <https://solbeam-main.actftx.workers.dev/> |
| Source | <https://github.com/solbeam-bot/solbeam>, branch `main` |
| Asset root | **`website/`** — declared in `wrangler.jsonc` at the repo root |
| Custom domains | `solbeam.me` ✅ · `www.solbeam.me` ⚠️ **not attached yet** |
| Retired | the old `solbeam` Worker, and `adamski-t/solbeam` as the source |

`wrangler.jsonc` is what tells Workers Builds where the site lives:

```jsonc
{
  "name": "solbeam-main",
  "compatibility_date": "2026-09-24",
  "assets": { "directory": "./website" }
}
```

`name` must stay `solbeam-main`. If it changed, the next deploy would create a *second* Worker and the custom domain would keep pointing at the old one — the site would silently stop updating.

### Redeploying

Push to `main`. Workers Builds runs the deploy; there is no build step, so nothing to install and nothing to compile.

To deploy by hand from the repo root:

```bash
npx wrangler deploy
```

Do not also pass `--assets` on the command line now that `wrangler.jsonc` declares it. The config is the single source of truth, and specifying both is the one way to get a confusing deploy.

**Verify after any deploy:**

```bash
for p in / /about/ /styles.css /assets/solana.webp /assets/bsv.jpg /robots.txt /sitemap.xml /litepaper/; do
  printf '%-22s %s\n' "$p" "$(curl -s -o /dev/null -w '%{http_code}' https://solbeam.me$p)"
done
curl -sI https://solbeam.me/assets/bsv.jpg | grep -i cache-control   # expect max-age=31536000, immutable
```

Everything `200` **except** `/litepaper/`, which must be `301`. That `301` proves `_redirects` is honoured; the `cache-control` header proves `_headers` is honoured. Both files are read from the asset root — which is exactly why `assets.directory` must point at `website/` and not the repo root.

### Two behaviours worth knowing

- **`_headers` caching.** The `/*.html` rule matches *explicit* `.html` requests only. `/` and `/about/` are pretty URLs, so the platform default `max-age=0, must-revalidate` applies — a good default while iterating, and no fix needed.
- **The custom `404.html` is currently dead.** The live config uses the platform default, so an unknown path returns a bare `404` with an empty body rather than `website/404.html`. To serve it, add one line:
  ```jsonc
  "assets": { "directory": "./website", "not_found_handling": "404-page" }
  ```
  That is a visible behaviour change, so it is deliberately left off until wanted.

## Alternative: Cloudflare Pages (from scratch)


### 1. The code is already in a repo

Nothing to do — `website/` lives in <https://github.com/solbeam-bot/solbeam>. (If you were starting from scratch you would `git init` in this folder and push it, but this project does not need a separate site repo.)

### 2. Create the Pages project

1. Go to <https://dash.cloudflare.com> → **Workers & Pages** → **Create** → **Pages** → **Connect to Git**.
2. Pick the repo.
3. Settings — these exact values:
   - **Framework preset:** `None`
   - **Build command:** *(leave blank)*
   - **Build output directory:** `website`
   - **Root directory (advanced):** *(leave blank — do **not** also set this to `website`, or the build will look for `website/website`)*
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

The live setup is the Worker path: push to `main`, or run `npx wrangler deploy` from the repo root — `wrangler.jsonc` already declares the asset directory.

### Checking a deploy

Run these against **`solbeam.me`** — the apex is the only thing that matters. Swap in the `*.pages.dev` or `*.workers.dev` host if you want to check the origin before moving the domain.

```bash
for p in / /about/ /styles.css /assets/solana.webp /assets/bsv.jpg /robots.txt /sitemap.xml /litepaper/; do
  printf '%-22s %s\n' "$p" "$(curl -s -o /dev/null -w '%{http_code}' https://solbeam.me$p)"
done
```

Expected: everything `200` **except** `/litepaper/`, which should be `301` — that proves `_redirects` is being honoured. A `404` there means the host is ignoring `_redirects`.

Two more that matter:

- **`assets/bsv.jpg` must be 200.** If it 404s, the BSV face has nothing to show. Confirm both images are committed (`git ls-files assets/`).
- **Cache headers must still be applied.** `curl -sI https://solbeam.me/assets/bsv.jpg | grep -i cache-control` should show `max-age=31536000, immutable`. If it doesn't, `_headers` is not being consumed — which is the difference between Workers static assets (honours it) and a Worker with a custom `fetch` handler (does not).

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
| Hosting (Cloudflare Workers free tier) | $0 |
| SSL | $0 |
| CDN bandwidth | $0 (fair use) |
| The `solbeam.me` domain | already purchased |

---

## Handoff checklist

Already done:

- [x] Domain registered, DNS on Cloudflare
- [x] Site live at `solbeam.me`
- [x] `checks/`, `docs/` and `website/` all in `solbeam-bot/solbeam`
- [x] Cloudflare GitHub App granted access to the `solbeam-bot` account
- [x] Worker `solbeam-main` on `solbeam-bot/solbeam`, branch `main`
- [x] Asset root declared as `website/` in `wrangler.jsonc`
- [x] `solbeam.me` moved to the new Worker — serving our committed `index.html` byte-for-byte
- [x] Old `solbeam` Worker retired (now returns 404)
- [x] Cache purged
- [x] `/` and `/about/` return `200`
- [x] `/litepaper/` returns `301` — proves `_redirects` is honoured
- [x] `assets/bsv.jpg` returns `200` with `max-age=31536000, immutable` — proves `_headers` is honoured

Still to do:

- [ ] Attach `www.solbeam.me` as a custom domain on `solbeam-main` — **it currently has no DNS records at all**
- [ ] Add a zone Redirect Rule `www.solbeam.me/*` → `https://solbeam.me/$1` (301), keeping the apex canonical
- [ ] Set `adamski-t/solbeam` to private — the old source is confirmed replaced
- [ ] Optional: `"not_found_handling": "404-page"` in `wrangler.jsonc`, to serve the custom `404.html`
- [ ] TODO: add `assets/og.png` (1200×630) and uncomment the `og:image` meta tag in `index.html`

## Design notes

- **Palette:** Solana purple `#9945FF`, blue `#00D1FF`, green `#14F195`; BSV gold `#E4A11B`; near-black base `#05060A`.
- **The mark:** a coin that shows the Solana logo, spins on the Y axis, and reveals the BSV logo — the "beam".
- **Copy rule:** never claim more than the trust model supports. Minting is trustless; redemption is trust-minimised. If you add marketing lines, keep that distinction.
