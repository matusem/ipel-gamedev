use dioxus::prelude::*;

#[component]
pub fn Chip(label: String, muted: bool) -> Element {
    let class = if muted { "chip chip-muted" } else { "chip" };
    rsx! {
        span { class: "{class}", "{label}" }
    }
}
