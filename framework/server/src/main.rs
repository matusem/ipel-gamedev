use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, rt, web};
use actix_ws::AggregatedMessage;
use common::{Game, ProcessingTransaction, SerializationFormat, SerializedBuffer};
use futures_util::StreamExt as _;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock, mpsc},
};
use uuid::Uuid;
use wasmtime::*;
use wasmtime_wasi::{p2::WasiCtxBuilder, preview1::WasiP1Ctx};

use crate::wasm::Wasm;

mod wasm;

async fn game(
    request: HttpRequest,
    stream: web::Payload,
    module: web::Data<Module>,
    wasm: web::Data<Wasm>,
    game_db: web::Data<GameDb>,
) -> Result<HttpResponse, Error> {
    let query = request
        .uri()
        .query()
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing query parameters"))?;

    let params: HashMap<String, String> = serde_urlencoded::from_str(query).unwrap();
    let game_id = params
        .get("game-id")
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'game-id' parameter"))?;
    let game_id = Uuid::parse_str(game_id)
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid 'game-id' format"))?;
    let player = params
        .get("player")
        .ok_or_else(|| actix_web::error::ErrorBadRequest("Missing 'player' parameter"))?
        .as_bytes()
        .iter()
        .cloned()
        .collect::<Vec<_>>();

    let (response, mut session, stream) = actix_ws::handle(&request, stream)?;

    let mut stream = stream
        .aggregate_continuations()
        .max_continuation_size(2_usize.pow(20));

    let (sender, mut receiver) = mpsc::channel::<SerializedBuffer>();

    // {
    //     let games = game_db.get_mut().expect("Failed to lock game database");
    //     games
    //         .get_mut(&game_id)
    //         .ok_or_else(|| actix_web::error::ErrorNotFound("Game not found"))?
    //         .1
    //         .push((sender, SerializedBuffer::default()));
    // }

    let mut send_session = session.clone();
    rt::spawn(async move {
        while let Ok(msg) = receiver.recv() {
            // if send_session.text(msg).await.is_err() {
            //     break; // Exit if sending fails
            // }
        }
    });

    rt::spawn(async move {
        // let mut store = wasm.create_store();
        // let instance = wasm
        //     .instantiate_module(&mut store, &module)
        //     .await
        //     .expect("Failed to instantiate module");

        // let game_instance = GameWasmInstance::new(&mut store, instance).unwrap();

        // while let Some(msg) = stream.next().await {
        //     match msg {
        //         Ok(AggregatedMessage::Text(text)) => {
        //             println!("Received text message: {}", text);

        //             if let Some((command, payload)) = text.split_once(':') {
        //                 let output = match command {
        //                     "init" => {
        //                         println!("Received init command with payload: {}", payload);
        //                         game_instance
        //                             .try_init(
        //                                 &mut store,
        //                                 payload.as_bytes(),
        //                                 SerializationFormat::Json,
        //                             )
        //                             .await
        //                     }
        //                     "action" => {
        //                         game_instance
        //                             .try_take_action(
        //                                 &mut store,
        //                                 payload.as_bytes(),
        //                                 SerializationFormat::Json,
        //                             )
        //                             .await
        //                     }
        //                     _ => {
        //                         println!("Unknown command: {}", command);
        //                         Err(format!("Unknown command: {}", command))
        //                     }
        //                 };

        //                 let json = output.and_then(|output| {
        //                     String::from_utf8(output.clone())
        //                         .map_err(|e| format!("Error converting output to UTF-8: {}", e))
        //                 });

        //                 match json {
        //                     Ok(json) => {
        //                         println!("Received init command with payload: {}", json);
        //                         session.text(json).await.unwrap();
        //                     }
        //                     Err(e) => {
        //                         println!("Error during init: {}", e);
        //                         session.text(format!("Error: {}", e)).await.unwrap();
        //                     }
        //                 }
        //             }
        //         }
        //         _ => {}
        //     }
        // }
    });

    Ok(response)
}

async fn create_game(
    request: HttpRequest,
    wasm: web::Data<Wasm>,
    module: web::Data<Module>,
    game_db: web::Data<GameDb>,
) -> Result<HttpResponse, Error> {
    if let Some(query) = request.uri().query() {
        println!("Query params: {}", query);
        let params: HashMap<String, String> = serde_urlencoded::from_str(query).unwrap();
        println!("Parsed params: {:?}", params);
    }

    let mut store = wasm.create_store();
    let instance = wasm
        .instantiate_module(&mut store, &module)
        .await
        .expect("Failed to instantiate module");

    let game_instance = GameWasmInstance::new(&mut store, instance).unwrap();

    let game = Game {
        state: SerializedBuffer::default(),
        player_states: SerializedBuffer::default(),
    };

    let game_id = Uuid::new_v4();
    let game_db_clone = game_db.clone();
    {
        let mut db = game_db_clone.write().unwrap();
        db.insert(game_id.clone(), Arc::new(Mutex::new((game, vec![]))));
    }

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(game_id.to_string()))
}

async fn get_games(
    request: HttpRequest,
    game_db: web::Data<GameDb>,
) -> Result<HttpResponse, Error> {
    let games = game_db
        .read()
        .expect("Failed to lock game database")
        .iter()
        .map(|(id, _)| id.to_string())
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&games).unwrap()))
}

