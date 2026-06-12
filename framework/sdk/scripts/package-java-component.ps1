# Embed a TeaVM core Wasm module into a WebAssembly Component for game-core WIT.
# Requires wasm-tools on PATH (pin: see sdk/java/WIT_VERSIONS.md).
param(
    [Parameter(Mandatory = $true)][string]$CoreWasm,
    [Parameter(Mandatory = $true)][string]$WitFile,
    [Parameter(Mandatory = $true)][string]$OutWasm
)

$ErrorActionPreference = "Stop"
if (-not (Test-Path $CoreWasm)) { throw "missing core wasm: $CoreWasm" }
if (-not (Test-Path $WitFile)) { throw "missing wit: $WitFile" }
$outDir = Split-Path -Parent $OutWasm
if ($outDir -and -not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir | Out-Null }
$EmbeddedWasm = Join-Path $outDir "logic-embedded.wasm"
$Adapter = Join-Path (Split-Path $CoreWasm -Parent) "..\wasm\wasi_snapshot_preview1.reactor.wasm"
if (-not (Test-Path $Adapter)) {
    $Adapter = Join-Path (Split-Path $CoreWasm -Parent) "..\..\wasm\wasi_snapshot_preview1.reactor.wasm"
}
if (-not (Test-Path $Adapter)) {
    throw "missing WASI adapter (see component-template/wasm/README.md): $Adapter"
}
wasm-tools component embed --world game-core $WitFile $CoreWasm -o $EmbeddedWasm
$TeavmAdapter = Join-Path (Split-Path $Adapter -Parent) "teavm_ascii.wasm"
if (-not (Test-Path $TeavmAdapter)) { throw "missing TeaVM adapter: $TeavmAdapter" }
wasm-tools component new $EmbeddedWasm --adapt "wasi_snapshot_preview1=$Adapter" --adapt "teavm=$TeavmAdapter" -o $OutWasm
wasm-tools validate $OutWasm
Write-Host "Wrote component: $OutWasm"
