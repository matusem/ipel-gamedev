# IPEL / IHRA — Server Requirements

> Fill in `[brackets]`. **Minimal** = acceptable for launch. **Optimal** = recommended production. **Expansion** = headroom for growth without new VM.

---

## Shared platform (all projects)

Applies to every service below unless a project section says otherwise.

### Host OS

| | |
|---|---|
| **OS** | Debian 14 (amd64) |
| **Deployment** | Docker (+ Docker Compose) per project |
| **TLS** | University certificate or Let's Encrypt on reverse proxy |
| **Public ports** | 443 HTTPS (80 → redirect optional) |
| **App ports** | Bind to `127.0.0.1` only; **do not** expose app ports publicly |

### Debian packages (host)

**Runtime (each VM):**

```bash
apt-get update
apt-get install -y --no-install-recommends \
  ca-certificates \
  curl \
  docker.io \
  docker-compose-plugin
```

**Optional (operations / CI deploy target):**

```bash
apt-get install -y --no-install-recommends \
  git \
  rsync \
  unattended-upgrades
```

No separate DB server packages on host when a project uses embedded SQLite. Install DB packages only where noted (IPEL Rust optimal/expansion).

### Reverse proxy (per hostname)

| Requirement | Detail |
|-------------|--------|
| **WebSocket** | Required — upgrade support on project-specific paths |
| **Proxy target** | `http://127.0.0.1:<port>` |
| **WS timeout** | ≥ 3600 s (long sessions) |
| **Compression** | Optional gzip/brotli for static assets |

### Authentication

| | |
|---|---|
| **Uni OAuth / SSO** | Required as **login option** on IPEL projects (GameDev, Rust) |
| **Local accounts** | May remain as fallback where already implemented |
| **Admin / dev access** | SSH public-key only for maintainers (`[team SSH keys on file with IT]`) |

### Access for maintainers

| Item | Requirement |
|------|-------------|
| **SSH** | Key-based login for deploy user(s); no shared passwords |
| **Deploy user** | Dedicated user in `docker` group, e.g. `upjs-gdd-deploy` |
| **Sudo** | `docker compose` + volume paths only, or rootless Docker policy per uni rules |
| **Secrets** | OAuth client ID/secret, DB passwords, deploy keys — stored in uni secret store / env files on server, not in git |

### CI/CD (all projects)

| Item | Detail |
|------|--------|
| **Source** | GitHub (`github.com/matusem/…`) |
| **Trigger** | Push to `main` → build image → deploy to target VM |
| **Registry** | `[ghcr.io / uni registry]` |
| **Deploy** | SSH → `docker compose pull && docker compose up -d` |
| **Persistent data** | Named Docker volumes; **never** wiped on redeploy |
| **Downtime** | Brief container restart; active WebSockets disconnect |

### Backup (all projects)

| | |
|---|---|
| **Frequency** | Daily minimum |
| **Retention** | `[7 / 14 / 30]` days |
| **Scope** | All persistent volumes + compose/env config |
| **Restore test** | Once before go-live |

### Sizing legend

| Tier | Meaning |
|------|---------|
| **Minimal** | Launch / one course cohort |
| **Optimal** | Recommended steady production |
| **Expansion** | Larger cohorts, more concurrent jobs, or second course without new architecture |

---

## UPJŠ GDD Platform — `gdd.ics.upjs.sk`

Multiplayer web game platform. Authoritative game logic in WASM on server; lobby and clients in browser.

| | Minimal | Optimal | Expansion |
|---|---------|---------|-----------|
| **Users** | 100 | 200 | 500 |
| **Concurrent games** | 25 | 50 | 120 |
| **Peak WebSockets** | ~150 | ~300 | ~700 |
| **vCPU** | 2 | 4 | 8 |
| **RAM** | 8 GB | 16 GB | 32 GB |
| **Disk** | 20 GB SSD | 40 GB SSD | 80 GB SSD |
| **Database** | SQLite (file volume) | SQLite | SQLite; migrate to Postgres only if multi-instance needed |
| **Instances** | 1 | 1 | 1 (single-instance design) |

