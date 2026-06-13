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

Edit `GAMEDEV_IMAGE` in `.env`, then:

```bash
cd /opt/upjs-gdd/ipel-gamedev/framework
sudo -u upjs-gdd-deploy docker compose pull
sudo -u upjs-gdd-deploy docker compose up -d
curl -s http://127.0.0.1:8080/health
```

### Clone via HTTPS (alternative)

```bash
git clone https://github.com/matusem/ipel-gamedev.git
cd ipel-gamedev/framework
cp .env.example .env
# edit GAMEDEV_IMAGE if needed

docker compose pull
docker compose up -d
curl -s http://127.0.0.1:8080/health
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

## CI/CD autodeploy

GitHub Actions workflows live at the **repository root** [`.github/workflows/`](../../.github/workflows/) (not under `framework/`):

1. On tag `v*` or manual dispatch — build `linux/arm64` image, push to GHCR
2. SSH to deploy host — `docker compose pull && docker compose up -d`

### Required GitHub secrets

| Secret | Purpose |
|--------|---------|
| `DEPLOY_HOST` | SSH hostname or IP (reachable from Actions via tunnel/tailnet) |
| `DEPLOY_USER` | e.g. `upjs-gdd-deploy` |
| `DEPLOY_SSH_KEY` | Private key for deploy user |
| `DEPLOY_PATH` | Path to `framework/` on host (default `/opt/upjs-gdd/ipel-gamedev/framework`) |

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
