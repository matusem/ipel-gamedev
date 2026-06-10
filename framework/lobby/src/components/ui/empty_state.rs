use dioxus::prelude::*;
use super::Icon;

#[component]
pub fn EmptyState(
    icon: &'static str,
    title: String,
    description: String,
    cta_label: Option<String>,
    on_cta: Option<EventHandler<()>>,
) -> Element {
    rsx! {
        div { class: "empty-state",
            div { class: "text-outline mb-4",
                Icon { name: icon, filled: false }
            }
            h3 { class: "font-manrope text-lg font-semibold text-on-surface", "{title}" }
            p { class: "text-body-sm text-on-surface-variant mt-2 max-w-sm", "{description}" }
            if let Some(label) = cta_label {
                if let Some(handler) = on_cta {
                    button {
                        class: "btn-primary mt-6",
                        onclick: move |_| handler.call(()),
                        "{label}"
                    }
                }
            }
        }
    }
}
