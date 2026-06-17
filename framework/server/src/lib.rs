pub mod deploy_webhook;
pub mod auth_password;
pub mod auth_sessions;
pub mod google_oauth;
pub mod component_db;
pub mod db;
pub mod friends;
pub mod game_db;
pub mod game_registry;
pub mod game_service;
pub mod game_storefront;
pub mod game_upload;
pub mod graphql;
pub mod lobby_db;
pub mod logging;
pub mod platform_manifest;
pub mod platform_stats;
pub mod user_engagement;

pub mod game_core {
    use wasmtime::component::bindgen;

    bindgen!({
        path: "../test.wit",
        world: "game-core",
        imports: { default: async | trappable },
        exports: { default: async }
    });
}

pub mod test_support;
