# GitHub setup for SOLBEAM

How to stand up the repo and give the agent push access, safely.

**Note on account creation:** the agent cannot create the account for you —
GitHub requires a human, a verified email and a captcha, and automated signup
is against their terms. Everything else below is either already done or a
two-minute job for you.

---

## 0. Anonymity notes (read first if you want to stay unattributable)

The repo itself is easy to keep anonymous. These are the vectors that actually
leak identity, in rough order of how often people trip over them:

| Vector | What to do |
|---|---|
| **Commit author email** | Use the account's GitHub **noreply** address — `<id>+<username>@users.noreply.github.com`. A real address in `user.email` is published in every commit, forever |
| **The email on the GitHub account** | Use a privacy-focused provider or an alias, not your everyday address. It is not shown publicly, but it *is* the account's recovery path |
| **The domain** | Checked 2026-09-24: `solbeam.me` is registered at **GoDaddy**, not Cloudflare Registrar, and **privacy is already on** — the public record shows `Domains By Proxy, LLC` (GoDaddy's proxy service) with no name or email. Nameservers are Cloudflare. **Nothing to do.** Note only state/country are always public per ICANN policy |
| **Payments** | Anything that pays for infrastructure with a card de-anonymises the whole stack. Be proportionate: GoDaddy and Cloudflare already hold your payment identity, so GitHub anonymity is a smaller increment than it looks |
| **Linking back** | Don't reuse your personal GitHub's SSH keys, GPG keys, or commit email on this account |
| **Hosting accounts** | Cloudflare (already in use for `solbeam.me`) sees your IP and payment details. That is a separate identity surface from GitHub |
| **Recovery** | 2FA recovery codes are the thing you'll curse yourself for not saving. Store them offline |

The agent's side is already clean: it commits as `solbeam-agent`, uses a
noreply-shaped address, and holds no account credentials — only a repo-scoped
deploy key.

---

## 1. Account shape

**For this project: one account.** No personal account, no organisation. The
project account owns the repo, works from a project email, and the agent pushes
with a repo-scoped deploy key.

An earlier draft of this document suggested an organisation plus a separate
machine account. That is right for a team with billing and multiple
contributors; for a single anonymous project it is three times the surface area
for no benefit. Ignore it.

If the project later grows contributors, *then* add an organisation and move the
repo into it.

---

## 2. Create the repository

1. New repository → name `solbeam-poc` → **Private**.
2. **Do not** initialise with a README, `.gitignore` or licence — an empty repo
   makes the first push clean.
3. Note the URL: `https://github.com/<owner>/solbeam-poc.git`

---

## 2a. Two-factor authentication — do both

Two accounts control things that matter here: **GitHub** (the code) and
**Cloudflare** (DNS, the Worker, and the domain's nameservers). Enable 2FA on
both. Use a **TOTP app** (Aegis, Ente Auth, 1Password, Google Authenticator) —
not SMS, which is the weakest factor and the one worth SIM-swapping.

**GitHub** — Settings → Password and authentication → Two-factor authentication
→ set up with an authenticator app. When it shows the setup code, also open
**Recovery codes** and save them *before* you finish. Without a saved recovery
code, a lost phone means a lost account — and for an anonymous account there is
no ID document to fall back on.

**Cloudflare** — My Profile → Authentication → Two-Factor Authentication →
enable with an authenticator app → save the recovery codes it issues.

**Storage:** the recovery codes are the actual secret. Keep them offline and
somewhere different from the password (printed, or in an encrypted vault), and
not in the same password manager entry as the login.

**The agent never needs any of this.** It has no account access at all — only a
deploy key scoped to one repository. You hold the 2FA; that is the point.

---

## 3. Give the agent access

The agent is software: it cannot accept an invite or log in interactively. What
works is a **credential scoped to this one repository**, handed over through the
environment.

**Option A — deploy key (chosen; no account credentials involved)**

A keypair has already been generated on the agent's machine. The **private key
never leaves it**; only the public half is shared. Because `~/.ssh` is not
writable in the agent's sandbox, the key lives at
`/home/admin/ycash/.deploy_keys/` — outside this repo, so it can never be
committed.

**Public key to add:**

```
ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJyBFnsge+WTyfqdpVGsleb6bfHcD8QmFzucKb32SlOL solbeam-poc deploy key
```

Steps:

1. Repo → **Settings** → **Deploy keys** → **Add deploy key**.
2. Title: `solbeam agent`.
3. Key: paste the line above.
4. **Tick "Allow write access"** — without it the push is read-only.
5. Add.

Outbound SSH from the agent's machine is confirmed working to both
`github.com:22` and `ssh.github.com:443`. It pushes with an explicit key path
(never written into `.git/config`):

```bash
GIT_SSH_COMMAND='ssh -i /home/admin/ycash/.deploy_keys/solbeam_deploy \
  -o UserKnownHostsFile=/home/admin/ycash/.deploy_keys/known_hosts \
  -o StrictHostKeyChecking=accept-new' git push origin main
```

Revoke when the work pauses: Settings → Deploy keys → Delete.

**Option B — fine-grained personal access token**

On the **bot** account:

1. Settings → Developer settings → Personal access tokens → **Fine-grained tokens**
   → Generate new token.
2. **Resource owner:** the bot (or the org, if the org permits it).
3. **Repository access:** *Only select repositories* → `solbeam-poc`.
4. **Permissions** — Repository permissions:
   - **Contents: Read and write** (this is the only one needed)
   - **Metadata: Read** (auto-selected)
   - Leave *everything else* at No access: no Actions, no Secrets, no
     Administration, no Workflows, no org access.
5. **Expiration:** 30 days. Short is good; regenerate as needed.
6. Give it to the agent as an environment variable (e.g. `SOLBEAM_GITHUB_TOKEN`)
   — never paste it into chat or a committed file.

**What the agent should never be given:** an org-owner or admin token, a
classic PAT with `repo`+`workflow` scope, or any credential that can read
secrets, change billing or touch other repositories.

---

## 4. Protect `main`

Repo → Settings → Branches → Add branch protection rule for `main`:
- Require a pull request before merging.
- Do not allow force pushes or deletions.

The agent pushes to **feature branches** (e.g. `poc/p1-light-client`) and opens a
PR. Even a mistake cannot damage `main`.

---

## 5. How the agent uses the token

- Passed per command, never written to `.git/config` or any committed file.
- Never echoed into logs or pasted into source.
- Used only to push branches and open PRs on `solbeam-poc`.

When the work pauses, **revoke or rotate the token.**

---

## 6. What to do with the tarball in the meantime

If you would rather not issue a token yet:

```bash
tar xzf solbeam-poc.tar.gz
cd solbeam-poc
git init && git add -A
git commit -m "SOLBEAM PoC: plan and BSV verification checkers"
git branch -M main
git remote add origin https://github.com/<owner>/solbeam-poc.git
git push -u origin main
```

Then either issue a token for subsequent work, or keep exchanging tarballs.

---

## 7. Repo layout the agent will push

```
solbeam-poc/
├── README.md              plan, scope, task list, test matrix
├── GITHUB_SETUP.md        this file
└── checks/
    ├── bsvlib.py          shared BSV primitives (the reference implementation)
    ├── check_bsv_core.py  headers, PoW, Merkle
    ├── check_bsv_tx.py    transaction codec, SIGHASH_FORKID vs real signatures
    ├── check_bsv_deposit.py  deposit/redemption construction and signing
    └── run_all.sh         runs everything
```

Later: `services/` (Go bot), `programs/` (Anchor program), `scripts/` (regtest
and validator bring-up).
