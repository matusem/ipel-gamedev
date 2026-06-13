# SDK publishing guide

Public registry publishing is **optional** — the platform-hosted CLI manifest (`/platform/manifest.json`) is the compatibility source of truth. Registries are convenience channels for students.

## Version alignment

Bump together on each platform release:

| Artifact | Location |
|----------|----------|
| Platform manifest | `platform/manifest.json` |
| CLI manifest | `tools/gamedev-cli/manifest.json` |
| CLI constants | `tools/gamedev-cli/src/version.rs` |
| Rust SDK crates | `sdk/rust/*/Cargo.toml` |
| JS packages | `sdk/js/package.json`, `packages/game-sdk/package.json` |
| Java library | `sdk/java/game/build.gradle.kts` |

Run `scripts/update-cli-manifest.py` after CLI release artifacts are built.

## Rust (crates.io)

Crates (publish in dependency order):

1. `upjs-gdd-shared-types`
2. `upjs-gdd-rust-shared`
3. `upjs-gdd-bevy`, `upjs-gdd-dioxus`

Each `Cargo.toml` includes `license`, `description`, `repository`, and `readme`. Replace `path =` workspace deps with semver versions before `cargo publish`.

Use [Trusted Publishing](https://blog.rust-lang.org/2025/04/04/crates-io-trusted-publishing/) (OIDC) in GitHub Actions instead of long-lived tokens.

## npm

Packages:

| Package | Path | Scope |
|---------|------|-------|
| `@upjs-gdd/game-sdk` | `packages/game-sdk/` | browser iframe SDK |
| `@upjs-gdd/sdk-js` | `sdk/js/` | GraphQL/dev tooling |

Enable [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/) on each package. Use Node.js 24+ in CI for OIDC.

```bash
cd packages/game-sdk && npm publish --access public
```

## Java (Maven Central)

Library: `sk.upjs.gdd:game:0.1.0` from `sdk/java/game/`.

1. Register namespace at [central.sonatype.com](https://central.sonatype.com/)
2. Configure signing + `maven-publish` / Vanniktech plugin (see `sdk/java/game/build.gradle.kts` publishing block)
3. Publish with `./gradlew publishAndReleaseToMavenCentral`

The TeaVM `component-template` stays in-repo; games use composite builds until a separate template artifact is published.

## Package managers (CLI)

Templates in `packaging/`:

| Channel | Path | Notes |
|---------|------|-------|
| Homebrew tap | `packaging/homebrew/gamedev-cli.rb` | Update URL + sha256 per release |
| Winget | `packaging/winget/` | First submission manual; then `wingetcreate update` |
| Scoop bucket | `packaging/scoop/gamedev-cli.json` | Point bucket at your GitHub org |
| Chocolatey | `packaging/chocolatey/` | Community moderation on first publish |
| Linux `.deb` | `cargo deb` metadata in `tools/gamedev-cli/Cargo.toml` | Optional native packages |

Prefer platform install scripts first:

```powershell
irm https://gdd.ics.upjs.sk/tools/gamedev-cli/install.ps1 | iex
```

```bash
curl -fsSL https://gdd.ics.upjs.sk/tools/gamedev-cli/install.sh | bash
```
