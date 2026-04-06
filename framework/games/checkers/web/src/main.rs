//! Bevy **WASM** play client for checkers. URL query: `ws`, `id`, `player` (lobby iframe).
//! Click **dark** squares to append [`Cell`]s to the path; with 2+ cells a purple circle and a
//! two-bar mark above the last square confirms and sends the move (same as **Send move**).
//! Clicks outside the board clear the path (HUD top-left is excluded so **Send** / **Clear** work).

use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::ui::{BorderColor, BorderRadius};
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
/// Confirm control sits toward +Y from cell center (top of square on screen).
const PATH_CONFIRM_TOP_OFFSET: f32 = CELL * 0.38;
const PATH_CONFIRM_HIT_RADIUS: f32 = 18.0;

/// Window-space rect (origin top-left, Y down): clicks here skip “discard path” so HUD buttons work.
const HUD_PATH_CLEAR_EXCLUSION: Rect = Rect {
    min: Vec2::new(0.0, 0.0),
    max: Vec2::new(420.0, 560.0),
};

/// HUD palette aligned with the wooden board tones.
const HUD_PANEL_BG: Color = Color::srgb(0.13, 0.11, 0.10);
const HUD_PANEL_BORDER: Color = Color::srgb(0.46, 0.38, 0.30);
const HUD_MUTED: Color = Color::srgb(0.58, 0.54, 0.50);
const HUD_TITLE: Color = Color::srgb(0.97, 0.94, 0.88);
const HUD_DIVIDER: Color = Color::srgb(0.32, 0.28, 0.24);
const HUD_PATH: Color = Color::srgb(0.98, 0.70, 0.36);
const HUD_BTN_SEND: Color = Color::srgb(0.20, 0.50, 0.38);
const HUD_BTN_CLEAR: Color = Color::srgb(0.50, 0.26, 0.24);

#[derive(Resource, Default, Clone)]
struct WsInbox(Arc<Mutex<Vec<String>>>);

#[derive(Resource, Default)]
struct NetModel {
    snapshot: Option<PlayerState>,
    path: Vec<Cell>,
    status: String,
}

/// Clears stale send/path messages once the player changes the path (e.g. after "Path needs at least 2 cells"
/// while building a valid path, the HUD line must not keep prefixing that error).
fn refresh_status_after_path_edit(model: &mut NetModel) {
    if model.status.starts_with("Game over") {
        return;
    }
    if model.snapshot.is_some() {
        model.status = "In game".into();
    }
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
                sync_pieces,
                sync_legal_highlights,
                sync_built_path_visual,
                sync_path_confirm_overlay,
                button_send,
                button_clear,
                board_click,
                update_hud,
            ),
        )
        .init_resource::<LastBoardSig>()
        .init_resource::<LastHighlightSig>()
        .init_resource::<LastPathVizSig>()
        .init_resource::<HighlightEntities>()
        .init_resource::<PathConfirmState>()
        .run();
}

#[derive(Component)]
struct BoardCell {
    row: u8,
    col: u8,
}

#[derive(Component)]
struct HudStatusLine;

#[derive(Component)]
struct HudYouRole;

#[derive(Component)]
struct HudTurnRole;

#[derive(Component)]
struct HudPathLine;

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

#[derive(Component)]
struct PathConfirmRoot;

/// On-board send control (purple circle + check) above the last path cell.
#[derive(Resource, Default)]
struct PathConfirmState {
    entity: Option<Entity>,
    cache: Option<(u64, usize, Cell, Player)>,
}

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
    confirm_bg_mesh: Handle<Mesh>,
    confirm_bg_material: Handle<ColorMaterial>,
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
        confirm_bg_mesh: meshes.add(Mesh::from(Circle::new(15.0))),
        confirm_bg_material: materials.add(ColorMaterial::from(Color::srgb(0.55, 0.28, 0.82))),
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

/// Board layout: one coordinate system for sprites, pieces, highlights, and picking.
/// Logical `(row,col)` is the wire/game state. Seat `Dark` rotates the view 180° (near side + correct left/right).
mod board_geom {
    use super::{Cell, Player, Vec2, CELL};

    /// World position of the **center** of cell (0,0) in view space (top-left on screen for Light).
    const ORIGIN: Vec2 = Vec2::new(-4.0 * CELL + 0.5 * CELL, 4.0 * CELL - 0.5 * CELL);

    #[inline]
    fn logical_to_view(row: u8, col: u8, seat: Player) -> (u8, u8) {
        match seat {
            Player::Light => (row, col),
            Player::Dark => (7u8.saturating_sub(row), 7u8.saturating_sub(col)),
        }
    }

