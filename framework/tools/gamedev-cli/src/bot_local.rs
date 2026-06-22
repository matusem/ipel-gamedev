//! Local dev bot runner: request seat, poll approval, play via WS + Wasmtime.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::json;
use tungstenite::{connect, Message};
use url::Url;
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi::WasiCtxBuilder;

use crate::api::gql_raw;
use crate::auth::load_token;
use crate::build;
use crate::cli::{BotRunArgs, BuildArgs};
use crate::config;
use crate::project::{load_config, ProjectKind};

wasmtime::component::bindgen!({
    path: "../../bot.wit",
    world: "game-bot",
});

struct BotWasm {
    bot: GameBot,
    store: Store<WasiP1Ctx>,
}

impl BotWasm {
    fn load(wasm_path: &Path) -> Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        let engine = Engine::new(&config)?;
        let bytes = std::fs::read(wasm_path)
            .with_context(|| format!("read {}", wasm_path.display()))?;
        let component = Component::new(&engine, &bytes)
            .context("parse bot.wasm component")?;
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .context("link WASI for bot")?;
        let mut store = Store::new(&engine, WasiCtxBuilder::new().build_p1());
        let bot = GameBot::instantiate(&mut store, &component, &linker)
            .context("instantiate GameBot")?;
        Ok(Self { bot, store })
    }

    fn decide(&mut self, settings: &[u8], frame: &[u8]) -> Result<Option<Vec<u8>>> {
        let result = self
            .bot
            .call_decide(
                &mut self.store,
                SerializationFormat::Json,
                &settings.to_vec(),
                &frame.to_vec(),
            )
            .context("bot decide()")?;
        match result {
            Ok(opt) => Ok(opt),
            Err(e) => bail!("bot error: {e:?}"),
        }
    }
}

fn resolve_project_root(args: &BotRunArgs) -> Result<PathBuf> {
    args.project_dir
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .context("project directory")?
        .canonicalize()
        .context("canonicalize project dir")
}

fn bot_wasm_path(root: &Path) -> PathBuf {
    root.join("dist").join("bot.wasm")
}

fn graphql_to_ws_base(graphql_url: &str) -> Result<String> {
    let mut url = Url::parse(graphql_url).context("parse server URL")?;
    url.set_path("");
    url.set_query(None);
    let scheme = match url.scheme() {
        "https" => "wss",
        "http" => "ws",
        other => bail!("unsupported URL scheme: {other}"),
    };
    url.set_scheme(scheme).ok();
    Ok(url.to_string().trim_end_matches('/').to_string())
}

fn poll_until<F>(timeout: Duration, interval: Duration, mut check: F) -> Result<()>
where
    F: FnMut() -> Result<bool>,
{
    let start = Instant::now();
    loop {
        if check()? {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            bail!("timed out after {}s", timeout.as_secs());
        }
        thread::sleep(interval);
    }
}

