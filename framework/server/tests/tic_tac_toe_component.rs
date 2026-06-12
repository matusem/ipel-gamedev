//! Published tic_tac_toe logic.wasm must instantiate against the current game-core WIT.

use server::component_db::ComponentDb;

#[tokio::test]
async fn tic_tac_toe_logic_wasm_instantiates_game_core() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../games/tic_tac_toe/logic.wasm");
    if !path.is_file() {
        eprintln!(
            "skip: build with `cargo component build --release` in games/tic_tac_toe/rust/component"
        );
        return;
    }
    let bytes = std::fs::read(&path).expect("read logic.wasm");
    let db = ComponentDb::new();
    db.validate_component_instantiable(&bytes)
        .await
        .expect("tic_tac_toe logic.wasm should match server game-core WIT");
}
