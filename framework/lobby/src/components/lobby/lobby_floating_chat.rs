use crate::api::graphql_exec;
use crate::models::LobbyMessage;
use crate::components::ui::Icon;
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn LobbyFloatingChat(
    lobby_id: String,
    messages: Vec<LobbyMessage>,
    my_user_id: Option<String>,
) -> Element {
    let mut open = use_signal(|| false);
    let mut draft = use_signal(|| String::new());
    let lid_send = lobby_id.clone();
    let msg_count = messages.len();
    let preview = messages.last().map(|m| m.body.as_str()).unwrap_or("");

    rsx! {
        if open() {
            div { class: "lobby-chat-panel",
                div { class: "lobby-chat-panel-head",
                    div {
                        p { class: "lobby-section-kicker", "COMMS" }
                        p { class: "font-manrope font-semibold text-on-surface text-sm", "Squad chat" }
                    }
                    button {
                        class: "lobby-drawer-close",
                        onclick: move |_| open.set(false),
                        Icon { name: "close", filled: false }
                    }
                }
                div { class: "lobby-chat-panel-messages",
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
                                        span { class: "text-primary font-medium text-xs block mb-0.5",
                                            "{m.display_name}"
                                        }
                                    }
                                    "{m.body}"
                                }
                            }
                        }
                    }
                }
                div { class: "lobby-chat-panel-compose",
                    textarea {
                        class: "input-field min-h-[2.75rem] text-sm",
                        placeholder: "Message squad…",
                        value: "{draft()}",
                        oninput: move |e| draft.set(e.value()),
                    }
                    button {
                        class: "btn-primary text-sm py-2",
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
        } else {
            button {
                class: "lobby-chat-fab",
                onclick: move |_| open.set(true),
                Icon { name: "chat", filled: false }
                span { class: "lobby-chat-fab-label", "Chat" }
                if msg_count > 0 {
                    span { class: "lobby-chat-fab-badge", "{msg_count}" }
                }
                if !preview.is_empty() {
                    span { class: "lobby-chat-fab-preview truncate", "{preview}" }
                }
            }
        }
    }
}
