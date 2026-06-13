# Deployment (Raspberry Pi 5 + reverse proxy)

Production runs as a **single Docker container** on the Pi, bound to `127.0.0.1:8080`. A public server (nginx) or tunnel terminates TLS and forwards HTTP/WebSocket traffic.

## Quick start on Pi

```bash
# One-time host setup (Debian / Raspberry Pi OS)
sudo apt-get update
sudo apt-get install -y docker.io docker-compose-plugin curl

sudo usermod -aG docker $USER
# log out and back in

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

GitHub Actions workflow [`.github/workflows/release-deploy.yml`](../.github/workflows/release-deploy.yml):

1. On tag `v*` or manual dispatch — build `linux/arm64` image, push to GHCR
2. SSH to deploy host — `docker compose pull && docker compose up -d`

### Required GitHub secrets

| Secret | Purpose |
|--------|---------|
| `DEPLOY_HOST` | SSH hostname or IP (reachable from Actions via tunnel/tailnet) |
| `DEPLOY_USER` | e.g. `upjs-gdd-deploy` |
| `DEPLOY_SSH_KEY` | Private key for deploy user |
| `DEPLOY_PATH` | Path to `framework/` on host (default `/opt/upjs-gdd/framework`) |

## Backup & restore

Daily backup (cron on Pi):

```bash
0 3 * * * /opt/upjs-gdd/framework/scripts/backup.sh /var/backups/upjs-gdd
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
