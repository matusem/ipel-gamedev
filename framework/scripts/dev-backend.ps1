# Start the Actix backend on port 8081 (for use with `dx serve` on :8080).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# Load .env if present (PORT in .env should be 8081 for local dev)
$envFile = Join-Path $root ".env"
if (Test-Path $envFile) {
    Get-Content $envFile | ForEach-Object {
        $line = $_.Trim()
        if ($line -eq "" -or $line.StartsWith("#")) { return }
        $idx = $line.IndexOf("=")
        if ($idx -lt 1) { return }
        $key = $line.Substring(0, $idx).Trim()
        $val = $line.Substring($idx + 1).Trim()
        if ($val.Length -ge 2 -and $val.StartsWith('"') -and $val.EndsWith('"')) {
            $val = $val.Substring(1, $val.Length - 2)
        }
        Set-Item -Path "env:$key" -Value $val
    }
}

if (-not $env:PORT) { $env:PORT = "8081" }
Write-Host "Starting UPJŠ GDD Platform backend on http://127.0.0.1:$($env:PORT) ..."
Write-Host "Keep this window open. In another terminal: cd lobby && dx serve --platform web"
Write-Host ""

cargo run -p server
