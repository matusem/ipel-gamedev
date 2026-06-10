use dioxus::prelude::*;

#[component]
pub fn Icon(name: &'static str, filled: bool) -> Element {
    let fill = if filled { "1" } else { "0" };
    rsx! {
        span {
            class: "material-symbols-outlined select-none leading-none",
            style: "font-variation-settings: 'FILL' {fill}, 'wght' 400, 'GRAD' 0, 'opsz' 20;",
            "{name}"
        }
    }
}