```
Service:        UPJŠ GDD Platform — multiplayer web game platform
URL:            https://gdd.ics.upjs.sk/
Repository:     github.com/matusem/ipel-gamedev (framework/)
Deployment:     1× Docker container
OAuth:          Uni SSO as login option (required)
WebSockets:     /game, /graphql (lobby subscriptions)
Database:       SQLite — embedded, no separate DB server
Scaling:        Single instance only (in-memory game sessions)
Backup volumes: /app/data (SQLite), /app/games (published games)
Contact:        [dev team contact]
```

**WebSocket paths:** `/game`, `/graphql`

**Host packages:** shared runtime list only (no extra DB packages).

**Notes for admins:**

- No PostgreSQL/Redis required. Brief downtime on deploy is acceptable.
- **RAM:** each active game holds a Wasmtime instance (~15–40 MB) plus JSON state and WebSocket buffers; 50 concurrent games plus lobby traffic and Docker overhead needs **16 GB** at optimal, not 8 GB.

---

## IPEL Rust — `uwu.ics.upjs.sk`

Gamified portal for the Rust course: students link **their own Git repos** (GitHub / GitLab); the platform **displays commits** and runs **automated tests** against a chosen commit. Points and leaderboards. **Student source code is not stored** on our servers — only repo URL, commit metadata, and test results.

| | Minimal | Optimal | Expansion |
|---|---------|---------|-----------|
| **Students** | 100 | 200 | 500 |
| **Concurrent test runs** | 5 | 15 | 40 |
| **Test runs / day** | ~200 | ~500 | ~1500 |
| **vCPU** | 4 | 8 | 16 |
| **RAM** | 8 GB | 16 GB | 32 GB |
| **Disk** | 30 GB SSD | 60 GB SSD | 100 GB SSD |
| **Database** | PostgreSQL 16+ (small) | PostgreSQL 16+ | PostgreSQL + read replica optional |
| **Instances** | 1 app + 1 DB (or DB on same VM initially) | 1 app + managed/s separate DB VM | App + worker VM(s) + DB VM |

```
Service:        IPEL Rust — gamified Rust course exercise portal
URL:            https://uwu.ics.upjs.sk/
Repository:     [github.com/matusem/ipel-rust]
Deployment:     Docker Compose (app + PostgreSQL; optional worker container)
OAuth:          Uni SSO (required) + Git provider OAuth for repo access (GitHub/GitLab)
WebSockets:     [e.g. /ws — live test run status, leaderboard updates]
Git:            Student code on student Git accounts; platform clones at test time only
Database:       PostgreSQL (users, scores, repo URLs, commit SHAs, test results — not source code)
Scaling:        Horizontal workers for test queue (optimal+); DB separate from app
Backup volumes: PostgreSQL data only (no submission/source archive)
Contact:        [dev team contact]
```

**WebSocket paths:** `[TBD — e.g. /ws, /graphql]`

**Extra Debian packages (if PostgreSQL on same host):**

```bash
# Only if DB runs on host instead of container — prefer DB in Compose
apt-get install -y --no-install-recommends postgresql-client
```

**Compose services (expected):** `app`, `postgres`, optional `worker` (ephemeral `git clone` → `cargo test` → discard workspace).

**Notes for admins:**

- Heavier than GameDev: **CPU-bound** (Rust compile + test per run).
- Worker **clones repo at commit**, runs tests in sandbox, **does not persist student code** — disk is mainly OS, Docker, DB, and ephemeral build cache.
- Outbound **HTTPS to GitHub/GitLab** required for clones and commit APIs.
- Sandboxed execution likely needs **Docker-in-Docker** or dedicated worker with resource limits.
- Uni SSO + Git OAuth + PostgreSQL are hard requirements for production.

---

## IHRA — `ihra.ics.upjs.sk`

Modernized IHRA site: public web, **game upload** for competition, **validation** against many project templates (~same cohort size as GameDev).

| | Minimal | Optimal | Expansion |
|---|---------|---------|-----------|
| **Users / competitors** | 100 | 200 | 400 |
| **Concurrent uploads / validations** | 5 | 15 | 30 |
| **Templates** | ~10 | ~30 | ~60 |
| **vCPU** | 2 | 4 | 8 |
| **RAM** | 4 GB | 8 GB | 16 GB |
| **Disk** | 30 GB SSD | 60 GB SSD | 120 GB SSD |
| **Database** | SQLite or PostgreSQL | PostgreSQL | PostgreSQL |
| **Instances** | 1 | 1 | 1 app + optional validation worker |

