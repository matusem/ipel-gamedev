//! Bevy WASM play client for tic-tac-toe (framework protocol, same idea as checkers `web`).
//! URL query: `ws`, `id`, `player` (injected by the lobby iframe). Click an empty cell on your turn to move.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game::PlayerState as GamePlayerStateTrait;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use my_game_logic::{Player, PlayerEvent, PlayerState, Position};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

const CELL: f32 = 88.0;

#[derive(Resource, Default, Clone)]
struct WsInbox(Arc<Mutex<Vec<String>>>);

#[derive(Resource, Default)]
struct NetModel {
    snapshot: Option<PlayerState>,
    status: String,
}

#[derive(Resource, Clone)]
struct WsHandle {
    send: Arc<Mutex<Option<web_sys::WebSocket>>>,
}

#[derive(Resource, Default)]
struct BoardLayout {
    side: u8,
    cell_entities: Vec<Entity>,
}

#[derive(Resource, Default)]
struct MarkEntities(Vec<Entity>);

#[derive(Resource, Default)]
struct LastMarkSig(Option<u64>);

#[derive(Component)]
#[allow(dead_code)]
struct BoardCell {
    row: u8,
    col: u8,
}

#[derive(Component)]
struct CellMark;

#[derive(Component)]
struct HudStatus;

#[derive(Component)]
struct HudYou;

#[derive(Component)]
struct HudTurn;

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    let inbox = WsInbox::default();
    let ws_send: Arc<Mutex<Option<web_sys::WebSocket>>> = Arc::new(Mutex::new(None));

    #[cfg(target_arch = "wasm32")]
    {
        let inbox_c = inbox.clone();
        let ws_c = ws_send.clone();
        wasm_bindgen_futures::spawn_local(async move {
            connect_ws(inbox_c, ws_c).await;
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        inbox
            .0
            .lock()
            .unwrap()
            .push(r#"{"__status":"Build wasm32-unknown-unknown and open with lobby ?ws=&id=&player= for live play."}"#.into());
    }

    App::new()
        .insert_resource(inbox)
        .insert_resource(NetModel {
            snapshot: None,
            status: "…".into(),
        })
        .insert_resource(WsHandle { send: ws_send })
        .insert_resource(BoardLayout::default())
        .insert_resource(MarkEntities::default())
        .insert_resource(LastMarkSig::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Tic-Tac-Toe".into(),
                resolution: (640., 560.).into(),
                #[cfg(target_arch = "wasm32")]
                canvas: Some("#bevy-canvas".into()),
                #[cfg(target_arch = "wasm32")]
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (
                poll_inbox,
                sync_board_grid,
                sync_marks,
                board_click,
                update_hud,
            ),
        )
        .run();
}

fn cell_center(row: u8, col: u8, side: u8) -> Vec2 {
    let sf = side as f32;
    let o = -(sf * CELL) / 2.0 + CELL / 2.0;
    Vec2::new(o + col as f32 * CELL, o + row as f32 * CELL)
}

fn world_to_cell(world: Vec2, side: u8) -> Option<(u8, u8)> {
    let sf = side as f32;
    let o = -(sf * CELL) / 2.0 + CELL / 2.0;
    let col = ((world.x - o) / CELL + 0.5).floor() as i32;
    let row = ((world.y - o) / CELL + 0.5).floor() as i32;
    if row < 0 || col < 0 || row >= side as i32 || col >= side as i32 {
        return None;
    }
    Some((row as u8, col as u8))
}

fn board_hash(ps: &PlayerState) -> u64 {
    let side = ps.state.config.side_length;
    let mut h: u64 = 0;
    for r in 0..side {
        for c in 0..side {
            let v = ps
                .state
                .board
                .get(Position(r, c), side)
                .flatten()
                .map(|p| match p {
                    Player::X => 1u64,
                    Player::O => 2u64,
                })
                .unwrap_or(0);
            h = h.wrapping_mul(31).wrapping_add(r as u64).wrapping_add(c as u64);
            h = h.wrapping_mul(31).wrapping_add(v);
        }
    }
    h ^ match ps.state.current_player {
        Player::X => 0x9e37_79b1,
        Player::O => 0x85eb_ca6b,
    }
}

#[cfg(target_arch = "wasm32")]
async fn connect_ws(inbox: WsInbox, ws_slot: Arc<Mutex<Option<web_sys::WebSocket>>>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(search) = window.location().search() else {
        return;
    };
    let params: std::collections::HashMap<String, String> =
        serde_urlencoded::from_str(search.trim_start_matches('?')).unwrap_or_default();
    let Some(ws_base) = params.get("ws").filter(|s| !s.is_empty()) else {
        inbox
            .0
            .lock()
            .unwrap()
            .push(r#"{"__status":"Missing ws query param"}"#.into());
        return;
    };
    let id = params.get("id").cloned().unwrap_or_default();
    let player = params.get("player").cloned().unwrap_or_default();
    let url = format!(
        "{}?id={}&player={}",
        ws_base,
        urlencoding::encode(&id),
        urlencoding::encode(&player)
    );

    let Ok(ws) = web_sys::WebSocket::new(&url) else {
        inbox
            .0
            .lock()
            .unwrap()
            .push(r#"{"__status":"WebSocket new() failed"}"#.into());
        return;
    };
    *ws_slot.lock().unwrap() = Some(ws.clone());

    let inbox_open = inbox.clone();
    let onopen = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
        inbox_open
            .0
            .lock()
            .unwrap()
            .push(r#"{"__status":"Connected"}"#.into());
    });
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    let inbox_msg = inbox.clone();
    let onmsg = wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |e: web_sys::MessageEvent| {
        if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
            inbox_msg.0.lock().unwrap().push(String::from(txt));
        }
    });
    ws.set_onmessage(Some(onmsg.as_ref().unchecked_ref()));
    onmsg.forget();

    let inbox_err = inbox.clone();
    let onerr = wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |_e: web_sys::Event| {
        inbox_err
            .0
            .lock()
            .unwrap()
            .push(r#"{"__status":"WebSocket error"}"#.into());
    });
    ws.set_onerror(Some(onerr.as_ref().unchecked_ref()));
    onerr.forget();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn(Camera2d::default());

    let panel = Color::srgb(0.12, 0.14, 0.18);
    let border = Color::srgb(0.35, 0.4, 0.48);
    let muted = Color::srgb(0.55, 0.6, 0.68);
    let title_c = Color::srgb(0.92, 0.94, 0.98);

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                left: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(12.0)),
                min_width: Val::Px(200.0),
                max_width: Val::Px(280.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(panel),
            BorderColor(border),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Tic-Tac-Toe"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(title_c.into()),
            ));
            p.spawn((
                Text::new("Click a square on your turn."),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(muted.into()),
            ));
            p.spawn((
                Text::new("…"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.75, 0.82, 0.95).into()),
                HudStatus,
            ));
            p.spawn((
                Text::new("You: ?"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(title_c.into()),
                HudYou,
            ));
            p.spawn((
                Text::new("Turn: ?"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(title_c.into()),
                HudTurn,
            ));
        });
}

