param(
    [Parameter(Position = 0)]
    [ValidateSet("build", "deploy")]
    [string]$Action = "build"
)
$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$GameDir = Split-Path -Parent $ScriptDir
$FrameworkDir = Split-Path -Parent (Split-Path -Parent $GameDir)
Write-Host "[deprecated] This script now forwards to gamedev-cli."
if ($Action -eq "build") {
    cargo run -p gamedev-cli -- build --project-dir "$GameDir"
} else {
    cargo run -p gamedev-cli -- deploy --project-dir "$GameDir" --auto-publish
}
