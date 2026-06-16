//! Compile-time CLI defaults.
//!
//! Local `cargo run -p gamedev-cli` uses localhost. Release binaries built with
//! `--features packaged` (CI / install scripts) default to production URLs.

pub const LOCAL_GRAPHQL_URL: &str = "http://localhost:8080/graphql";
pub const LOCAL_PLATFORM_BASE: &str = "http://localhost:8080";

pub const PROD_GRAPHQL_URL: &str = "https://gamedev.jinxwashere.com/graphql";
pub const PROD_PLATFORM_BASE: &str = "https://gamedev.jinxwashere.com";

/// Default GraphQL URL for flags, TUI, and fresh `config.toml` profiles.
pub const DEFAULT_GRAPHQL_URL: &str = if cfg!(feature = "packaged") {
    PROD_GRAPHQL_URL
} else {
    LOCAL_GRAPHQL_URL
};

/// Default platform base for doctor/update and fresh config.
pub const DEFAULT_PLATFORM_BASE: &str = if cfg!(feature = "packaged") {
    PROD_PLATFORM_BASE
} else {
    LOCAL_PLATFORM_BASE
};

/// Active profile name when creating a new config file.
pub const DEFAULT_PROFILE_NAME: &str = if cfg!(feature = "packaged") {
    "prod"
} else {
    "local"
};
