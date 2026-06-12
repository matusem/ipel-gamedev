//! Java TeaVM logic.wasm must load as a WebAssembly Component and instantiate GameCore.

use server::component_db::ComponentDb;

#[tokio::test]
async fn java_template_logic_wasm_instantiates_game_core() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sdk/java/component-template/build/out/logic.wasm");
    if !path.is_file() {
        eprintln!("skip: build with `gradle exportLogicComponent` in sdk/java/component-template");
        return;
    }
    let bytes = std::fs::read(&path).expect("read logic.wasm");
    let db = ComponentDb::new();
    db.validate_component_instantiable(&bytes)
        .await
        .expect("Java logic.wasm should pass server upload validation");
}
