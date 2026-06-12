mod api;
mod components;
mod models;
mod pages;
mod stub;

use api::*;
use components::lobby::GamePlayer;
use components::{AppShell, LoadingState, ToastProvider};
use dioxus::prelude::*;
use models::*;
use pages::{
    AuthGate, DeveloperUploadsPage, GameDetailPage, GameResultPage, GamesListPage, HomePage,
    LobbiesBrowserPage, LobbyRoomPage, ProfilePage, SettingsPage,
};

#[derive(Clone, Copy)]
pub struct AppShellContext {
    pub playing: Signal<Option<PlayOverlay>>,
    pub error_msg: Signal<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Routable)]
#[rustfmt::skip]
pub enum LobbyRoute {
    #[layout(OverlayLayout)]
    #[route("/", HomePageRoute)]
    Home {},
    #[route("/games", GamesListRoute)]
    GamesList {},
    #[route("/games/:name", GameDetailRoute)]
    GameDetail { name: String },
    #[route("/lobbies", LobbiesBrowserRoute)]
    LobbiesBrowser {},
    #[route("/settings", SettingsRoute)]
    Settings {},
    #[route("/profile", ProfileRoute)]
    Profile {},
    #[route("/lobby/:id", LobbyRoomRoute)]
    Lobby { id: String },
    #[route("/game/:id", GameResultRoute)]
    GameResult { id: String },
    #[route("/developer/uploads", DeveloperUploadsRoute)]
    DeveloperUploads {},
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn HomePageRoute() -> Element {
    let shell = use_context::<AppShellContext>();
    rsx! {
        HomePage {
            playing: shell.playing,
            error_msg: shell.error_msg,
        }
    }
}

#[component]
fn GamesListRoute() -> Element {
    rsx! { GamesListPage {} }
}

#[component]
fn GameDetailRoute(name: String) -> Element {
    rsx! {
        GameDetailPage { key: "{name}", name }
    }
}

#[component]
fn LobbiesBrowserRoute() -> Element {
    rsx! { LobbiesBrowserPage {} }
}

#[component]
fn SettingsRoute() -> Element {
    rsx! { SettingsPage {} }
}

#[component]
fn ProfileRoute() -> Element {
    rsx! { ProfilePage {} }
}

#[component]
fn LobbyRoomRoute(id: String) -> Element {
    let shell = use_context::<AppShellContext>();
    rsx! {
        LobbyRoomPage {
            key: "{id}",
            lobby_id: id,
            playing: shell.playing,
            error_msg: shell.error_msg,
        }
    }
}

#[component]
fn GameResultRoute(id: String) -> Element {
    rsx! {
        GameResultPage {
            key: "{id}",
            game_id: id,
        }
    }
}

#[component]
fn DeveloperUploadsRoute() -> Element {
    rsx! {
        DeveloperUploadsPage {}
    }
}

#[component]
pub fn OverlayLayout() -> Element {
    let mut shell = use_context::<AppShellContext>();
    let nav = use_navigator();
    rsx! {
        ToastProvider {
            AppShell {
                Outlet::<LobbyRoute> {}
            }
        }
        if let Some(p) = (shell.playing)() {
            GamePlayer {
                game_type: p.game_type.clone(),
                game_id: p.game_id.clone(),
                player: p.player.clone(),
                return_lobby_id: p.return_lobby_id.clone(),
                spectator: p.spectator,
                on_close: move |_| {
                    shell.playing.set(None);
                },
                on_navigate_lobby: move |id: String| {
                    nav.push(LobbyRoute::Lobby { id });
                },
            }
        }
    }
}

#[component]
fn AuthedShell(playing: Signal<Option<PlayOverlay>>, error_msg: Signal<Option<String>>) -> Element {
    use_context_provider(|| AppShellContext { playing, error_msg });
    rsx! {
        Router::<LobbyRoute> {}
    }
}

#[component]
fn App() -> Element {
    let mut session_ok: Signal<bool> = use_signal(|| false);
    let mut session_checked: Signal<bool> = use_signal(|| false);
    let mut playing: Signal<Option<PlayOverlay>> = use_signal(|| None);
    let error_msg: Signal<Option<String>> = use_signal(|| None);

    use_effect(move || {
        let mut session_ok = session_ok;
        let mut session_checked = session_checked;
        let mut error_msg = error_msg;
        spawn(async move {
            if stored_user_id().is_none() {
                session_ok.set(false);
                session_checked.set(true);
                return;
            }
            let id = stored_user_id().unwrap();
            let q = r#"query UserExists($id: ID!) { user(id: $id) { id } }"#;
            let vars = serde_json::json!({ "id": id });
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct UserExists {
                user: Option<models::RegisterUserRow>,
            }
            match graphql_exec_anonymous::<UserExists>(q, Some(vars)).await {
                Ok(p) if p.user.is_some() => session_ok.set(true),
                _ => {
                    if let Some(st) = local_storage() {
                        let _ = st.remove_item(USER_ID_KEY);
                    }
                    session_ok.set(false);
                }
            }
            session_checked.set(true);
            error_msg.set(None);
        });
    });

    rsx! {
        document::Stylesheet {
            href: asset!("/assets/tailwind.css"),
        }
        div { class: "min-h-screen bg-background text-on-surface",
            if !session_checked() {
                LoadingState {
                    title: "Checking session…".to_string(),
                    subtitle: "Hang tight".to_string(),
                }
            } else if !session_ok() {
                AuthGate {
                    on_ready: move |_| {
                        session_ok.set(true);
                        session_checked.set(true);
                    }
                }
            } else {
                AuthedShell {
                    playing,
                    error_msg,
                }
            }
        }
    }
}