pub fn run(args: BotRunArgs) -> Result<()> {
    let root = resolve_project_root(&args)?;
    let cfg = load_config(&root)?;
    if cfg.kind != ProjectKind::Bot {
        bail!("bot-run requires a bot project (gamedev.toml kind = bot)");
    }
    let game = cfg.game.as_deref().context("bot project missing game slug")?;
    let contract_hash = cfg
        .contract_hash
        .as_deref()
        .context("bot project missing contract_hash")?;

    let wasm_path = bot_wasm_path(&root);
    if args.build || !wasm_path.is_file() {
        eprintln!("building bot.wasm...");
        build::run(BuildArgs {
            project_dir: Some(root.clone()),
            out: None,
            strict: false,
        })?;
    }
    if !wasm_path.is_file() {
        bail!("dist/bot.wasm not found — run gamedev build first");
    }

    let server_url = config::resolve_graphql_url(args.profile.as_deref(), &args.server_url)?;
    let token = load_token(&server_url)?.token;
    let label = args
        .label
        .clone()
        .unwrap_or_else(|| cfg.name.clone());

    let lobby_q = r#"query L($id: ID!) {
      lobby(id: $id) {
        id gameType status gameInstanceId
        seats { seatIndex playerIdentity botId externalBot }
      }
    }"#;
    let lobby_body = gql_raw(
        &server_url,
        &token,
        lobby_q,
        json!({ "id": args.lobby }),
    )?;
    let lobby_v: serde_json::Value = serde_json::from_str(&lobby_body)?;
    if let Some(errs) = lobby_v.get("errors") {
        bail!("lobby query failed: {errs}");
    }
    let lobby = &lobby_v["data"]["lobby"];
    if lobby.is_null() {
        bail!("lobby not found");
    }
    let game_type = lobby["gameType"].as_str().unwrap_or("");
    if game_type != game {
        bail!("lobby game type {game_type} does not match bot target {game}");
    }

    let mut bot = BotWasm::load(&wasm_path)?;
    let settings_bytes = bot
        .bot
        .call_default_settings(&mut bot.store, SerializationFormat::Json)
        .context("default-settings")?
        .map_err(|e| anyhow::anyhow!("default-settings error: {e:?}"))?;
    if let Some(err) = bot
        .bot
        .call_validate_settings(
            &mut bot.store,
            SerializationFormat::Json,
            &settings_bytes,
        )
        .context("validate-settings")?
        .map_err(|e| anyhow::anyhow!("validate-settings wasm error: {e:?}"))?
    {
        bail!("invalid settings: {}", String::from_utf8_lossy(&err));
    }

    let settings_json = String::from_utf8_lossy(&settings_bytes).into_owned();

    let req_q = r#"mutation R($id: ID!, $cat: String!, $label: String!, $hash: String!, $seat: Int, $settings: String) {
      requestExternalBotSeat(lobbyId: $id, category: $cat, label: $label, contractHash: $hash, desiredSeatIndex: $seat, settingsJson: $settings) {
        requestId connectToken
      }
    }"#;
    let req_body = gql_raw(
        &server_url,
        &token,
        req_q,
        json!({
            "id": args.lobby,
            "cat": "dev_local",
            "label": label,
            "hash": contract_hash,
            "seat": args.seat,
            "settings": settings_json,
        }),
    )?;
    let req_v: serde_json::Value = serde_json::from_str(&req_body)?;
    if let Some(errs) = req_v.get("errors") {
        bail!("request seat failed: {errs}");
    }
    let result = &req_v["data"]["requestExternalBotSeat"];
    let request_id = result["requestId"]
        .as_str()
        .context("missing requestId")?
        .to_string();
    let connect_token = result["connectToken"]
        .as_str()
        .context("missing connectToken")?
        .to_string();

    eprintln!("requested a seat — waiting for host approval (request {request_id})");

    let timeout = Duration::from_secs(args.timeout_secs);
    let rid = request_id.clone();
    let mut seat_index: Option<i32> = None;
    poll_until(timeout, Duration::from_secs(2), || {
        let body = gql_raw(
            &server_url,
            &token,
            r#"query B($id: ID!) { botRequest(requestId: $id) { status seatIndex } }"#,
            json!({ "id": rid }),
        )?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        let status = v.pointer("/data/botRequest/status")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        match status {
            "approved" => {
                seat_index = v
                    .pointer("/data/botRequest/seatIndex")
                    .and_then(|s| s.as_i64())
                    .map(|i| i as i32);
                Ok(true)
            }
            "denied" | "cancelled" => bail!("seat request was {status}"),
            _ => Ok(false),
        }
    })?;

    let seat_index = seat_index.context("approved but no seat index")?;
    eprintln!("seat approved (index {seat_index}) — waiting for host to start game");

    let lobby_id = args.lobby.clone();
    let mut game_id = String::new();
    let mut player_identity = String::new();
    poll_until(timeout, Duration::from_secs(2), || {
        let body = gql_raw(
            &server_url,
            &token,
            lobby_q,
            json!({ "id": lobby_id }),
        )?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        let lob = &v["data"]["lobby"];
        let status = lob["status"].as_str().unwrap_or("");
        if status == "in_game" {
            game_id = lob["gameInstanceId"]
                .as_str()
                .unwrap_or("")
                .to_string();
            if let Some(seats) = lob["seats"].as_array() {
                for s in seats {
                    if s["seatIndex"].as_i64() == Some(seat_index as i64) {
                        player_identity = s["playerIdentity"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                    }
                }
            }
            return Ok(!game_id.is_empty() && !player_identity.is_empty());
        }
        Ok(false)
    })?;

    let ws_base = graphql_to_ws_base(&server_url)?;
    let player_enc = urlencoding::encode(&player_identity);
    let ws_url = format!(
        "{ws_base}/game?id={game_id}&player={player_enc}&mode=bot&token={connect_token}"
    );
    eprintln!("connecting to {ws_url}");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let rid_cancel = request_id.clone();
    let server_cancel = server_url.clone();
    let token_cancel = token.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        let _ = gql_raw(
            &server_cancel,
            &token_cancel,
            r#"mutation C($id: ID!) { cancelExternalBotSeat(requestId: $id) }"#,
            json!({ "id": rid_cancel }),
        );
        eprintln!("\ninterrupted");
        std::process::exit(130);
    })
    .ok();

    let (mut socket, _) = connect(&ws_url).context("websocket connect")?;

    loop {
        if !running.load(Ordering::SeqCst) {
            break;
        }
        let msg = socket.read().context("websocket read")?;
        match msg {
            Message::Text(text) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if v.get("GameOver").is_some() {
                        eprintln!("GameOver: {text}");
                        break;
                    }
                }
                if let Some(action) = bot.decide(&settings_bytes, text.as_bytes())? {
                    socket
                        .send(Message::Text(
                            String::from_utf8_lossy(&action).into_owned().into(),
                        ))
                        .context("send action")?;
                }
            }
            Message::Close(_) => {
                eprintln!("connection closed");
                break;
            }
            _ => {}
        }
    }

    let _ = socket.close(None);
    eprintln!("done");
    Ok(())
}
