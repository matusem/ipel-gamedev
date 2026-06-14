param(
  [string]$Platform = "https://gamedev.jinxwashere.com",
  [string]$InstallDir = "$env:LOCALAPPDATA\gamedev-cli\bin"
)

$ErrorActionPreference = "Stop"

$manifestUrl = "$Platform/tools/gamedev-cli/manifest.json"
Write-Host "Fetching $manifestUrl"
$manifest = Invoke-RestMethod -Uri $manifestUrl
$key = "windows-x86_64"
$asset = $manifest.assets.$key
if (-not $asset) { throw "No asset for $key in manifest" }

$downloadUrl = if ($asset.url -match '^https?://') { $asset.url } else { "$Platform$($asset.url)" }
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$zipPath = Join-Path $env:TEMP "gamedev-cli.zip"
Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath
$hash = (Get-FileHash $zipPath -Algorithm SHA256).Hash.ToLower()
if ($hash -ne $asset.sha256.ToLower()) {
  throw "Checksum mismatch: expected $($asset.sha256) got $hash"
}

Expand-Archive -Path $zipPath -DestinationPath $InstallDir -Force
$exe = Join-Path $InstallDir "gamedev.exe"
if (-not (Test-Path $exe)) { throw "gamedev.exe not found after extract" }

# Shim so both `gamedev` and `gamedev-cli` work on Windows (matches Linux install.sh).
$shim = Join-Path $InstallDir "gamedev-cli.cmd"
Set-Content -Path $shim -Encoding ascii -Value "@echo off`r`n""%~dp0gamedev.exe"" %*"

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$InstallDir*") {
  [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
  Write-Host "Added $InstallDir to user PATH (restart terminal)"
}

Write-Host "Installed gamedev-cli $($manifest.version) to $exe"