fn poll_inbox(
    inbox: Res<WsInbox>,
    mut model: ResMut<NetModel>,
    mut layout: ResMut<BoardLayout>,
    mut last_marks: ResMut<LastMarkSig>,
) {
    let mut g = inbox.0.lock().unwrap();
    while let Some(msg) = g.pop() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&msg) {
            if let Some(s) = v.get("__status").and_then(|x| x.as_str()) {
                model.status = s.into();
                continue;
            }
            if let Some(st) = v.get("state") {
                if let Ok(ps) = serde_json::from_value::<PlayerState>(st.clone()) {
                    model.snapshot = Some(ps);
                    layout.cell_entities.clear();
                    layout.side = 0;
                    last_marks.0 = None;
                    if !model.status.starts_with("Game over") {
                        model.status = "In game".into();
                    }
                    continue;
                }
            }
        }
        if let Ok(ps) = serde_json::from_str::<PlayerState>(&msg) {
            model.snapshot = Some(ps);
            layout.cell_entities.clear();
            layout.side = 0;
            last_marks.0 = None;
            if !model.status.starts_with("Game over") {
                model.status = "In game".into();
            }
            continue;
        }

        #[derive(Deserialize)]
        struct Wrap {
            #[serde(rename = "Event")]
            event: Option<PlayerEvent>,
            #[serde(rename = "GameOver")]
            game_over: Option<serde_json::Value>,
        }
        if let Ok(w) = serde_json::from_str::<Wrap>(&msg) {
            if let Some(ev) = w.event {
                if let Some(ref mut snap) = model.snapshot {
                    GamePlayerStateTrait::apply_event(snap, &ev);
                    last_marks.0 = None;
                }
                continue;
            }
            if w.game_over.is_some() {
                model.status = "Game over".into();
            }
        }
    }
}

