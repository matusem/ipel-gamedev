//! Bevy **WASM** play client for checkers. URL query: `ws`, `id`, `player` (lobby iframe).
//! Click **dark** squares to append [`Cell`]s to the path, then **Send move** (JSON `MovePath`).

use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::window::PrimaryWindow;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
use checkers::{
    apply_event_to_state, legal_next_cells, Cell, MovePath, PieceKind, Player, PlayerEvent,
    PlayerState,
};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

const CELL: f32 = 56.0;

#[derive(Resource, Default, Clone)]
struct WsInbox(Arc<Mutex<Vec<String>>>);

#[derive(Resource, Default)]
struct NetModel {
    snapshot: Option<PlayerState>,
    path: Vec<Cell>,
    status: String,
}

#[derive(Resource, Clone)]
struct WsHandle {
    send: Arc<Mutex<Option<web_sys::WebSocket>>>,
}

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
        let mut g = inbox.0.lock().unwrap();
        g.push(r#"{"__status":"Run wasm32-unknown-unknown for live play."}"#.into());
    }

    App::new()
        .insert_resource(inbox)
        .insert_resource(NetModel {
            snapshot: None,
            path: Vec::new(),
            status: "…".into(),
        })
        .insert_resource(WsHandle { send: ws_send })
        .insert_resource(PieceEntities::default())
        .insert_resource(PathVizEntities::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Checkers".into(),
                resolution: (900., 720.).into(),
                #[cfg(target_arch = "wasm32")]
                canvas: Some("#bevy-canvas".into()),
                #[cfg(target_arch = "wasm32")]
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, (setup_visual_assets, setup_scene).chain())
        .add_systems(
            Update,
            (
                sync_board_cell_layout,
                poll_inbox,
                board_click,
                sync_pieces,
                sync_legal_highlights,
                sync_built_path_visual,
                button_send,
                button_clear,
                update_status_text,
            ),
        )
        .init_resource::<LastBoardSig>()
        .init_resource::<LastHighlightSig>()
        .init_resource::<LastPathVizSig>()
        .init_resource::<HighlightEntities>()
        .run();
}

#[derive(Component)]
struct BoardCell {
    row: u8,
    col: u8,
}

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct SendBtn;

#[derive(Component)]
struct ClearBtn;

#[derive(Component)]
struct PieceMark;

#[derive(Component)]
struct LegalMoveHint;

#[derive(Component)]
struct BuiltPathViz;

#[derive(Resource, Default)]
struct PieceEntities(Vec<Entity>);

#[derive(Resource, Default)]
struct HighlightEntities(Vec<Entity>);

#[derive(Resource, Default)]
struct PathVizEntities(Vec<Entity>);

/// Board state signature plus seat (Dark flips the view).
#[derive(Resource, Default)]
struct LastBoardSig(Option<(u64, Player)>);

#[derive(Resource, Default)]
struct LastHighlightSig(Option<(u64, Vec<Cell>, Player)>);

#[derive(Resource, Default)]
struct LastPathVizSig(Option<(u64, Vec<Cell>, Player)>);

#[derive(Resource, Clone)]
struct CheckersVisualAssets {
    man_mesh: Handle<Mesh>,
    king_mesh: Handle<Mesh>,
    legal_mesh: Handle<Mesh>,
    legal_material: Handle<ColorMaterial>,
    path_node_mesh: Handle<Mesh>,
    path_node_material: Handle<ColorMaterial>,
}

fn setup_visual_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.insert_resource(CheckersVisualAssets {
        man_mesh: meshes.add(Mesh::from(Circle::new(9.0))),
        king_mesh: meshes.add(Mesh::from(Circle::new(12.0))),
        legal_mesh: meshes.add(Mesh::from(Circle::new(20.0))),
        legal_material: materials.add(ColorMaterial::from(Color::srgba(0.15, 0.75, 0.45, 0.7))),
        path_node_mesh: meshes.add(Mesh::from(Circle::new(6.0))),
        path_node_material: materials.add(ColorMaterial::from(Color::srgba(1.0, 0.62, 0.12, 0.92))),
    });
}

