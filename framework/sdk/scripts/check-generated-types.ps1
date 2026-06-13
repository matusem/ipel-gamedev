$ErrorActionPreference = "Stop"

Push-Location "$PSScriptRoot/../.."
try {
  cargo run -p upjs-gdd-shared-types --features typegen --bin export_ts
  git diff --exit-code -- "framework/sdk/js/generated-types"
}
finally {
  Pop-Location
}
