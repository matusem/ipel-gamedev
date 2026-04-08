$ErrorActionPreference = "Stop"

Push-Location "$PSScriptRoot/../.."
try {
  cargo run -p framework-sdk-shared-types --features typegen --bin export_ts
  git diff --exit-code -- "framework/sdk/js/generated-types"
}
finally {
  Pop-Location
}