    #[inline]
    fn view_to_logical(view_row: u8, view_col: u8, seat: Player) -> Cell {
        match seat {
            Player::Light => Cell {
                row: view_row,
                col: view_col,
            },
            Player::Dark => Cell {
                row: 7u8.saturating_sub(view_row),
                col: 7u8.saturating_sub(view_col),
            },
        }
    }

    /// Center of the logical cell in world space (must match spawned tile sprites).
    pub fn cell_center_world(logical: Cell, seat: Player) -> Vec2 {
        let (vr, vc) = logical_to_view(logical.row, logical.col, seat);
        Vec2::new(
            ORIGIN.x + vc as f32 * CELL,
            ORIGIN.y - vr as f32 * CELL,
        )
    }

    /// Inverse of [`cell_center_world`]: which logical dark square was hit (if any).
    pub fn world_to_logical_cell(world: Vec2, seat: Player) -> Option<Cell> {
        let dx = (world.x - ORIGIN.x) / CELL;
        let dy = (ORIGIN.y - world.y) / CELL;
        let vr = (dy + 0.5).floor() as i32;
        let vc = (dx + 0.5).floor() as i32;
        if !(0..=7).contains(&vr) || !(0..=7).contains(&vc) {
            return None;
        }
        Some(view_to_logical(vr as u8, vc as u8, seat))
    }
}

use board_geom::{cell_center_world, world_to_logical_cell};

#[inline]
fn board_cell_world_xy(logical_row: u8, col: u8, seat: Player) -> (f32, f32) {
    let p = cell_center_world(Cell { row: logical_row, col }, seat);
    (p.x, p.y)
}

#[inline]
fn path_confirm_anchor(last: Cell, seat: Player) -> Vec2 {
    cell_center_world(last, seat) + Vec2::new(0.0, PATH_CONFIRM_TOP_OFFSET)
}

