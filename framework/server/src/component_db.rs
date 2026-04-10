use crate::game_core::GameCore;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use wasmtime::{Config, Engine, Module, Store, component::Component};
use wasmtime_wasi::{WasiCtxBuilder, p1::WasiP1Ctx};

#[derive(Clone)]
pub struct ComponentDb {
    engine: Engine,
    components: Arc<RwLock<HashMap<String, Component>>>,
}

impl ComponentDb {
    pub fn new() -> Self {
        let mut config = Config::default();
        config.async_support(true);
        let engine = Engine::new(&config).unwrap();

        Self {
            engine,
            components: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn insert_components_as_wasm_bytes(
        &self,
        name: &str,
        wasm_bytes: &[u8],
    ) -> Result<(), String> {
        let component = Component::new(&self.engine, wasm_bytes)
            .map_err(|e| format!("Failed to create component: {}", e))?;
        self.insert_component(name, component)
    }

    pub fn insert_component(&self, name: &str, component: Component) -> Result<(), String> {
        let name = name.to_string();

        let mut components = self.components.write().unwrap();
        if components.contains_key(&name) {
            Err(format!("Component with name '{}' already exists", name))
        } else {
            components.insert(name, component);
            Ok(())
        }
    }

    pub fn get(&self, name: &str) -> Option<Component> {
        let components = self.components.read().unwrap();
        components.get(name).cloned()
    }

    pub async fn create_game_core(
        &self,
        name: &str,
    ) -> Result<(GameCore, Store<WasiP1Ctx>), String> {
        let component = self
            .get(&name)
            .ok_or_else(|| String::from("Game component not found"))?;

        let mut linker = wasmtime::component::Linker::new(&self.engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker).unwrap();

        let mut store = Store::new(&self.engine, WasiCtxBuilder::new().build_p1());
        let game_core = GameCore::instantiate_async(&mut store, &component, &linker)
            .await
            .map_err(|e| format!("Failed to instantiate component: {e}"))?;

        Ok((game_core, store))
    }

    pub fn get_engine(&self) -> Engine {
        self.engine.clone()
    }

    /// Validates that `logic.wasm` is a **WebAssembly Component** the runtime can load and that
    /// [`GameCore`] can be instantiated (same path as upload / `cargo component build` output).
    ///
    /// Error strings are detailed for API diagnostics and are also printed to stderr on failure
    /// so Docker/server logs show the same context.
    pub async fn validate_component_instantiable(&self, wasm_bytes: &[u8]) -> Result<(), String> {
        let summary = wasm_bytes_summary(wasm_bytes);

        let component = match Component::new(&self.engine, wasm_bytes) {
            Ok(c) => c,
            Err(parse_err) => {
                let mut detail = format!(
                    "[1/3 parse component] {parse_err}. \
                     Explanation: the server loads a WebAssembly Component (component-model binary); \
                     this error usually means the file is a core Wasm module (MVP) or another format Wasmtime does not accept as Component::new. \
                     | {summary}"
                );
                match Module::new(&self.engine, wasm_bytes) {
                    Ok(module) => {
                        let n = module.imports().len();
                        let imp = core_import_preview(&module);
                        detail.push_str(&format!(
                            " | Confirmed: the same bytes load as a core Wasm module (wasmtime::Module), not as a component. \
                             Import count: {n}. Sample imports (module::name, up to 20): {imp}. \
                             For this server, build logic.wasm with: cargo component build --release in the Rust component crate (game-core WIT world). \
                             Java/TeaVM/Fermyon output is typically core Wasm plus WASI; it cannot be uploaded as-is unless you add a separate component packaging step."
                        ));
                    }
                    Err(core_err) => {
                        detail.push_str(&format!(
                            " | Could not load as a core Wasm module either: {core_err}"
                        ));
                    }
                }
                log_validation_failure(&detail);
                return Err(detail);
            }
        };

        let mut linker = wasmtime::component::Linker::new(&self.engine);
        if let Err(e) = wasmtime_wasi::p2::add_to_linker_async(&mut linker) {
            let msg = format!("[2/3 link WASI] {e} | {summary}");
            log_validation_failure(&msg);
            return Err(msg);
        }

        let mut store = Store::new(&self.engine, WasiCtxBuilder::new().build_p1());
        if let Err(e) = GameCore::instantiate_async(&mut store, &component, &linker).await {
            let msg = format!(
                "[3/3 instantiate GameCore] {e} | {summary} \
                 (component parsed and WASI linked; failure is often missing or mismatched exports vs the game-core WIT world.)"
            );
            log_validation_failure(&msg);
            return Err(msg);
        }

        Ok(())
    }
}

fn log_validation_failure(detail: &str) {
    eprintln!("[logic.wasm validation] {}", detail.replace('\n', " "));
}

fn wasm_bytes_summary(bytes: &[u8]) -> String {
    let n = bytes.len();
    if n < 8 {
        return format!("size={n} bytes (too small for a wasm header)");
    }
    let magic = &bytes[0..4];
    if magic != b"\0asm" {
        return format!(
            "size={n} bytes, magic={magic:02x?} (expected 00 61 73 6d), first_8={:02x?}",
            &bytes[..8]
        );
    }
    let ver = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    format!("size={n} bytes, wasm_header_version=0x{ver:08x} (little-endian dword after magic)")
}

fn core_import_preview(module: &Module) -> String {
    let mut parts = Vec::new();
    for (i, imp) in module.imports().enumerate() {
        if i >= 20 {
            parts.push("…".to_string());
            break;
        }
        parts.push(format!("{}::{}", imp.module(), imp.name()));
    }
    if parts.is_empty() {
        "(none)".to_string()
    } else {
        parts.join(", ")
    }
}
