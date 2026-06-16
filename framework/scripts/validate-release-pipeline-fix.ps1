# Validates release pipeline fixes (native arm64, cargo-chef layer cache, no dead disk cache).
# Full timing validation still requires a GitHub Actions release run.
$ErrorActionPreference = 'Stop'
$root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent

function Assert-NotMatch {
    param([string]$Path, [string]$Pattern, [string]$Message)
    $content = Get-Content -Raw -Path $Path
    if ($content -match $Pattern) {
        throw $Message
    }
}

function Assert-Match {
    param([string]$Path, [string]$Pattern, [string]$Message)
    $content = Get-Content -Raw -Path $Path
    if ($content -notmatch $Pattern) {
        throw $Message
    }
}

$release = Join-Path $root '.github/workflows/release-deploy.yml'
$ciBuilder = Join-Path $root '.github/workflows/ci-builder-image.yml'
$dockerfile = Join-Path $root 'framework/Dockerfile'
$builderDockerfile = Join-Path $root 'framework/docker/ci-builder/Dockerfile'

Assert-Match $release 'runs-on: ubuntu-24\.04-arm' 'build-image must use ubuntu-24.04-arm'
Assert-NotMatch $release 'setup-qemu-action' 'release-deploy must not use QEMU'
Assert-NotMatch $release 'docker-cargo-cache' 'release-deploy must not reference .docker-cargo-cache'
Assert-NotMatch $release 'build-contexts:' 'release-deploy must not use build-contexts cargo-cache'
Assert-Match $release 'command -v cargo-chef' 'release-deploy must verify cargo-chef in builder image'

Assert-Match $ciBuilder 'runs-on: ubuntu-24\.04-arm' 'ci-builder must use ubuntu-24.04-arm'
Assert-NotMatch $ciBuilder 'setup-qemu-action' 'ci-builder must not use QEMU'
Assert-Match $ciBuilder 'cargo-chef --version' 'ci-builder must verify tools after push'

Assert-NotMatch $dockerfile 'from=cargo-cache' 'framework/Dockerfile must not bind-mount cargo-cache'
Assert-Match $dockerfile 'type=cache,target=/usr/local/cargo/registry,sharing=locked' 'Dockerfile must use registry cache mounts'
Assert-NotMatch $dockerfile 'install-target' 'Dockerfile must not use install-target bootstrap'

Assert-Match $builderDockerfile 'ENV PATH="/usr/local/cargo/bin:\$\{PATH\}"' 'ci-builder must expose cargo bin on PATH'
Assert-Match $builderDockerfile 'cargo install cargo-chef' 'ci-builder must install cargo-chef'

Write-Host 'Release pipeline fix validation passed (static checks).'
Write-Host 'Next: push to main, run CI Builder Image workflow, then trigger Release and Deploy on a test tag.'
