use crate::game_core::GameCore;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use wasmtime::{Config, Engine, Store, component::Component};
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
            .unwrap();

        Ok((game_core, store))
    }

    pub fn get_engine(&self) -> Engine {
        self.engine.clone()
    }
}
