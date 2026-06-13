# Third-party package manager templates

Update `REPLACE_WITH_SHA256` and version fields after each `gamedev-cli-v*` release.

| Directory | Target |
|-----------|--------|
| `homebrew/` | `brew tap <org>/homebrew-tap && brew install gamedev-cli` |
| `winget/` | Submit to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) |
| `scoop/` | Custom bucket manifest |
| `chocolatey/` | `choco pack` + `choco push` |

Primary install path remains platform-hosted scripts:

- `/tools/gamedev-cli/install.ps1`
- `/tools/gamedev-cli/install.sh`

See [docs/sdk-publishing.md](../docs/sdk-publishing.md).
