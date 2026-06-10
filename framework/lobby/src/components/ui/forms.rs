use dioxus::prelude::*;

use super::Icon;

#[derive(Clone, Copy, PartialEq)]
pub enum CalloutVariant {
    Secondary,
    Tertiary,
    Neutral,
}

impl CalloutVariant {
    fn class(self) -> &'static str {
        match self {
            CalloutVariant::Secondary => "callout-secondary",
            CalloutVariant::Tertiary => "callout-tertiary",
            CalloutVariant::Neutral => "callout-neutral",
        }
    }
}

#[component]
pub fn Callout(variant: CalloutVariant, children: Element) -> Element {
    let class = variant.class();
    rsx! {
        div { class: "{class}", {children} }
    }
}

#[component]
pub fn FieldLabel(label: String) -> Element {
    rsx! {
        label { class: "field-label", "{label}" }
    }
}

#[component]
pub fn SearchInput(
    placeholder: &'static str,
    value: String,
    oninput: EventHandler<String>,
    #[props(default = "w-40")]
    width_class: &'static str,
) -> Element {
    rsx! {
        div { class: "search-field {width_class}",
            span { class: "inline-flex items-center justify-center text-on-surface-variant shrink-0",
                Icon { name: "search", filled: false }
            }
            input {
                class: "search-input",
                placeholder: "{placeholder}",
                value: "{value}",
                oninput: move |e| oninput.call(e.value()),
            }
        }
    }
}
