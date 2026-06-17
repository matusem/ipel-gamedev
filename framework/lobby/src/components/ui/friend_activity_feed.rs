use crate::models::{format_relative_time, FriendActivityGql};
use crate::components::ui::{Avatar, AvatarSize};
use dioxus::prelude::*;

fn activity_text(kind: &str, target: &str) -> String {
    match kind {
        "lobby_created" => format!("created a lobby ({target})"),
        "game_won" => format!("won a game ({target})"),
        "game_finished" => format!("finished a game ({target})"),
        "friend_added" => format!("became friends with {target}"),
        other => format!("{other} {target}"),
    }
}

#[component]
pub fn FriendActivityFeed(events: Vec<FriendActivityGql>) -> Element {
    rsx! {
        div { class: "space-y-3",
            if events.is_empty() {
                p { class: "text-body-sm text-outline", "No friend activity yet." }
            }
            for ev in events {
                div { class: "flex items-start gap-3",
                    Avatar {
                        seed: ev.actor_id.clone(),
                        size: AvatarSize::Sm,
                        image_url: ev.actor_avatar_url.clone(),
                    }
                    div { class: "min-w-0 flex-1",
                        p { class: "text-body-sm text-on-surface leading-snug",
                            span { class: "font-medium", "{ev.actor_name}" }
                            " {activity_text(&ev.kind, &ev.target)}"
                        }
                        p { class: "text-label-caps font-label-caps text-outline mt-0.5",
                            "{format_relative_time(ev.timestamp)}"
                        }
                    }
                }
            }
        }
    }
}
