# Deployment (Raspberry Pi 5 + reverse proxy)

Production runs as a **single Docker container** on the Pi, bound to `127.0.0.1:8080`. A public server (nginx) or tunnel terminates TLS and forwards HTTP/WebSocket traffic.

## Quick start on Pi

```bash
# One-time host setup (Debian / Raspberry Pi OS)
sudo apt-get update
sudo apt-get install -y docker.io docker-compose-plugin curl git

sudo usermod -aG docker $USER
# log out and back in

sudo mkdir -p /opt/upjs-gdd
sudo chown upjs-gdd-deploy:upjs-gdd-deploy /opt/upjs-gdd   # if using deploy user
```

### Clone via SSH (deploy user)

`upjs-gdd-deploy` needs its **own** GitHub deploy key (do not reuse your personal `pi` user key).

On the Pi as root:

```bash
sudo -u upjs-gdd-deploy mkdir -p /home/upjs-gdd-deploy/.ssh
sudo chmod 700 /home/upjs-gdd-deploy/.ssh

sudo -u upjs-gdd-deploy ssh-keygen -t ed25519 -f /home/upjs-gdd-deploy/.ssh/id_ed25519_github -N "" -C "upjs-gdd-deploy@arianagrande"
sudo -u upjs-gdd-deploy cat /home/upjs-gdd-deploy/.ssh/id_ed25519_github.pub
```

Copy the printed public key → GitHub repo **Settings → Deploy keys → Add deploy key** (read-only is enough).

Pin GitHub’s host key and use the deploy key:

```bash
sudo -u upjs-gdd-deploy bash -c 'cat >> /home/upjs-gdd-deploy/.ssh/config << EOF
Host github.com
  HostName github.com
  User git
  IdentityFile ~/.ssh/id_ed25519_github
  IdentitiesOnly yes
EOF'
sudo chmod 600 /home/upjs-gdd-deploy/.ssh/config

sudo -u upjs-gdd-deploy bash -c 'ssh-keyscan github.com >> ~/.ssh/known_hosts'
sudo chmod 644 /home/upjs-gdd-deploy/.ssh/known_hosts
```

Test, then clone:

```bash
sudo -u upjs-gdd-deploy ssh -T git@github.com
# expect: "Hi matusem/ipel-gamedev! You've successfully authenticated..."

sudo -u upjs-gdd-deploy git clone git@github.com:matusem/ipel-gamedev.git /opt/upjs-gdd/ipel-gamedev
sudo -u upjs-gdd-deploy cp /opt/upjs-gdd/ipel-gamedev/framework/.env.example /opt/upjs-gdd/ipel-gamedev/framework/.env
```

Edit `GAMEDEV_IMAGE` and `HOST_PORT` in `.env` (default host bind `8090`), then:

```bash
cd /opt/upjs-gdd/ipel-gamedev/framework
sudo -u upjs-gdd-deploy docker compose pull
sudo -u upjs-gdd-deploy docker compose up -d
curl -s http://127.0.0.1:8090/health
```

### Clone via HTTPS (alternative)

```bash
git clone https://github.com/matusem/ipel-gamedev.git
cd ipel-gamedev/framework
cp .env.example .env
# edit GAMEDEV_IMAGE if needed

docker compose pull
docker compose up -d
curl -s http://127.0.0.1:8090/health
```

## Architecture

```
Internet → nginx (public server, TLS) → tunnel/LAN → Pi:127.0.0.1:8080 → upjs-gdd container
```

The app serves lobby SPA, GraphQL, game WebSockets, static games, CLI downloads (`/tools/`), and platform manifest (`/platform/manifest.json`) on one port.

## Reverse proxy

Copy [nginx/gdd.conf.example](nginx/gdd.conf.example) to your public server. Adjust:

- `server_name` — your domain
- `upstream` — tunnel endpoint or Pi LAN IP
- TLS certificate paths

Required: WebSocket upgrade on `/graphql` and `/game`, proxy read/send timeout ≥ 3600s.

## Tunnel options

| Method | Notes |
|--------|--------|
| **SSH reverse tunnel** | `ssh -R 8080:127.0.0.1:8080 user@public-server` — nginx upstream `127.0.0.1:8080` on public host |
| **WireGuard / Tailscale** | nginx upstream = Pi tailnet IP |
| **Cloudflare Tunnel** | Point ingress to `http://127.0.0.1:8080` on Pi |

## CI/CD autodeploy (signed webhook)

GitHub Actions workflows live at the **repository root** [`.github/workflows/`](../../.github/workflows/) (not under `framework/`):

1. On tag `v*` — **CLI Release** builds `gamedev-cli` for all platforms, updates manifests, and creates a GitHub Release
2. When CLI Release finishes — **Release and Deploy** reuses those CLI artifacts (no second compile), builds the `linux/arm64` image, pushes to GHCR
3. **POST** `https://<your-domain>/internal/deploy` with an Ed25519 signature — the running container pulls the new image and runs `docker compose up -d` on the host (via mounted Docker socket)