struct GameWasmInstance {
    alloc: TypedFunc<u32, u32>,
    dealloc: TypedFunc<(u32, u32), ()>,
    process: TypedFunc<(u32, u32, u32), i32>,
    memory: Memory,
}

impl GameWasmInstance {
    fn new(store: &mut Store<WasiP1Ctx>, instance: Instance) -> Result<Self, String> {
        let alloc = instance
            .get_typed_func::<u32, u32>(&mut *store, "alloc")
            .map_err(|e| format!("Failed to get alloc function: {}", e))?;

        let dealloc = instance
            .get_typed_func::<(u32, u32), ()>(&mut *store, "dealloc")
            .map_err(|e| format!("Failed to get dealloc function: {}", e))?;

        let process = instance
            .get_typed_func::<(u32, u32, u32), i32>(&mut *store, "process")
            .map_err(|e| format!("Failed to get process function: {}", e))?;

        let memory = instance
            .get_memory(&mut *store, "memory")
            .ok_or_else(|| "Failed to get memory from instance".to_string())?;

        Ok(Self {
            alloc,
            dealloc,
            process,
            memory,
        })
    }

    // async fn try_init(
    //     &self,
    //     mut store: &mut Store<WasiP1Ctx>,
    //     input_buffer: &[u8],
    //     serialization_format: SerializationFormat,
    // ) -> Result<Vec<u8>, String> {
    //     self.wasm_buffers_io(
    //         &self.try_init,
    //         &mut store,
    //         input_buffer,
    //         serialization_format,
    //     )
    //     .await
    // }

    async fn process<T: ProcessingTransaction>(
        &self,
        input: T::Input,
        serialization_format: SerializationFormat,
    ) -> T::Output {
        todo!("Implement process method")
    }

    async fn wasm_buffers_io(
        &self,
        function: &TypedFunc<(u32, u32, u32), i32>,
        mut store: &mut Store<WasiP1Ctx>,
        input_buffer: &[u8],
        serialization_format: SerializationFormat,
    ) -> Result<Vec<u8>, String> {
        todo!("Implement wasm_buffers_io");

        // let input_buffer_length = input_buffer.len() as u32;

        // let input_buffer_pointer = self
        //     .alloc
        //     .call_async(&mut store, input_buffer_length)
        //     .await
        //     .unwrap();
        // self.memory
        //     .write(&mut store, input_buffer_pointer as usize, &input_buffer)
        //     .unwrap();

        // let output_struct_size = 4u32;
        // let output_buffer_pointer = self
        //     .alloc
        //     .call_async(&mut store, output_struct_size)
        //     .await
        //     .unwrap();

        // let output_buffer_length = function
        //     .call_async(
        //         &mut store,
        //         (
        //             input_buffer_pointer,
        //             input_buffer_length,
        //             output_buffer_pointer,
        //             serialization_format.into(),
        //         ),
        //     )
        //     .await
        //     .unwrap() as u32;

        // let mut output_struct = [0u8; 4];
        // self.memory
        //     .read(
        //         &mut store,
        //         output_buffer_pointer as usize,
        //         &mut output_struct,
        //     )
        //     .unwrap();
        // let output_buffer_pointer = u32::from_le_bytes(output_struct);

        // let output_buffer = {
        //     self.memory
        //         .data(&store)
        //         .get(
        //             output_buffer_pointer as usize
        //                 ..(output_buffer_pointer + output_buffer_length) as usize,
        //         )
        //         .map(|data| data.to_vec())
        // };

        // self.dealloc
        //     .call_async(&mut store, (input_buffer_pointer, input_buffer_length))
        //     .await
        //     .unwrap();
        // self.dealloc
        //     .call_async(&mut store, (output_buffer_pointer, output_buffer_length))
        //     .await
        //     .unwrap();

        // output_buffer.ok_or_else(|| {
        //     format!(
        //         "Output buffer out of bounds: pointer = {}, length = {}",
        //         output_buffer_pointer, output_buffer_length
        //     )
        // })
    }
}

type ClientId = u64;
type GameDb = Arc<
    RwLock<
        HashMap<
            Uuid,
            Arc<
                Mutex<(
                    Game,
                    Vec<(mpsc::Sender<SerializedBuffer>, SerializedBuffer)>,
                )>,
            >,
        >,
    >,
>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let wasm_bytes = std::fs::read("../target/wasm32-wasip1/release/wasm_game.wasm")
        .expect("Wasm module not found, build wasm_game first");

    let wasm = Wasm::new();

    let module = wasm
        .create_module(&wasm_bytes)
        .expect("Failed to create module");

    let wasm = web::Data::new(wasm);
    let module = web::Data::new(module);

    let game_db: GameDb = Arc::new(RwLock::new(HashMap::new()));
    let game_db = web::Data::new(game_db.clone());

    HttpServer::new(move || {
        App::new()
            .app_data(wasm.clone())
            .app_data(module.clone())
            .app_data(game_db.clone())
            .route("/create_game", web::post().to(create_game))
            .route("/game", web::get().to(game))
            .route("/games", web::get().to(get_games))
    })
    .bind(("127.0.0.1", 80))?
    .run()
    .await
}