fn board_signature(ps: &PlayerState) -> u64 {
    let mut h: u64 = 0;
    for i in 0..64 {
        let v = ps
            .state
            .board
            .cells()
            .get(i)
            .copied()
            .flatten()
            .map(|p| {
                let o = match p.owner {
                    Player::Dark => 1u64,
                    Player::Light => 2,
                };
                let k = match p.kind {
                    PieceKind::Man => 1u64,
                    PieceKind::King => 2,
                };
                o << 32 | k
            })
            .unwrap_or(0);
        h = h
            .wrapping_mul(31)
            .wrapping_add(i as u64)
            .wrapping_add(v);
    }
    h ^ match ps.state.current_player {
        Player::Dark => 0x9e37_79b1,
        Player::Light => 0x85eb_ca6b,
    }
}

/// Vertical screen row for drawing: Dark sits at the bottom (near side).
fn view_display_row(logical_row: u8, seat: Player) -> u8 {
    match seat {
        Player::Dark => 7u8.saturating_sub(logical_row),
        Player::Light => logical_row,
    }
}

fn board_cell_world_xy(logical_row: u8, col: u8, seat: Player) -> (f32, f32) {
    let ox = -4.0 * CELL + CELL / 2.0;
    let oy = 4.0 * CELL - CELL / 2.0;
    let vr = view_display_row(logical_row, seat);
    let x = ox + col as f32 * CELL;
    let y = oy - vr as f32 * CELL;
    (x, y)
}

