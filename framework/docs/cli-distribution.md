# CLI distribution plan (`gamedev-cli`)

Notes for hosting download links on the Platform, version checks against production, and optional self-update.

**Status:** implemented. Platform `v*` releases bake real CLI checksums and binaries into the production image via `scripts/package-cli-image-bundle.py`. CLI-only tags use `gamedev-cli-v*`.

---

## Recommended model

Treat the CLI as a **first-class platform artifact**, separate from game uploads:

```
Production (gdd.ics.upjs.sk)
├── /tools/gamedev-cli/manifest.json     ← version + per-OS download URLs + checksums
├── /tools/gamedev-cli/v0.2.0/…          ← binaries (or zip per platform)
└── Lobby → Developer page               ← “Download CLI” links from manifest
```

The CLI should talk to the **same origin** it deploys to (configurable; default = production URL).

---

## 1. Version manifest (source of truth)

Small JSON file served by the server (or embedded in the lobby build from CI):

```json
{
  "version": "0.2.0",
  "min_supported": "0.1.0",
  "released_at": "2026-06-12",
  "assets": {
    "windows-x86_64": {
      "url": "https://gdd.ics.upjs.sk/tools/gamedev-cli/v0.2.0/gamedev-cli-windows-x86_64.zip",
      "sha256": "…"
    },
    "linux-x86_64": { },
    "macos-aarch64": { }
  },
  "notes": "…"
}
```

**Endpoint options** (pick one primary; mirror in GraphQL if useful):

| Option | Pros |
|--------|------|
| `GET /tools/gamedev-cli/manifest.json` | Simple, cacheable, works before login |
| GraphQL `platformCliRelease` | Fits existing API; easy to gate later |

Production deployment **is** the version source — no separate update server. Bump manifest when the platform image is released.

---

## 2. Platform download UI

The Developer page (`lobby/src/pages/developer.rs`) lists real `gamedev-cli` commands with clipboard copy. Remaining work for distribution:

- Detect OS → show primary download button
- Links for other platforms
- Show installed vs latest version (requires `gamedev-cli --version`)
- Copy-paste install one-liners

Example Windows install:

```powershell
irm https://gdd.ics.upjs.sk/tools/gamedev-cli/install.ps1 | iex
```

Install script reads `manifest.json`, downloads the matching zip, verifies `sha256`, installs to e.g. `%LOCALAPPDATA%\gamedev-cli\bin` and updates `PATH`.

---

## 3. CLI version check + self-update

**On every run** (cheap, non-blocking):

```
gamedev-cli deploy
  → fetch manifest (short timeout)
  → if remote > local: print warning
     "CLI 0.1.0 is outdated; latest 0.2.0 — run: gamedev-cli update"
```

**Explicit commands:**

```
gamedev-cli --version
gamedev-cli update          # download + replace self
gamedev-cli update --check  # exit 1 if outdated (CI-friendly)
```

**Rust implementation sketch:**

- Embed version via `env!("CARGO_PKG_VERSION")`
- Compare with `semver` crate
- Download with `reqwest` (already a dependency)
- Replace binary: `self_update` crate or write `.new` + rename (Windows-safe)

**Policy (university / lab environment):**

| Behavior | Recommendation |
|----------|----------------|
| Notify when outdated | Yes |
| Auto-update without prompt | No |
| Block deploy if CLI too old | Optional later via `min_supported` in manifest |

---

## 4. Build & release pipeline

### Platform release (`v*` tag)

Workflow: `.github/workflows/release-deploy.yml`

1. Build `gamedev-cli` for all platforms (matrix in `reusable-build-cli.yml`)
2. `scripts/package-cli-image-bundle.py pack` — update manifests, stage archives, produce `cli-image-bundle.tar.gz`
3. `scripts/verify-cli-image-bundle.py` — assert five archives, real checksums, matching URLs
4. Create GitHub Release with raw CLI archives
5. Extract bundle into Docker build context; build and push `linux/arm64` image
6. Verify `/app/tools/gamedev-cli/v<version>/` exists inside the image
7. Deploy via signed webhook

### CLI-only release (`gamedev-cli-v*` tag)

Workflow: `.github/workflows/cli-release.yml`

Builds and publishes CLI binaries to GitHub Releases without redeploying the platform image.

### Caching

Rust (`Swatinem/rust-cache`), npm (`setup-node`), Maven, Gradle, Docker BuildKit (GHA + cargo/npm mounts), apt archives for cross-compilers, pip for deploy signing, and version-keyed `~/.cargo/bin` for installed tools.

---

## 5. What to avoid (for this audience)

| Approach | Why skip for now |
|----------|------------------|
| `cargo install` only | Requires full Rust toolchain for game devs |
| Crates.io publish | Same; awkward coupling to platform version |
| Silent auto-update | Surprising in lab / AV environments |
| CLI bundled inside game runtime Docker | Wrong lifecycle — CLI is dev-machine tooling |

Keep `cargo run -p gamedev-cli` documented for **framework contributors** only.

---

## 6. Phased rollout

### Phase 1 — Distribution (high value, low risk)

- `gamedev-cli --version`
- Release workflow + static hosting under `/tools/gamedev-cli/`
- Lobby download section + `install.ps1` / `install.sh`
- Docs: “Download CLI” as default student onboarding

### Phase 2 — Version awareness

- Manifest fetch on startup or in `gamedev-cli doctor`
- Outdated warning banner

### Phase 3 — Self-update

- `gamedev-cli update` with checksum verification
- Optional `min_supported` enforcement on `uploadGameZip`

---

## 7. Security basics

- **SHA256** in manifest; CLI verifies before replacing binary
- **HTTPS only** for downloads
- Optional later: sign manifests or binaries
- CLI download can be **public**; uploads still require `gamedev-cli login` / publish token

---

## 8. Current codebase anchors

| Area | Path |
|------|------|
| CLI crate | `tools/gamedev-cli/` (version `0.1.0` in `Cargo.toml`) |
| Server routes | `server/src/main.rs` — no `/tools` yet |
| Developer UI | `lobby/src/pages/developer.rs` — real CLI commands + clipboard copy |
| Game upload API | GraphQL `uploadGameZip` (used by `deploy`) |
| Production target | `docs/requirements.md` — `https://gdd.ics.upjs.sk/` |

---

## Summary

Platform-hosted download links + production `manifest.json` + CLI `update` is the right shape for `gdd.ics.upjs.sk`. Implement Phase 1 first (artifacts, `/tools/` route, lobby links, `--version`), then version checks, then self-update.
