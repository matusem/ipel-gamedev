use crate::api::graphql_exec;
use crate::components::ui::{push_toast, use_toast, Avatar, AvatarSize, Chip, EmptyState, Icon, SearchInput, ToastKind};
use crate::models::LobbyDetail;
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn LobbyBotPicker(
    open: bool,
    on_close: EventHandler<()>,
    lobby_id: String,
    game_type: String,
    seat_index: i32,
    player_identity: String,
    on_detail_updated: EventHandler<LobbyDetail>,
) -> Element {
    if !open {
        return rsx! {};
    }

    let toast = use_toast();
    let mut bots: Signal<Vec<Value>> = use_signal(Vec::new);
    let mut loading: Signal<bool> = use_signal(|| true);
    let mut filter: Signal<String> = use_signal(String::new);

    let gt = game_type.clone();
    use_effect(move || {
        let gt = gt.clone();
        let mut bots = bots;
        let mut loading = loading;
        spawn(async move {
            loading.set(true);
            let q = r#"query B($slug: String!) { compatibleBots(gameSlug: $slug) { id slug displayName version contractHash avatarSeed settingsSchemaJson settingsJson } }"#;
            let vars = serde_json::json!({ "slug": gt });
            match graphql_exec::<Value>(q, Some(vars)).await {
                Ok(v) => {
                    let list = v
                        .get("compatibleBots")
                        .and_then(|x| x.as_array())
                        .cloned()
                        .unwrap_or_default();
                    bots.set(list);
                }
                Err(e) => push_toast(toast.show, e, ToastKind::Error),
            }
            loading.set(false);
        });
    });

    let needle = filter().to_lowercase();
    let filtered: Vec<Value> = bots()
        .into_iter()
        .filter(|b| {
            if needle.is_empty() {
                return true;
            }
            let name = b.get("displayName").and_then(|x| x.as_str()).unwrap_or("");
            let slug = b.get("slug").and_then(|x| x.as_str()).unwrap_or("");
            name.to_lowercase().contains(&needle) || slug.to_lowercase().contains(&needle)
        })
        .collect();

    let on_close_back = on_close;

    rsx! {
        div { class: "lobby-game-modal-layer",
            button {
                class: "lobby-game-modal-backdrop",
                onclick: move |_| on_close_back.call(()),
            }
            div { class: "lobby-game-modal panel-card max-w-lg w-full",
                div { class: "flex items-start justify-between gap-3 mb-4",
                    div {
                        h2 { class: "font-manrope font-semibold text-on-surface", "Add a bot" }
                        p { class: "text-body-sm text-on-surface-variant mt-1",
                            "Seat {seat_index + 1} — playing as {player_identity}"
                        }
                    }
                    button {
                        class: "btn-ghost p-2",
                        onclick: move |_| on_close.call(()),
                        Icon { name: "close", filled: false }
                    }
                }

                SearchInput {
                    placeholder: "Search published bots…",
                    value: filter(),
                    oninput: EventHandler::new(move |v| filter.set(v)),
                    width_class: "w-full",
                }

                div { class: "mt-3 max-h-64 overflow-y-auto space-y-2",
                    if loading() {
                        p { class: "text-body-sm text-on-surface-variant p-4 text-center", "Loading bots…" }
                    } else if filtered.is_empty() {
                        EmptyState {
                            icon: "smart_toy",
                            title: "No compatible bots".to_string(),
                            description: "Publish a bot for this game, or run a dev-local bot with gamedev bot-run --lobby <id>.".to_string(),
                            cta_label: None,
                            on_cta: None,
                        }
                    } else {
                        for bot in filtered {
                            {
                                let bid = bot.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let name = bot.get("displayName").and_then(|x| x.as_str()).unwrap_or("Bot").to_string();
                                let version = bot.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let seed = bot.get("avatarSeed").and_then(|x| x.as_str()).unwrap_or(&bid).to_string();
                                let settings_for_assign = bot
                                    .get("settingsJson")
                                    .and_then(|x| x.as_str())
                                    .map(str::to_string);
                                let lid = lobby_id.clone();
                                let toast = toast;
                                let on_detail_updated = on_detail_updated;
                                let on_close = on_close;
                                rsx! {
                                    div { class: "flex items-center gap-3 p-3 rounded-lg border border-outline-variant/30",
                                        Avatar { seed: seed.clone(), size: AvatarSize::Md, image_url: None }
                                        div { class: "flex-1 min-w-0",
                                            p { class: "font-medium text-on-surface truncate", "{name}" }
                                            div { class: "flex gap-2 mt-1",
                                                Chip { label: version.clone(), muted: true }
                                                Chip { label: "PUBLISHED".to_string(), muted: false }
                                            }
                                        }
                                        button {
                                            class: "btn-secondary shrink-0",
                                            onclick: move |_| {
                                                let lid = lid.clone();
                                                let bid = bid.clone();
                                                let settings_for_assign = settings_for_assign.clone();
                                                let name = name.clone();
                                                let toast = toast;
                                                let on_detail_updated = on_detail_updated;
                                                let on_close = on_close;
                                                spawn(async move {
                                                    let q = r#"mutation A($id: ID!, $i: Int!, $b: ID!, $s: String) { assignBotToSeat(lobbyId: $id, seatIndex: $i, botId: $b, settingsJson: $s) { id } }"#;
                                                    let settings_val = settings_for_assign
                                                        .map(Value::String)
                                                        .unwrap_or(Value::Null);
                                                    let vars = serde_json::json!({
                                                        "id": lid,
                                                        "i": seat_index,
                                                        "b": bid,
                                                        "s": settings_val,
                                                    });
                                                    match graphql_exec::<Value>(q, Some(vars)).await {
                                                        Ok(_) => {
                                                            let detail_q = format!(
                                                                "query L($id: ID!) {{ lobby(id: $id) {{ {} }} }}",
                                                                crate::api::graphql::LOBBY_DETAIL_FIELDS
                                                            );
                                                            if let Ok(v) = graphql_exec::<Value>(&detail_q, Some(serde_json::json!({ "id": lid }))).await {
                                                                if let Ok(d) = serde_json::from_value::<LobbyDetail>(v.get("lobby").cloned().unwrap_or(Value::Null)) {
                                                                    on_detail_updated.call(d);
                                                                }
                                                            }
                                                            push_toast(toast.show, format!("{name} assigned"), ToastKind::Success);
                                                            on_close.call(());
                                                        }
                                                        Err(e) => push_toast(toast.show, e, ToastKind::Error),
                                                    }
                                                });
                                            },
                                            "Assign"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div { class: "mt-4 p-3 rounded-lg bg-surface-container-low text-body-sm text-on-surface-variant",
                    p { class: "font-medium text-on-surface mb-1", "Dev-local or external bots" }
                    p { "Run gamedev bot-run --lobby {lobby_id} from your bot project, or connect an external bot with an API key. The host approves incoming seat requests below." }
                }
            }
        }
    }
}
