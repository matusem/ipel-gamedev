# Build a framework-compatible game zip (manifest.json, logic.wasm, client/*).
# Requires: Rust + wasm32-wasip1, cargo-component, Node/npm.
#
# Usage:
#   .\build-game-zip.ps1 build              # build only (default)
#   .\build-game-zip.ps1 deploy             # build + upload + publishGameDraft on local server
# Optional: -OutZip path -ServerUrl http://localhost:8080 -AuthToken <user-uuid>
# Token fallback: $env:GAME_UPLOAD_BEARER or $env:GAMEDEV_AUTH_TOKEN (register in lobby first).

param(
    [Parameter(Position = 0)]
    [ValidateSet("build", "deploy")]
    [string]$Action = "build"
)
$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$GameDir = Split-Path -Parent $ScriptDir
Write-Host "[deprecated] This script now forwards to gamedev-cli."
if ($Action -eq "build") {
    cargo run -p gamedev-cli -- build --project-dir "$GameDir"
} else {
    cargo run -p gamedev-cli -- deploy --project-dir "$GameDir" --auto-publish
}