No SSH from GitHub to the Pi is required. The webhook must be reachable over HTTPS (e.g. through your nginx → tunnel → Pi path).

### 1. Generate keys

On any machine with Python:

```bash
pip install pynacl
python3 framework/scripts/generate-deploy-keys.py
```

- **Pi `.env`:** `DEPLOY_WEBHOOK_PUBLIC_KEY=<public>`
- **GitHub secret:** `DEPLOY_WEBHOOK_PRIVATE_KEY=<private>`

### 2. Pi compose mounts

`docker-compose.yml` mounts the host Docker socket and compose directory (read-only) so the app can restart itself:

```yaml
- /var/run/docker.sock:/var/run/docker.sock
- /opt/upjs-gdd/ipel-gamedev/framework:/deploy:ro
```

Add to Pi `.env`:

```env
DEPLOY_WEBHOOK_PUBLIC_KEY=<base64 public key>
DEPLOY_COMPOSE_DIR=/deploy
```

`upjs-gdd-deploy` must be in the `docker` group (host Docker access).

### 3. GitHub secrets

| Secret | Purpose |
|--------|---------|
| `DEPLOY_WEBHOOK_URL` | Public base URL, e.g. `https://gdd.ics.upjs.sk` |
| `DEPLOY_WEBHOOK_PRIVATE_KEY` | Base64 Ed25519 signing key from keygen script |

### 4. Manual trigger (debug)

```bash
export DEPLOY_WEBHOOK_URL=https://gdd.ics.upjs.sk
export DEPLOY_WEBHOOK_PRIVATE_KEY=<private>
python3 framework/scripts/trigger-deploy-webhook.py \
  --image ghcr.io/matusem/ipel-gamedev \
  --tag 0.1.1
```

Request format:

- `POST /internal/deploy`
- Headers: `X-Deploy-Timestamp` (unix seconds), `X-Deploy-Signature` (base64 Ed25519 of `timestamp + "\n" + body`)
- Body: `{"image":"ghcr.io/matusem/ipel-gamedev","tag":"0.1.1"}`

Returns `202 Accepted` immediately; compose runs in the background (~30–90s until `/health` recovers).

CLI-only updates without a platform redeploy: push tag `gamedev-cli-v*` instead.

### GHCR container image

The server image is **not** attached to GitHub Releases. It is pushed to GitHub Container Registry:

| Pull | Image |
|------|--------|
| Latest | `ghcr.io/matusem/ipel-gamedev:latest` |
| Version tag | `ghcr.io/matusem/ipel-gamedev:0.1.1` (no `v` prefix) |

**Find it in the UI (must be logged into GitHub):**

- Repo packages: `https://github.com/matusem/ipel-gamedev/pkgs/container/ipel-gamedev`
- Your account: **Profile → Packages**

A **404** on the packages URL usually means one of:

1. **Not signed in** — GHCR packages are private by default; GitHub returns 404 instead of “login required”.
2. **Release and Deploy failed** — check **Actions → Release and Deploy** for the tag; `build-image` must be green.
3. **Package not public yet** — open the package → **Package settings → Change visibility → Public** (optional, for anonymous `docker pull`).

**Pull on the Pi (private package):**

```bash
# PAT needs read:packages (classic) or packages:read (fine-grained)
echo "$GITHUB_TOKEN" | docker login ghcr.io -u matusem --password-stdin
docker pull ghcr.io/matusem/ipel-gamedev:0.1.1
```

**Prebuilt CI builder (`ipel-gamedev-builder`):** release builds use `ghcr.io/matusem/ipel-gamedev-builder:bookworm`. Linking the package to the repo is not enough for private images pushed from your laptop — also do **one** of:

1. **Package settings → Manage Actions access** → add `matusem/ipel-gamedev` with **Read**
2. **Change visibility → Public** (fine for tooling; no secrets in the image)
3. Add repo secret **`GHCR_TOKEN`** = classic PAT with `read:packages`

Push **linux/arm64** (`docker buildx build --platform linux/arm64 ... --push`).

## Backup & restore

Daily backup (cron on Pi):

```bash
0 3 * * * /opt/upjs-gdd/ipel-gamedev/framework/scripts/backup.sh /var/backups/upjs-gdd
```

Restore:

```bash
./scripts/restore.sh /var/backups/upjs-gdd/upjs-gdd-YYYYMMDD-HHMMSS.tar.gz
docker compose up -d
```

## Raspberry Pi notes

- **16 GB RAM** is comfortable for course-scale traffic (~50 concurrent games per requirements).
- Build images in CI (`linux/arm64`); do not compile the full image on the Pi unless necessary.
- Image is multi-stage Rust + Dioxus; first deploy uses prebuilt GHCR artifact.

## Local smoke test

```bash
docker build -t upjs-gdd:local .
docker run --rm -p 8080:8080 -v upjs-gdd-data:/app/data -v upjs-gdd-games:/app/games upjs-gdd:local
```