fn sync_board_cell_layout(
    model: Res<NetModel>,
    mut q: Query<(&BoardCell, &mut Transform), Without<PieceMark>>,
) {
    let seat = model
        .snapshot
        .as_ref()
        .map(|s| s.player)
        .unwrap_or(Player::Light);
    for (cell, mut tf) in &mut q {
        let (x, y) = board_cell_world_xy(cell.row, cell.col, seat);
        tf.translation.x = x;
        tf.translation.y = y;
        tf.translation.z = 0.0;
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
        let mut m = inbox.0.lock().unwrap();
        m.push(r#"{"__status":"Missing ws"}"#.into());
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
        let mut m = inbox.0.lock().unwrap();
        m.push(r#"{"__status":"WebSocket new() failed"}"#.into());
        return;
    };
    *ws_slot.lock().unwrap() = Some(ws.clone());

    let inbox_open = inbox.clone();
    let onopen = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
        let mut m = inbox_open.0.lock().unwrap();
        m.push(r#"{"__status":"Connected"}"#.into());
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
    commands.spawn(Camera2dBundle::default());

    let ox = -4.0 * CELL + CELL / 2.0;
    let oy = 4.0 * CELL - CELL / 2.0;

    for row in 0..8u8 {
        for col in 0..8u8 {
            let dark = (row + col) % 2 == 1;
            let x = ox + col as f32 * CELL;
            let y = oy - row as f32 * CELL;
            let color = if dark {
                Color::srgb(0.38, 0.24, 0.14)
            } else {
                Color::srgb(0.94, 0.88, 0.76)
            };
            commands.spawn((
                SpriteBundle {
                    sprite: Sprite {
                        color,
                        custom_size: Some(Vec2::splat(CELL - 2.0)),
                        ..default()
                    },
                    transform: Transform::from_xyz(x, y, 0.0),
                    ..default()
                },
                BoardCell { row, col },
            ));
        }
    }

    commands
        .spawn(NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(6.0),
                left: Val::Px(6.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            background_color: Color::srgba(0.0, 0.0, 0.0, 0.55).into(),
            ..default()
        })
        .with_children(|p| {
            p.spawn(TextBundle::from_section(
                "Checkers — dark squares only. Build path, then Send.",
                TextStyle {
                    font_size: 15.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
            p.spawn((
                TextBundle::from_section(
                    "",
                    TextStyle {
                        font_size: 13.0,
                        color: Color::srgb(0.75, 0.9, 1.0),
                        ..default()
                    },
                ),
                StatusText,
            ));
            p.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                ..default()
            })
            .with_children(|p| {
                p.spawn((
                    ButtonBundle {
                        style: Style {
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                            ..default()
                        },
                        background_color: Color::srgb(0.2, 0.5, 0.28).into(),
                        ..default()
                    },
                    SendBtn,
                ))
                .with_children(|c| {
                    c.spawn(TextBundle::from_section(
                        "Send move",
                        TextStyle {
                            font_size: 14.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    ));
                });
                p.spawn((
                    ButtonBundle {
                        style: Style {
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                            ..default()
                        },
                        background_color: Color::srgb(0.5, 0.25, 0.2).into(),
                        ..default()
                    },
                    ClearBtn,
                ))
                .with_children(|c| {
                    c.spawn(TextBundle::from_section(
                        "Clear path",
                        TextStyle {
                            font_size: 14.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    ));
                });
            });
        });
}

fn poll_inbox(
    inbox: Res<WsInbox>,
    mut model: ResMut<NetModel>,
    mut last: ResMut<LastBoardSig>,
) {
    let mut g = inbox.0.lock().unwrap();
    while let Some(msg) = g.pop() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&msg) {
            if let Some(s) = v.get("__status").and_then(|x| x.as_str()) {
                model.status = s.into();
                continue;
            }
        }
        if let Ok(ps) = serde_json::from_str::<PlayerState>(&msg) {
            model.snapshot = Some(ps);
            last.0 = None;
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
                    apply_event_to_state(&mut snap.state, &ev);
                    last.0 = None;
                }
                continue;
            }
            if w.game_over.is_some() {
                model.status = "Game over".into();
            }
        }
    }
}

fn board_click(
    mouse: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    q_cell: Query<(&GlobalTransform, &Sprite, &BoardCell)>,
    mut model: ResMut<NetModel>,
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
    let Some(ray) = camera.viewport_to_world(cam_tf, cursor) else {
        return;
    };
    let world = ray.origin.truncate();

    let mut clicked_on_board = false;
    for (tf, sprite, cell) in q_cell.iter() {
        let center = tf.translation().truncate();
        let half = sprite.custom_size.unwrap_or(Vec2::ONE) * 0.5;
        let d = world - center;
        if d.x.abs() <= half.x && d.y.abs() <= half.y {
            clicked_on_board = true;
        }
        if d.x.abs() <= half.x && d.y.abs() <= half.y && (cell.row + cell.col) % 2 == 1 {
            let clicked = Cell {
                row: cell.row,
                col: cell.col,
            };
            if let Some(snap) = model.snapshot.as_ref() {
                let allowed = legal_next_cells(&snap.state, snap.player, &model.path);
                if !allowed.contains(&clicked) {
                    model.status = "That square is not a legal next step".into();
                    break;
                }
            }
            model.path.push(clicked);
            break;
        }
    }
    if !clicked_on_board {
        model.path.clear();
    }
}

fn button_send(
    interaction: Query<&Interaction, (Changed<Interaction>, With<SendBtn>)>,
    mut model: ResMut<NetModel>,
    ws: Res<WsHandle>,
) {
    for i in interaction.iter() {
        if *i != Interaction::Pressed {
            continue;
        }
        if model.path.len() < 2 {
            model.status = "Path needs at least 2 cells".into();
            continue;
        }
        let Some(snap) = model.snapshot.as_ref() else {
            model.status = "No game state yet".into();
            continue;
        };
        let path = MovePath(model.path.clone());
        if let Err(e) = snap.validate_move_for_send(&path) {
            model.status = e;
            continue;
        }
        let json = match serde_json::to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let g = ws.send.lock().unwrap();
        if let Some(socket) = g.as_ref() {
            let _ = socket.send_with_str(&json);
            model.path.clear();
            model.status = "Move sent".into();
        } else {
            model.status = "No socket".into();
        }
    }
}

fn button_clear(
    interaction: Query<&Interaction, (Changed<Interaction>, With<ClearBtn>)>,
    mut model: ResMut<NetModel>,
) {
    for i in interaction.iter() {
        if *i == Interaction::Pressed {
            model.path.clear();
        }
    }
}

fn sync_pieces(
    mut commands: Commands,
    model: Res<NetModel>,
    mut store: ResMut<PieceEntities>,
    mut last: ResMut<LastBoardSig>,
    assets: Res<CheckersVisualAssets>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Some(snap) = model.snapshot.as_ref() else {
        if !store.0.is_empty() {
            for e in store.0.drain(..) {
                commands.entity(e).despawn();
            }
            last.0 = None;
        }
        return;
    };
    let sig = board_signature(snap);
    let seat = snap.player;
    if last.0 == Some((sig, seat)) {
        return;
    }
    last.0 = Some((sig, seat));
    for e in store.0.drain(..) {
        commands.entity(e).despawn();
    }
    let board = snap.state.board.cells();
    for i in 0..64 {
        let Some(piece) = board.get(i).copied().flatten() else {
            continue;
        };
        let row = (i / 8) as u8;
        let col = (i % 8) as u8;
        if (row + col) % 2 == 0 {
            continue;
        }
        let (x, y) = board_cell_world_xy(row, col, seat);
        let (pr, pg, pb) = match piece.owner {
            Player::Dark => (0.15, 0.12, 0.12),
            Player::Light => (0.92, 0.9, 0.88),
        };
        let mesh_h = if piece.kind == PieceKind::King {
            assets.king_mesh.clone()
        } else {
            assets.man_mesh.clone()
        };
        let mat = materials.add(ColorMaterial::from(Color::srgb(pr, pg, pb)));
        let e = commands
            .spawn((
                MaterialMesh2dBundle {
                    mesh: mesh_h.into(),
                    material: mat,
                    transform: Transform::from_xyz(x, y, 1.0),
                    ..default()
                },
                PieceMark,
            ))
            .id();
        store.0.push(e);
    }
}

fn sync_legal_highlights(
    mut commands: Commands,
    model: Res<NetModel>,
    mut store: ResMut<HighlightEntities>,
    mut last: ResMut<LastHighlightSig>,
    assets: Res<CheckersVisualAssets>,
) {
    let clear_all = |commands: &mut Commands, store: &mut HighlightEntities, last: &mut LastHighlightSig| {
        for e in store.0.drain(..) {
            commands.entity(e).despawn();
        }
        last.0 = None;
    };

    let Some(snap) = model.snapshot.as_ref() else {
        clear_all(&mut commands, &mut store, &mut *last);
        return;
    };

    let sig = board_signature(snap);
    let seat = snap.player;
    let path = model.path.clone();
    if last.0.as_ref() == Some(&(sig, path.clone(), seat)) {
        return;
    }
    last.0 = Some((sig, path, seat));

    for e in store.0.drain(..) {
        commands.entity(e).despawn();
    }

    let cells = legal_next_cells(&snap.state, snap.player, &model.path);
    for c in cells {
        let (x, y) = board_cell_world_xy(c.row, c.col, seat);
        let e = commands
            .spawn((
                MaterialMesh2dBundle {
                    mesh: assets.legal_mesh.clone().into(),
                    material: assets.legal_material.clone(),
                    transform: Transform::from_xyz(x, y, 0.5),
                    ..default()
                },
                LegalMoveHint,
            ))
            .id();
        store.0.push(e);
    }
}

fn spawn_path_segment(
    commands: &mut Commands,
    store: &mut PathVizEntities,
    a: Vec2,
    b: Vec2,
    color: Color,
    z: f32,
    thickness: f32,
) {
    let d = b - a;
    let len = d.length();
    if len <= f32::EPSILON {
        return;
    }
    let mid = (a + b) * 0.5;
    let angle = d.y.atan2(d.x);
    let e = commands
        .spawn((
            SpriteBundle {
                sprite: Sprite {
                    color,
                    custom_size: Some(Vec2::new(len, thickness)),
                    ..default()
                },
                transform: Transform::from_xyz(mid.x, mid.y, z)
                    .with_rotation(Quat::from_rotation_z(angle)),
                ..default()
            },
            BuiltPathViz,
        ))
        .id();
    store.0.push(e);
}

fn sync_built_path_visual(
    mut commands: Commands,
    model: Res<NetModel>,
    mut store: ResMut<PathVizEntities>,
    mut last: ResMut<LastPathVizSig>,
    assets: Res<CheckersVisualAssets>,
) {
    let clear_all = |commands: &mut Commands, store: &mut PathVizEntities, last: &mut LastPathVizSig| {
        for e in store.0.drain(..) {
            commands.entity(e).despawn();
        }
        last.0 = None;
    };

    let Some(snap) = model.snapshot.as_ref() else {
        clear_all(&mut commands, &mut store, &mut *last);
        return;
    };
    if model.path.is_empty() {
        clear_all(&mut commands, &mut store, &mut *last);
        return;
    }

    let sig = board_signature(snap);
    let seat = snap.player;
    let path = model.path.clone();
    if last.0.as_ref() == Some(&(sig, path.clone(), seat)) {
        return;
    }
    last.0 = Some((sig, path.clone(), seat));

    for e in store.0.drain(..) {
        commands.entity(e).despawn();
    }

    let line_color = Color::srgba(1.0, 0.62, 0.12, 0.82);
    for (idx, c) in path.iter().copied().enumerate() {
        let (x, y) = board_cell_world_xy(c.row, c.col, seat);
        let e = commands
            .spawn((
                MaterialMesh2dBundle {
                    mesh: assets.path_node_mesh.clone().into(),
                    material: assets.path_node_material.clone(),
                    transform: Transform::from_xyz(x, y, 1.35),
                    ..default()
                },
                BuiltPathViz,
            ))
            .id();
        store.0.push(e);

        if idx == 0 {
            continue;
        }
        let prev = path[idx - 1];
        let (ax, ay) = board_cell_world_xy(prev.row, prev.col, seat);
        let a = Vec2::new(ax, ay);
        let b = Vec2::new(x, y);
        let dr = (c.row as i16 - prev.row as i16).unsigned_abs();
        let dc = (c.col as i16 - prev.col as i16).unsigned_abs();
        if dr == 1 && dc == 1 {
            spawn_path_segment(&mut commands, &mut store, a, b, line_color, 1.32, 4.0);
        } else {
            // Curved jump connector (quadratic sampled polyline) so it doesn't cut through centers.
            let d = b - a;
            let n = Vec2::new(-d.y, d.x).normalize_or_zero();
            let sign = if idx % 2 == 0 { 1.0 } else { -1.0 };
            let ctrl = (a + b) * 0.5 + n * (CELL * 0.42 * sign);
            let mut prev_p = a;
            let steps = 14usize;
            for s in 1..=steps {
                let t = s as f32 / steps as f32;
                let p = (1.0 - t) * (1.0 - t) * a + 2.0 * (1.0 - t) * t * ctrl + t * t * b;
                spawn_path_segment(&mut commands, &mut store, prev_p, p, line_color, 1.32, 4.2);
                prev_p = p;
            }
        }
    }
}

fn update_status_text(model: Res<NetModel>, mut q: Query<&mut Text, With<StatusText>>) {
    let path_s = model
        .path
        .iter()
        .map(|c| format!("({},{})", c.row, c.col))
        .collect::<Vec<_>>()
        .join(" ");
    let turn = model
        .snapshot
        .as_ref()
        .map(|s| format!("{:?}", s.state.current_player))
        .unwrap_or_else(|| "?".into());
    let you = model
        .snapshot
        .as_ref()
        .map(|s| format!("{:?}", s.player))
        .unwrap_or_else(|| "?".into());
    let line = format!(
        "{} | You: {} | Turn: {} | Path: {}",
        model.status, you, turn, path_s
    );
    for mut t in q.iter_mut() {
        if let Some(s) = t.sections.first_mut() {
            s.value = line.clone();
        }
    }
}

