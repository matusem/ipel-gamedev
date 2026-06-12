# CLI distribution plan (`gamedev-cli`)

Notes for hosting download links on the Platform, version checks against production, and optional self-update.

**Status:** planned (not implemented). Today devs run `cargo run -p gamedev-cli` from the framework repo. The lobby Developer page shows real CLI commands (copy via clipboard) aligned with `gamedev-cli` subcommands; hosted download manifest is still future work.

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

GitHub Actions job on tag `gamedev-cli-v*`:

1. `cargo build --release -p gamedev-cli` for:
   - `x86_64-pc-windows-msvc`
   - `x86_64-unknown-linux-gnu`
   - `aarch64-apple-darwin` (and/or `x86_64-apple-darwin`)
2. Zip each binary + short README
3. Compute `sha256`
4. Publish artifacts:
   - **Option A (preferred for single-container deploy):** copy into Docker image at `/app/tools/gamedev-cli/`; manifest updated at image build time
   - **Option B:** GitHub Releases; manifest URLs point there (simpler CI, worse if GitHub is blocked on campus)

Current gap: `.github/workflows/ci.yml` tests only; no release job. `Dockerfile` builds server + lobby, not `gamedev-cli`.

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
