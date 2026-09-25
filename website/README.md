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

### Where it is deployed today

`solbeam.me` is currently served by a **Cloudflare Worker with static assets** (not a Pages project):

| | |
|---|---|
| Account | `actftx` (the `workers.dev` subdomain) |
| Worker name | `solbeam` → <https://solbeam.actftx.workers.dev/> |
| Custom domains | `solbeam.me`, presumably `www.solbeam.me` |
| Source | `adamski-t/solbeam` — **being replaced by `solbeam-bot/solbeam`** |
| Asset root | the old repo's `website/` folder |

Workers static assets honours `_headers` and `_redirects` (confirmed: the `/assets/*` and `/*.css` cache rules from `_headers` are live on `solbeam.me`), so nothing about the header/redirect files needs to change — but **the asset root must become `website/` in the new repo**, because there is no `index.html` at the repo root.

> **Note on the HTML cache rule.** `_headers` sets `/*.html` to `max-age=300`, but that pattern matches *explicit* `.html` requests only. Requests for `/` and `/about/` are pretty URLs, so the platform default `max-age=0, must-revalidate` applies. That is a good default while iterating and needs no fix.

### Migrating off the old repo

Pick one. **Option A is recommended** — it ends with a plain static site, no Worker code to maintain, and it is the only path that needs no Wrangler config.

#### Option A — new Pages project on the new repo, then move the domain

1. **Workers & Pages → Create → Pages → Connect to Git.** When Cloudflare asks which repositories it may see, make sure the **`solbeam-bot`** account is included. This is the step people miss: the Cloudflare GitHub App was probably granted access to `adamski-t` only, and that grant does **not** extend to the new account.
2. Select `solbeam-bot/solbeam`, production branch `main`.
3. Use the build settings from step 3 above (`None` / blank / `website`).
4. **Save and Deploy**, then open the `*.pages.dev` URL and check the coin animates before touching DNS.
5. **Custom domains → Set up a domain** → add `solbeam.me` and `www.solbeam.me`. Cloudflare will offer to move them off the Worker; accept. The DNS records already exist and are reused.
6. Purge cache (`Caching → Configuration → Purge Everything`).
7. Verify with the checks below, then retire the old Worker and only then make `adamski-t/solbeam` private.

#### Option B — keep the Worker, repoint its source

Only works if the Worker is **Git-connected**: dashboard → the `solbeam` Worker → **Settings → Build**. If there is no Build tab, it was uploaded by hand and there is nothing to repoint — use Option A, or deploy from the new repo with the config below.

Workers Builds needs a Wrangler config to know where the assets live, and the repo has none. Create `wrangler.jsonc` at the repo root (note `name` must stay `solbeam` so this updates the existing Worker rather than creating a second one):

```jsonc
{
  "name": "solbeam",
  "compatibility_date": "2026-09-24",
  "assets": { "directory": "./website" }
}
```

Then either let the Git build run, or deploy it yourself from the repo root:

```bash
npx wrangler deploy
```

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

If you are staying on a Worker (Option B), add the `wrangler.jsonc` from that section and deploy from the repo root with `npx wrangler deploy`.

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
| Hosting (Cloudflare Pages free tier) | $0 |
| SSL | $0 |
| CDN bandwidth | $0 (fair use) |
| The `solbeam.me` domain | already purchased |

---

## Handoff checklist

Already done:

- [x] Domain registered, DNS on Cloudflare
- [x] Site live at `solbeam.me`
- [x] `checks/`, `docs/` and `website/` all in `solbeam-bot/solbeam`

Migrating the host off `adamski-t/solbeam`:

- [ ] Cloudflare GitHub App granted access to the **`solbeam-bot`** account (the old grant does not cover it)
- [ ] New Pages project — or repointed Worker — connected to `solbeam-bot/solbeam`, branch `main`
- [ ] Build output / asset root set to **`website`**, never the repo root
- [ ] The `*.pages.dev` (or `*.workers.dev`) preview renders and the coin animates, *before* any DNS change
- [ ] `solbeam.me` + `www.solbeam.me` moved to the new project, SSL active
- [ ] Cache purged
- [ ] `/` and `/about/` return `200`
- [ ] `/litepaper/` returns **`301`** — this is the proof that `_redirects` is honoured
- [ ] `assets/bsv.jpg` returns `200` **and** `cache-control: max-age=31536000, immutable` — the proof that `_headers` is honoured
- [ ] Old Worker retired
- [ ] `adamski-t/solbeam` set to private — **only after every line above passes**
- [ ] TODO: add `assets/og.png` (1200×630) and uncomment the `og:image` meta tag in `index.html`

## Design notes

- **Palette:** Solana purple `#9945FF`, blue `#00D1FF`, green `#14F195`; BSV gold `#E4A11B`; near-black base `#05060A`.
- **The mark:** a coin that shows the Solana logo, spins on the Y axis, and reveals the BSV logo — the "beam".
- **Copy rule:** never claim more than the trust model supports. Minting is trustless; redemption is trust-minimised. If you add marketing lines, keep that distinction.
