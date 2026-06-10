use crate::api::{get_ws_base, graphql_exec};
use crate::components::ui::Icon;
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn GamePlayer(
    game_type: String,
    game_id: String,
    player: String,
    return_lobby_id: Option<String>,
    on_close: EventHandler<()>,
    on_navigate_lobby: EventHandler<String>,
) -> Element {
    let ws_base = get_ws_base();
    let player_q = urlencoding::encode(&player);
    let iframe_src = format!(
        "/games/{game_type}/?ws={ws_base}&id={game_id}&player={player_q}"
    );
    let ret = return_lobby_id.clone();

    rsx! {
        div { class: "flex flex-col h-screen fixed inset-0 z-50 bg-background",
            div { class: "flex items-center gap-4 px-4 py-3 border-b border-outline-variant/40 bg-surface-container-lowest/95 backdrop-blur-md",
                button {
                    class: "btn-ghost",
                    onclick: move |_| {
                        let lid = ret.clone();
                        spawn(async move {
                            if let Some(ref id) = lid {
                                let q = "mutation R($id: ID!) { reopenLobbyAfterGame(lobbyId: $id) }";
                                let vars = serde_json::json!({ "id": id });
                                let _ = graphql_exec::<Value>(q, Some(vars)).await;
                            }
                            on_close.call(());
                            if let Some(id) = lid {
                                on_navigate_lobby.call(id);
                            }
                        });
                    },
                    Icon { name: "arrow_back", filled: false }
                    "Back to lobby"
                }
                span { class: "text-body-sm text-on-surface-variant",
                    "Playing "
                    span { class: "font-semibold text-on-surface", "{game_type}" }
                    " as "
                    span { class: "text-tertiary font-mono-code text-xs sm:text-sm", "{player}" }
                }
            }
            iframe {
                class: "flex-1 w-full border-0 bg-surface-container-lowest",
                src: "{iframe_src}",
            }
        }
    }
}
