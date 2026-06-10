use crate::api::graphql_exec;
use crate::models::LobbyMessage;
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn LobbyChatPanel(
    lobby_id: String,
    messages: Vec<LobbyMessage>,
    my_user_id: Option<String>,
) -> Element {
    let mut draft = use_signal(|| String::new());
    let lid_send = lobby_id.clone();
    rsx! {
        h4 { class: "font-label-caps text-label-caps text-secondary uppercase mt-6 mb-2", "Chat" }
        div { class: "lobby-chat-messages flex flex-col gap-2",
            for m in messages.iter().rev().take(40).rev() {
                {
                    let is_self = my_user_id.as_deref() == Some(m.user_id.as_str());
                    let is_system = m.display_name.eq_ignore_ascii_case("system");
                    let bubble_class = if is_system {
                        "chat-bubble chat-bubble-system"
                    } else if is_self {
                        "chat-bubble chat-bubble-self"
                    } else {
                        "chat-bubble chat-bubble-other"
                    };
                    rsx! {
                        div {
                            key: "{m.id}",
                            class: "{bubble_class}",
                            if !is_system {
                                span { class: "text-primary font-medium text-xs block mb-0.5", "{m.display_name}" }
                            }
                            "{m.body}"
                        }
                    }
                }
            }
        }
        textarea {
            class: "input-field mt-2 min-h-[3.25rem]",
            placeholder: "Message…",
            value: "{draft()}",
            oninput: move |e| draft.set(e.value()),
        }
        button {
            class: "btn-primary mt-2",
            onclick: move |_| {
                let body = draft();
                if body.trim().is_empty() { return; }
                let lid = lid_send.clone();
                spawn(async move {
                    let q = "mutation P($id: ID!, $b: String!) { postLobbyMessage(lobbyId: $id, body: $b) { id } }";
                    let vars = serde_json::json!({ "id": lid, "b": body });
                    let _ = graphql_exec::<Value>(q, Some(vars)).await;
                });
                draft.set(String::new());
            },
            "Send"
        }
    }
}
