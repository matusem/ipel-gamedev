# Start the Actix backend on port 8081 (for use with `dx serve` on :8080).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$env:PORT = "8081"
Write-Host "Starting IPEL GameDev backend on http://127.0.0.1:8081 ..."
Write-Host "Keep this window open. In another terminal: cd lobby && dx serve --platform web"
Write-Host ""

cargo run -p server
