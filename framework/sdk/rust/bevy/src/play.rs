use bevy::prelude::*;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use upjs_gdd_rust_shared::play::{PlayClient, PlayClientConfig};

#[derive(Resource, Clone, Default)]
pub struct PlayInbox(pub Arc<Mutex<Vec<Value>>>);

#[derive(Resource)]
pub struct GamePlayResource {
    pub client: PlayClient,
    pub status: String,
    pub initial_received: bool,
}

#[derive(Event, Clone)]
pub struct GameStateReceived(pub Value);

#[derive(Event, Clone)]
pub struct GameEventReceived(pub Value);

#[derive(Event, Clone)]
pub struct GamePlayStatus(pub String);

pub struct FrameworkGamePlayPlugin;

impl Plugin for FrameworkGamePlayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlayInbox::default())
            .insert_resource(GamePlayResource {
                client: PlayClient::new(),
                status: "…".into(),
                initial_received: false,
            })
            .add_event::<GameStateReceived>()
            .add_event::<GameEventReceived>()
            .add_event::<GamePlayStatus>()
            .add_systems(Startup, connect_play_client)
            .add_systems(Update, drain_play_inbox);
    }
}

fn connect_play_client(
    inbox: Res<PlayInbox>,
    mut res: ResMut<GamePlayResource>,
    mut ev_status: EventWriter<GamePlayStatus>,
) {
    let Some(cfg) = PlayClientConfig::from_window_location() else {
        res.status = "Missing lobby URL params (ws, id, player)".into();
        ev_status.send(GamePlayStatus(res.status.clone()));
        return;
    };
    let inbox_c = inbox.0.clone();
    let inbox_e = inbox.0.clone();
    res.client.connect(
        &cfg,
        move |v| {
            inbox_c.lock().unwrap().push(v);
        },
        move |v| {
            inbox_e.lock().unwrap().push(v);
        },
        move |s| {
            let _ = s;
        },
    );
    res.status = "Connecting…".into();
    ev_status.send(GamePlayStatus(res.status.clone()));
}

fn drain_play_inbox(
    inbox: Res<PlayInbox>,
    mut res: ResMut<GamePlayResource>,
    mut ev_state: EventWriter<GameStateReceived>,
    mut ev_event: EventWriter<GameEventReceived>,
) {
    let mut g = inbox.0.lock().unwrap();
    while let Some(v) = g.pop() {
        if !res.initial_received {
            res.initial_received = true;
            res.status = "In game".into();
            ev_state.send(GameStateReceived(v));
        } else {
            ev_event.send(GameEventReceived(v));
        }
    }
}

/// Send a JSON action on the active play socket.
pub fn send_play_action(res: &GamePlayResource, action: &Value) -> Result<(), String> {
    res.client.send_action_value(action)
}
