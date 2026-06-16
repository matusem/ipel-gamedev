# gamedev-cli

Developer CLI for the UPJŠ GDD Platform. See [docs/cli-distribution.md](../../docs/cli-distribution.md).

## Build profiles

| Build | Command | Default server |
|-------|---------|----------------|
| Local dev | `cargo run -p gamedev-cli` | `http://localhost:8080` |
| Release / CI | `cargo build -p gamedev-cli --features packaged` | `https://gamedev.jinxwashere.com` |

Both builds ship `local` and `prod` profiles in `config.toml`; use `--profile local` on a packaged binary to target localhost.