```
Service:        IHRA — competition website and game submission portal
URL:            https://ihra.ics.upjs.sk/
Repository:     [github.com/matusem/ihra]
Deployment:     1× Docker container (Compose: app + DB if PostgreSQL)
OAuth:          Optional at launch; uni SSO recommended
WebSockets:     Optional — [validation progress / admin dashboard]
Database:       SQLite (minimal) or PostgreSQL (optimal+)
Scaling:        Single app instance; async validation queue for bursts
Backup volumes: DB volume, /uploads or equivalent submission storage
Contact:        [dev team contact]
```

**WebSocket paths:** `[TBD — optional]`

**Host packages:** shared runtime list; add `postgresql-client` if external Postgres.

**Notes for admins:**

- Upload + template validation similar traffic profile to GameDev uploads.
- WASM/template checks may need **WebAssembly runtime** inside container (no extra host libs).
- Competition deadlines cause **upload bursts** — disk and CPU spikes; optimal tier preferred.

---

## Summary matrix (for IT ticket)

Copy one block per VM/service request.

### GameDev — optimal

```
Hostname:       gdd.ics.upjs.sk
Project:        UPJŠ GDD Platform
OS:             Debian 14 amd64
CPU / RAM / Disk: 4 vCPU, 16 GB RAM, 40 GB SSD
Database:       SQLite (embedded, persistent volume)
Docker:         yes
HTTPS + WSS:    yes
OAuth:          uni SSO required
SSH:            maintainer keys, deploy user in docker group
CI/CD:          GitHub → SSH deploy
Backup:         daily, volumes: app/data, app/games
```

### GameDev — minimal (pilot)

```
Hostname:       gdd.ics.upjs.sk
Project:        UPJŠ GDD Platform
OS:             Debian 14 amd64
CPU / RAM / Disk: 2 vCPU, 8 GB RAM, 20 GB SSD
Database:       SQLite (embedded, persistent volume)
Note:           ≤25 concurrent games; upgrade to optimal tier before full cohort
```

### Rust — optimal

```
Hostname:       uwu.ics.upjs.sk
Project:        IPEL Rust
OS:             Debian 14 amd64
CPU / RAM / Disk: 8 vCPU, 16 GB RAM, 60 GB SSD
Database:       PostgreSQL 16+ (persistent volume or separate DB VM)
Docker:         yes (app + postgres [+ worker])
HTTPS + WSS:    yes
OAuth:          uni SSO + Git provider (GitHub/GitLab) for student repos
Git:            student code not stored; clone at test time only
SSH:            maintainer keys, deploy user in docker group
CI/CD:          GitHub → SSH deploy
Backup:         daily, PostgreSQL only
```

### IHRA — optimal

```
Hostname:       ihra.ics.upjs.sk
Project:        IHRA modernization
OS:             Debian 14 amd64
CPU / RAM / Disk: 4 vCPU, 8 GB RAM, 60 GB SSD
Database:       PostgreSQL (or SQLite for minimal pilot)
Docker:         yes
HTTPS + WSS:    yes (WSS optional)
OAuth:          uni SSO recommended
SSH:            maintainer keys, deploy user in docker group
CI/CD:          GitHub → SSH deploy
Backup:         daily, DB + upload storage
```

---

## Open items

- [ ] OAuth provider details: `[uni IdP URL, client registration process]`
- [ ] TLS: `[uni CA vs Let's Encrypt]`
- [ ] Reverse proxy: `[existing uni proxy / per-VM Nginx / Caddy]`
- [ ] Container registry: `[ghcr.io / harbor.uni.sk / …]`
- [ ] VM allocation: `[one VM per project vs shared host]`
- [ ] IPEL Rust repo URL, Git provider OAuth apps (GitHub/GitLab), worker/sandbox policy
- [ ] IHRA repo URL and template count at launch
- [ ] Backup retention and off-site copy
- [ ] Staging hostnames: `[gdd-staging.…, uwu-staging.…, ihra-staging.…]`

---

## Contact

| Role | Contact |
|------|---------|
| **Dev team** | `[name, email]` |
| **Course / faculty** | `[department, UPJŠ ICS]` |