fn send_move_if_ready(model: &mut NetModel, ws: &WsHandle) {
    if model.path.len() < 2 {
        model.status = "Path needs at least 2 cells".into();
        return;
    }
    let Some(snap) = model.snapshot.as_ref() else {
        model.status = "No game state yet".into();
        return;
    };
    let path = MovePath(model.path.clone());
    if let Err(e) = snap.validate_move_for_send(&path) {
        model.status = e;
        return;
    }
    let json = match serde_json::to_string(&path) {
        Ok(s) => s,
        Err(_) => return,
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

    for row in 0..8u8 {
        for col in 0..8u8 {
            let dark = (row + col) % 2 == 1;
            let p = cell_center_world(Cell { row, col }, Player::Light);
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
                    transform: Transform::from_xyz(p.x, p.y, 0.0),
                    ..default()
                },
                BoardCell { row, col },
            ));
        }
    }

    let label_style = TextStyle {
        font_size: 12.0,
        color: HUD_MUTED,
        ..default()
    };
    let value_style = TextStyle {
        font_size: 14.0,
        color: HUD_TITLE,
        ..default()
    };

    commands
        .spawn(NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(16.0)),
                min_width: Val::Px(268.0),
                max_width: Val::Px(360.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            background_color: BackgroundColor(HUD_PANEL_BG),
            border_color: BorderColor(HUD_PANEL_BORDER),
            border_radius: BorderRadius::all(Val::Px(12.0)),
            ..default()
        })
        .with_children(|p| {
            p.spawn(TextBundle::from_section(
                "Checkers",
                TextStyle {
                    font_size: 20.0,
                    color: HUD_TITLE,
                    ..default()
                },
            ));
            p.spawn(TextBundle::from_section(
                "Tap dark squares to build a path, then send your move.",
                TextStyle {
                    font_size: 12.5,
                    color: HUD_MUTED,
                    ..default()
                },
            ));
            p.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    margin: UiRect::vertical(Val::Px(2.0)),
                    ..default()
                },
                background_color: BackgroundColor(HUD_DIVIDER),
                ..default()
            });
            p.spawn(TextBundle::from_section(
                "Message",
                TextStyle {
                    font_size: 11.0,
                    color: HUD_MUTED,
                    ..default()
                },
            ));
            p.spawn((
                TextBundle::from_section(
                    "…",
                    TextStyle {
                        font_size: 15.0,
                        color: Color::srgb(0.78, 0.82, 0.90),
                        ..default()
                    },
                ),
                HudStatusLine,
            ));
            p.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(28.0),
                    align_items: AlignItems::FlexStart,
                    ..default()
                },
                ..default()
            })
            .with_children(|row| {
                row.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|col| {
                    col.spawn(TextBundle::from_section(
                        "You play as",
                        label_style.clone(),
                    ));
                    col.spawn((
                        TextBundle::from_section("?", value_style.clone()),
                        HudYouRole,
                    ));
                });
                row.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|col| {
                    col.spawn(TextBundle::from_section(
                        "Current turn",
                        label_style.clone(),
                    ));
                    col.spawn((
                        TextBundle::from_section("?", value_style.clone()),
                        HudTurnRole,
                    ));
                });
            });
            p.spawn((
                TextBundle::from_sections([
                    TextSection::new("Path\n", label_style.clone()),
                    TextSection::new(
                        "—",
                        TextStyle {
                            font_size: 14.0,
                            color: HUD_PATH,
                            ..default()
                        },
                    ),
                ]),
                HudPathLine,
            ));
            p.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    align_items: AlignItems::Center,
                    margin: UiRect::top(Val::Px(6.0)),
                    ..default()
                },
                ..default()
            })
            .with_children(|btn_row| {
                btn_row
                    .spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            background_color: BackgroundColor(HUD_BTN_SEND),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        SendBtn,
                    ))
                    .with_children(|c| {
                        c.spawn(TextBundle::from_section(
                            "Send move",
                            TextStyle {
                                font_size: 14.0,
                                color: HUD_TITLE,
                                ..default()
                            },
                        ));
                    });
                btn_row
                    .spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            background_color: BackgroundColor(HUD_BTN_CLEAR),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        ClearBtn,
                    ))
                    .with_children(|c| {
                        c.spawn(TextBundle::from_section(
                            "Clear path",
                            TextStyle {
                                font_size: 14.0,
                                color: HUD_TITLE,
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
    let Some(ray) = camera.viewport_to_world(cam_tf, cursor) else {
        return;
    };
    let world = ray.origin.truncate();

    let seat = model
        .snapshot
        .as_ref()
        .map(|s| s.player)
        .unwrap_or(Player::Light);

    if model.path.len() >= 2 {
        if let Some(snap) = model.snapshot.as_ref() {
            let last = *model.path.last().expect("len >= 2");
            let anchor = path_confirm_anchor(last, snap.player);
            if world.distance(anchor) <= PATH_CONFIRM_HIT_RADIUS {
                send_move_if_ready(&mut model, &ws);
                return;
            }
        }
    }

    let Some(clicked) = world_to_logical_cell(world, seat) else {
        if !HUD_PATH_CLEAR_EXCLUSION.contains(cursor) {
            model.path.clear();
            refresh_status_after_path_edit(&mut model);
        }
        return;
    };

    if (clicked.row + clicked.col) % 2 == 0 {
        return;
    }
    if let Some(snap) = model.snapshot.as_ref() {
        let allowed = legal_next_cells(&snap.state, snap.player, &model.path);
        if !allowed.contains(&clicked) {
            model.status = "That square is not a legal next step".into();
            return;
        }
    }
    model.path.push(clicked);
    refresh_status_after_path_edit(&mut model);
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
        send_move_if_ready(&mut model, &ws);
    }
}

fn button_clear(
    interaction: Query<&Interaction, (Changed<Interaction>, With<ClearBtn>)>,
    mut model: ResMut<NetModel>,
) {
    for i in interaction.iter() {
        if *i == Interaction::Pressed {
            model.path.clear();
            refresh_status_after_path_edit(&mut model);
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

fn sync_path_confirm_overlay(
    mut commands: Commands,
    model: Res<NetModel>,
    mut state: ResMut<PathConfirmState>,
    assets: Res<CheckersVisualAssets>,
) {
    let clear = |commands: &mut Commands, state: &mut PathConfirmState| {
        if let Some(e) = state.entity.take() {
            commands.entity(e).despawn_recursive();
        }
        state.cache = None;
    };

    let Some(snap) = model.snapshot.as_ref() else {
        clear(&mut commands, &mut state);
        return;
    };

    if model.path.len() < 2 {
        clear(&mut commands, &mut state);
        return;
    }

    let seat = snap.player;
    let last = *model.path.last().expect("len >= 2");
    let sig = board_signature(snap);
    let key = (sig, model.path.len(), last, seat);

    if state.cache == Some(key) && state.entity.is_some() {
        return;
    }

    clear(&mut commands, &mut state);

    let anchor = path_confirm_anchor(last, seat);
    let id = commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_xyz(anchor.x, anchor.y, 1.58)),
            PathConfirmRoot,
        ))
        .with_children(|c| {
            c.spawn(MaterialMesh2dBundle {
                mesh: assets.confirm_bg_mesh.clone().into(),
                material: assets.confirm_bg_material.clone(),
                transform: Transform::from_xyz(0.0, 0.0, 0.0),
                ..default()
            });
            let mark = Color::srgb(0.96, 0.96, 1.0);
            // Sharp ~90° “L”: vertical stem + horizontal base (corner toward bottom-left of circle).
            c.spawn(SpriteBundle {
                sprite: Sprite {
                    color: mark,
                    custom_size: Some(Vec2::new(2.4, 6.2)),
                    ..default()
                },
                transform: Transform::from_xyz(-2.35, 0.85, 0.002),
                ..default()
            });
            c.spawn(SpriteBundle {
                sprite: Sprite {
                    color: mark,
                    custom_size: Some(Vec2::new(8.0, 2.4)),
                    ..default()
                },
                transform: Transform::from_xyz(1.45, -2.32, 0.002),
                ..default()
            });
        })
        .id();

    state.entity = Some(id);
    state.cache = Some(key);
}

fn hud_status_color(status: &str) -> Color {
    let lower = status.to_lowercase();
    if status.starts_with("Game over") {
        Color::srgb(0.92, 0.72, 0.48)
    } else if lower.contains("not a legal")
        || lower.contains("needs at least")
        || lower.contains("no game state")
        || lower.contains("no socket")
        || lower.contains("failed")
        || lower.contains("error")
        || lower.contains("illegal")
        || lower.contains("missing")
    {
        Color::srgb(0.94, 0.52, 0.48)
    } else if lower.contains("move sent")
        || lower == "in game"
        || lower.contains("connected")
    {
        Color::srgb(0.48, 0.88, 0.65)
    } else {
        Color::srgb(0.78, 0.82, 0.90)
    }
}

fn hud_player_color(label: &str) -> Color {
    match label {
        "Dark" => Color::srgb(0.74, 0.70, 0.88),
        "Light" => Color::srgb(0.98, 0.95, 0.78),
        _ => Color::srgb(0.85, 0.82, 0.78),
    }
}

fn update_hud(
    model: Res<NetModel>,
    mut q_status: Query<
        &mut Text,
        (
            With<HudStatusLine>,
            Without<HudYouRole>,
            Without<HudTurnRole>,
            Without<HudPathLine>,
        ),
    >,
    mut q_you: Query<
        &mut Text,
        (
            With<HudYouRole>,
            Without<HudStatusLine>,
            Without<HudTurnRole>,
            Without<HudPathLine>,
        ),
    >,
    mut q_turn: Query<
        &mut Text,
        (
            With<HudTurnRole>,
            Without<HudStatusLine>,
            Without<HudYouRole>,
            Without<HudPathLine>,
        ),
    >,
    mut q_path: Query<
        &mut Text,
        (
            With<HudPathLine>,
            Without<HudStatusLine>,
            Without<HudYouRole>,
            Without<HudTurnRole>,
        ),
    >,
) {
    let you = model
        .snapshot
        .as_ref()
        .map(|s| format!("{:?}", s.player))
        .unwrap_or_else(|| "?".into());
    let turn = model
        .snapshot
        .as_ref()
        .map(|s| format!("{:?}", s.state.current_player))
        .unwrap_or_else(|| "?".into());

    let path_s = model
        .path
        .iter()
        .map(|c| format!("({}, {})", c.row, c.col))
        .collect::<Vec<_>>()
        .join(" → ");
    let path_display = if path_s.is_empty() {
        "—".into()
    } else {
        path_s
    };

    for mut t in q_status.iter_mut() {
        if let Some(s) = t.sections.first_mut() {
            s.value = model.status.clone();
            s.style.color = hud_status_color(&model.status);
        }
    }
    for mut t in q_you.iter_mut() {
        if let Some(s) = t.sections.first_mut() {
            s.value = you.clone();
            s.style.color = hud_player_color(you.as_str());
        }
    }
    for mut t in q_turn.iter_mut() {
        if let Some(s) = t.sections.first_mut() {
            s.value = turn.clone();
            s.style.color = hud_player_color(turn.as_str());
        }
    }
    for mut t in q_path.iter_mut() {
        if t.sections.len() >= 2 {
            t.sections[1].value = path_display.clone();
        }
    }
}

