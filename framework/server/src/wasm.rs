use wasmtime::*;
use wasmtime_wasi::{p1::{add_to_linker_async, WasiP1Ctx}, WasiCtxBuilder};

#[derive(Clone)]
pub struct Wasm {
    engine: Engine,
    linker: Linker<WasiP1Ctx>,
}

impl Wasm {
    pub fn get_linker(&self) -> &Linker<WasiP1Ctx> {
        &self.linker
    }

    pub fn create_module(&self, wasm_bytes: &[u8]) -> Result<Module, String> {
        Module::new(&self.engine, wasm_bytes).map_err(|e| e.to_string())
    }

    pub fn create_store(&self) -> Store<WasiP1Ctx> {
        let wasi_ctx = WasiCtxBuilder::new()
            .inherit_stdio()
            .inherit_env()
            .build_p1();

        Store::new(&self.engine, wasi_ctx)
    }

    pub async fn instantiate_module(
        &self,
        store: &mut Store<WasiP1Ctx>,
        module: &Module,
    ) -> Result<Instance, String> {
        self.linker
            .instantiate_async(store, module)
            .await
            .map_err(|e| e.to_string())
    }

    pub fn new() -> Self {
        let engine = Engine::new(&Config::default().async_support(true)).unwrap();
        let mut linker = Linker::new(&engine);
        add_to_linker_async(&mut linker, |ctx| ctx).unwrap();

        Wasm { engine, linker }
    }
}
