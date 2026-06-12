# Build tic_tac_toe component and stage under server/tests/fixtures/games for integration tests.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$GameDir = Join-Path $Root "games\tic_tac_toe"
$FixtureDir = Join-Path $Root "server\tests\fixtures\games\tic_tac_toe"

Push-Location (Join-Path $GameDir "rust\component")
try {
    cargo component build --release
    $Wasm = Join-Path (Get-Location) "target\wasm32-wasip2\release\tic_tac_toe_component.wasm"
    if (-not (Test-Path $Wasm)) {
        throw "Missing component output: $Wasm"
    }
    New-Item -ItemType Directory -Force -Path (Join-Path $FixtureDir "client") | Out-Null
    Copy-Item -Force $Wasm (Join-Path $FixtureDir "logic.wasm")
    Copy-Item -Force (Join-Path $GameDir "manifest.json") (Join-Path $FixtureDir "manifest.json")
    foreach ($html in @("index.html", "config.html", "result.html", "about.html")) {
        $src = Join-Path $GameDir "client\$html"
        if (Test-Path $src) {
            Copy-Item -Force $src (Join-Path $FixtureDir "client\$html")
        }
    }
    Write-Host "Staged fixture at $FixtureDir"
}
finally {
    Pop-Location
}
