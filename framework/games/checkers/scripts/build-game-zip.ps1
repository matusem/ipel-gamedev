# Build a framework-compatible game zip (manifest.json, logic.wasm, client/*).
# Requires: Rust + wasm32-wasip1 + wasm32-unknown-unknown, cargo-component,
#           wasm-bindgen on PATH (cargo install wasm-bindgen-cli --locked).
#
# Usage:
#   .\build-game-zip.ps1 build              # build only (default)
#   .\build-game-zip.ps1 deploy             # build + upload + publishGameDraft on local server
# Optional: -OutZip path -ServerUrl http://localhost:8080 -AuthToken <user-uuid>
# Token fallback: $env:GAME_UPLOAD_BEARER or $env:GAMEDEV_AUTH_TOKEN (register in lobby first).

param(
    [Parameter(Position = 0)]
    [ValidateSet("build", "deploy")]
    [string]$Action = "build",
    [string]$OutZip = "",
    [string]$ServerUrl = "http://localhost:8080",
    [string]$AuthToken = ""
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$GameDir = Split-Path -Parent $ScriptDir

if (-not $OutZip) {
    $OutZip = Join-Path $GameDir "dist\checkers.zip"
}

function Invoke-CheckersZipBuild {
    $wg = Get-Command wasm-bindgen -ErrorAction SilentlyContinue
    if (-not $wg) {
        throw "wasm-bindgen not found. Install: cargo install wasm-bindgen-cli --locked"
    }

    Write-Host "==> Game dir:   $GameDir"
    Write-Host "==> Output zip: $OutZip"

    Push-Location $GameDir
    try {
        $prevEa = $ErrorActionPreference
        $ErrorActionPreference = "SilentlyContinue"
        rustup target add wasm32-wasip1 wasm32-unknown-unknown | Out-Null
        $ErrorActionPreference = $prevEa
        cargo component build -p checkers_component --release
        cargo build -p checkers_web --release --target wasm32-unknown-unknown

        $WasmBin = Join-Path $GameDir "target\wasm32-unknown-unknown\release\checkers_web.wasm"
        $LogicSrc = Join-Path $GameDir "target\wasm32-wasip1\release\checkers_component.wasm"
        if (-not (Test-Path $LogicSrc)) { throw "Missing $LogicSrc" }
        if (-not (Test-Path $WasmBin)) { throw "Missing $WasmBin" }

        & wasm-bindgen $WasmBin --out-dir (Join-Path $GameDir "client") --target web --no-typescript
    } finally {
        Pop-Location
    }

    $Stage = Join-Path ([System.IO.Path]::GetTempPath()) ("gamezip-" + [Guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Path $Stage | Out-Null
    try {
        Copy-Item (Join-Path $GameDir "manifest.json") $Stage
        Copy-Item (Join-Path $GameDir "target\wasm32-wasip1\release\checkers_component.wasm") (Join-Path $Stage "logic.wasm")
        Copy-Item (Join-Path $GameDir "client") (Join-Path $Stage "client") -Recurse

        foreach ($f in @("index.html", "config.html", "result.html")) {
            if (-not (Test-Path (Join-Path $Stage "client\$f"))) {
                throw "Missing client\$f"
            }
        }

        $OutDir = Split-Path -Parent $OutZip
        if ($OutDir -and -not (Test-Path $OutDir)) {
            New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
        }
        if (Test-Path $OutZip) { Remove-Item $OutZip -Force }

        Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $OutZip -Force
    } finally {
        Remove-Item $Stage -Recurse -Force -ErrorAction SilentlyContinue
    }

    Write-Host "==> Wrote $OutZip"
}

function Invoke-GameZipUploadAndPublish {
    param(
        [string]$ZipPath,
        [string]$GraphqlUrl,
        [hashtable]$Headers
    )

    if (-not (Test-Path -LiteralPath $ZipPath)) {
        throw "Zip not found: $ZipPath"
    }

    Write-Host "==> Uploading to $GraphqlUrl"

    $bytes = [System.IO.File]::ReadAllBytes($ZipPath)
    $b64 = [Convert]::ToBase64String($bytes)
    $filename = [System.IO.Path]::GetFileName($ZipPath)

    $query = @'
mutation UploadGameZip($filename: String!, $zipBase64: String!) {
  uploadGameZip(filename: $filename, zipBase64: $zipBase64) {
    uploadId
    report {
      ok
      errors
      warnings
      diagnostics {
        severity
        code
        message
      }
    }
    draft {
      id
      gameName
      version
      status
    }
  }
}
'@

    $payload = [ordered]@{
        query     = $query
        variables = [ordered]@{
            filename  = $filename
            zipBase64 = $b64
        }
    }
    $json = $payload | ConvertTo-Json -Depth 8 -Compress

    try {
        $resp = Invoke-RestMethod -Uri $GraphqlUrl -Method Post -Headers $Headers -Body $json
    } catch {
        throw "Upload request failed: $($_.Exception.Message)"
    }

    if ($resp.errors) {
        $msg = ($resp.errors | ConvertTo-Json -Depth 6 -Compress)
        throw "GraphQL errors: $msg"
    }

    $r = $resp.data.uploadGameZip
    if (-not $r) {
        throw "Unexpected GraphQL response (no data.uploadGameZip): $($resp | ConvertTo-Json -Depth 6)"
    }

    Write-Host "==> uploadId: $($r.uploadId)"
    if ($r.draft) {
        Write-Host "==> draft: $($r.draft.gameName) $($r.draft.version) ($($r.draft.status)) id=$($r.draft.id)"
    } else {
        Write-Host "==> draft: (none)"
    }

    $rep = $r.report
    Write-Host "==> validation ok=$($rep.ok) errors=$($rep.errors) warnings=$($rep.warnings)"
    foreach ($d in @($rep.diagnostics)) {
        Write-Host "    [$($d.severity)] $($d.code): $($d.message)"
    }

    if (-not $rep.ok) {
        throw "Upload stored but validation failed (see diagnostics above). Publish may be blocked until fixed."
    }
    if (-not $r.draft -or -not $r.draft.id) {
        throw "Upload validated but no draft id returned (cannot publish)."
    }

    $draftId = [string]$r.draft.id
    Write-Host "==> Publishing draft $draftId"

    $pubQuery = @'
mutation PublishGameDraft($draftId: ID!) {
  publishGameDraft(draftId: $draftId) {
    id
    gameName
    version
    status
    publishedAt
  }
}
'@

    $pubPayload = [ordered]@{
        query     = $pubQuery
        variables = [ordered]@{ draftId = $draftId }
    }
    $pubJson = $pubPayload | ConvertTo-Json -Depth 8 -Compress

    try {
        $pubResp = Invoke-RestMethod -Uri $GraphqlUrl -Method Post -Headers $Headers -Body $pubJson
    } catch {
        throw "Publish request failed: $($_.Exception.Message)"
    }

    if ($pubResp.errors) {
        $msg = ($pubResp.errors | ConvertTo-Json -Depth 6 -Compress)
        throw "GraphQL errors (publish): $msg"
    }

    $p = $pubResp.data.publishGameDraft
    if (-not $p) {
        throw "Unexpected GraphQL response (no data.publishGameDraft): $($pubResp | ConvertTo-Json -Depth 6)"
    }

    Write-Host "==> Published: $($p.gameName) $($p.version) status=$($p.status) publishedAt=$($p.publishedAt)"
}

Invoke-CheckersZipBuild

if ($Action -eq "deploy") {
    $token = $AuthToken
    if (-not $token) { $token = $env:GAME_UPLOAD_BEARER }
    if (-not $token) { $token = $env:GAMEDEV_AUTH_TOKEN }
    if (-not $token) {
        throw @"
deploy requires a registered user id as Bearer token.
  - Pass -AuthToken '<uuid>' from the lobby after registerUser, or
  - Set `$env:GAME_UPLOAD_BEARER or `$env:GAMEDEV_AUTH_TOKEN
Server must be running (default $ServerUrl). Uses POST /graphql: uploadGameZip, then publishGameDraft.
"@
    }

    $graphqlUrl = ($ServerUrl.TrimEnd("/") + "/graphql")
    $headers = @{
        Authorization = "Bearer $($token.Trim())"
        "Content-Type" = "application/json; charset=utf-8"
    }

    Invoke-GameZipUploadAndPublish -ZipPath $OutZip -GraphqlUrl $graphqlUrl -Headers $headers
}