fn sync_board_grid(
    mut commands: Commands,
    model: Res<NetModel>,
    mut layout: ResMut<BoardLayout>,
    q_cells: Query<Entity, With<BoardCell>>,
) {
    let want = model
        .snapshot
        .as_ref()
        .map(|s| s.state.config.side_length)
        .unwrap_or(3);
    if layout.side == want && !layout.cell_entities.is_empty() {
        return;
    }
    for e in q_cells.iter() {
        commands.entity(e).despawn();
    }
    layout.cell_entities.clear();
    layout.side = want;

    for row in 0..want {
        for col in 0..want {
            let p = cell_center(row, col, want);
            let alt = (row + col) % 2 == 1;
            let color = if alt {
                Color::srgb(0.22, 0.26, 0.34)
            } else {
                Color::srgb(0.32, 0.36, 0.44)
            };
            let id = commands
                .spawn((
                    Sprite::from_color(color, Vec2::splat(CELL - 4.0)),
                    Transform::from_xyz(p.x, p.y, 0.0),
                    BoardCell { row, col },
                ))
                .id();
            layout.cell_entities.push(id);
        }
    }
}

fn sync_marks(
    mut commands: Commands,
    model: Res<NetModel>,
    mut marks: ResMut<MarkEntities>,
    mut last: ResMut<LastMarkSig>,
) {
    let Some(snap) = model.snapshot.as_ref() else {
        if !marks.0.is_empty() {
            for e in marks.0.drain(..) {
                commands.entity(e).despawn();
            }
        }
        last.0 = None;
        return;
    };

    let h = board_hash(snap);
    if last.0 == Some(h) {
        return;
    }
    last.0 = Some(h);

    for e in marks.0.drain(..) {
        commands.entity(e).despawn();
    }

    let side = snap.state.config.side_length;
    for row in 0..side {
        for col in 0..side {
            let Some(cell) = snap.state.board.get(Position(row, col), side).flatten() else {
                continue;
            };
            let label = match cell {
                Player::X => "X",
                Player::O => "O",
            };
            let (font_size, color) = match cell {
                Player::X => (58.0 / 1.2, Color::srgb(1.0, 0.42, 0.28)),
                Player::O => (58.0 / 1.2, Color::srgb(0.35, 0.72, 1.0)),
            };
            let p = cell_center(row, col, side);
            let id = commands
                .spawn((
                    Text2d::new(label),
                    TextFont {
                        font_size,
                        ..default()
                    },
                    TextColor(color.into()),
                    Transform::from_xyz(p.x, p.y, 1.5),
                    CellMark,
                ))
                .id();
            marks.0.push(id);
        }
    }
}

fn board_click(
    mouse: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut model: ResMut<NetModel>,
    ws: Res<WsHandle>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = q_window.get_single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_tf)) = q_camera.get_single() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(cam_tf, cursor) else {
        return;
    };
    let world = ray.origin.truncate();

    let Some(snap) = model.snapshot.as_ref() else {
        model.status = "Waiting for game state…".into();
        return;
    };
    if model.status.starts_with("Game over") {
        return;
    }

    let side = snap.state.config.side_length;
    let Some((row, col)) = world_to_cell(world, side) else {
        return;
    };
    let pos = Position(row, col);
    if let Err(e) = snap.can_take_action(&pos) {
        model.status = e;
        return;
    }

    let json = match serde_json::to_string(&pos) {
        Ok(s) => s,
        Err(_) => return,
    };
    let g = ws.send.lock().unwrap();
    if let Some(socket) = g.as_ref() {
        let _ = socket.send_with_str(&json);
        model.status = "Move sent".into();
    } else {
        model.status = "No WebSocket (open with lobby params)".into();
    }
}

fn update_hud(
    model: Res<NetModel>,
    mut q_status: Query<&mut Text, With<HudStatus>>,
    mut q_you: Query<&mut Text, With<HudYou>>,
    mut q_turn: Query<&mut Text, With<HudTurn>>,
) {
    for mut t in &mut q_status {
        **t = model.status.clone();
    }
    let (you_s, turn_s) = model.snapshot.as_ref().map_or(("?".into(), "?".into()), |s| {
        (
            format!("{:?}", s.player),
            format!("{:?}", s.state.current_player),
        )
    });
    for mut t in &mut q_you {
        **t = format!("You: {}", you_s);
    }
    for mut t in &mut q_turn {
        **t = format!("Turn: {}", turn_s);
    }
}
