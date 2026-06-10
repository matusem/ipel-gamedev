use crate::models::{format_relative_time, ActivityEventGql};
use dioxus::prelude::*;
use super::{Avatar, AvatarSize};

#[component]
pub fn ActivityFeed(events: Vec<ActivityEventGql>) -> Element {
    rsx! {
        div { class: "space-y-3",
            for ev in events {
                div { class: "flex items-start gap-3",
                    Avatar { seed: ev.actor.clone(), size: AvatarSize::Sm, image_url: None }
                    div { class: "min-w-0 flex-1",
                        p { class: "text-body-sm text-on-surface leading-snug",
                            span { class: "font-medium", "{ev.actor}" }
                            " {ev.action} "
                            span { class: "text-primary", "{ev.target}" }
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
