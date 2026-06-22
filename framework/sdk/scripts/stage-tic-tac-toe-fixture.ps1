# Build tic_tac_toe component and stage under server/tests/fixtures for integration tests.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$GameDir = Join-Path $Root "games\tic_tac_toe"
$BotDir = Join-Path $Root "games\tic_tac_toe_bot"
$FixtureDir = Join-Path $Root "server\tests\fixtures\games\tic_tac_toe"
$BotFixtureDir = Join-Path $Root "server\tests\fixtures\bots\tic_tac_toe_bot"

Push-Location $Root
try {
    cargo run -p tic_tac_toe --features schemars --bin export_schema
}
finally {
    Pop-Location
}

Push-Location (Join-Path $GameDir "rust\component")
try {
    cargo component build --release
    $Wasm = Join-Path $Root "target\wasm32-wasip1\release\tic_tac_toe_component.wasm"
    if (-not (Test-Path $Wasm)) {
        throw "Missing component output: $Wasm"
    }
    New-Item -ItemType Directory -Force -Path (Join-Path $FixtureDir "client") | Out-Null
    Copy-Item -Force $Wasm (Join-Path $FixtureDir "logic.wasm")
    Copy-Item -Force (Join-Path $GameDir "manifest.json") (Join-Path $FixtureDir "manifest.json")
    Copy-Item -Force (Join-Path $GameDir "contract.json") (Join-Path $FixtureDir "contract.json")
    foreach ($html in @("index.html", "config.html", "result.html", "about.html")) {
        $src = Join-Path $GameDir "client\$html"
        if (Test-Path $src) {
            Copy-Item -Force $src (Join-Path $FixtureDir "client\$html")
        }
    }
    Write-Host "Staged game fixture at $FixtureDir"
}
finally {
    Pop-Location
}

Push-Location (Join-Path $BotDir "rust\component")
try {
    cargo component build --release
    $BotWasm = Join-Path $Root "target\wasm32-wasip1\release\tic_tac_toe_bot_component.wasm"
    if (-not (Test-Path $BotWasm)) {
        throw "Missing bot component output: $BotWasm"
    }
    New-Item -ItemType Directory -Force -Path $BotFixtureDir | Out-Null
    Copy-Item -Force $BotWasm (Join-Path $BotFixtureDir "bot.wasm")
    Copy-Item -Force (Join-Path $BotDir "manifest.json") (Join-Path $BotFixtureDir "manifest.json")
    Copy-Item -Force (Join-Path $BotDir "contract\contract.json") (Join-Path $BotFixtureDir "contract.json")
    Write-Host "Staged bot fixture at $BotFixtureDir"
}
finally {
    Pop-Location
}
