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
| **The domain** | `solbeam.me` is already registered in your name. Turn on WHOIS redaction (Cloudflare Registrar includes it free) and be aware the registrar and Cloudflare accounts are identity-linked |
| **Payments** | Anything that pays for infrastructure with a card de-anonymises the whole stack. Keep it separate or funded indirectly |
| **Linking back** | Don't reuse your personal GitHub's SSH keys, GPG keys, or commit email on this account |
| **Hosting accounts** | Cloudflare (already in use for `solbeam.me`) sees your IP and payment details. That is a separate identity surface from GitHub |
| **Recovery** | 2FA recovery codes are the thing you'll curse yourself for not saving. Store them offline |

The agent's side is already clean: it commits as `solbeam-agent`, uses a
noreply-shaped address, and holds no account credentials — only a repo-scoped
deploy key.

---

## 1. Account shape

**Recommended:** keep your personal account, create a **GitHub Organisation** for the
project (e.g. `solbeam-org`), and add a separate **machine account** (e.g.
`solbeam-bot`) as an organisation member.

Why not just a second personal account: GitHub's terms expect one free personal
account per human. A machine account used purely for automation is the accepted
pattern, and the organisation keeps ownership, billing and access control in
your name.

If you would rather keep it simple for now: one new personal account for the
project is fine to start, and you can migrate to an org later. Just add 2FA.

---

## 2. Create the repository

1. New repository → name `solbeam-poc` → **Private**.
2. **Do not** initialise with a README, `.gitignore` or licence — an empty repo
   makes the first push clean.
3. Note the URL: `https://github.com/<owner>/solbeam-poc.git`

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
