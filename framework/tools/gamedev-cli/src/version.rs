//! Version constants kept in sync with `platform/manifest.json` at release time.

use std::collections::HashMap;

pub const FRAMEWORK_VERSION: &str = "0.1.0";
pub const WIT_VERSION: &str = "game-core-v1";

pub fn cli_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn sdk_versions() -> HashMap<String, String> {
    HashMap::from([
        ("upjs-gdd-shared-types".into(), "0.1.0".into()),
        ("upjs-gdd-rust-shared".into(), "0.1.0".into()),
        ("upjs-gdd-bevy".into(), "0.1.0".into()),
        ("upjs-gdd-dioxus".into(), "0.1.0".into()),
        ("@upjs-gdd/game-sdk".into(), "0.1.0".into()),
        ("@upjs-gdd/sdk-js".into(), "0.1.0".into()),
        ("sk.upjs.gdd:game".into(), "0.1.0".into()),
    ])
}
