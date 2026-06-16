# Dioxus lobby dev server on :8080 (proxies GraphQL/games to backend on :8081).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$lobby = Join-Path $root "lobby"
Set-Location $lobby

if (-not (Test-Path "node_modules")) {
    Write-Host "Installing lobby CSS tooling (npm ci)..."
    npm ci
}
if (-not (Test-Path "assets/tailwind.css")) {
    Write-Host "Building Tailwind CSS..."
    npm run build:css
}

Write-Host ""
Write-Host "Starting lobby at http://127.0.0.1:8080"
Write-Host "Backend must run separately on :8081 - from framework/: .\scripts\dev-backend.ps1"
Write-Host ""

dx serve --platform web
