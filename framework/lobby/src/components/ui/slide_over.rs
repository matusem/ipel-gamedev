use dioxus::prelude::*;
use super::Icon;

#[component]
pub fn SlideOver(
    open: bool,
    title: String,
    on_close: EventHandler<()>,
    children: Element,
    footer: Option<Element>,
) -> Element {
    if !open {
        return rsx! {};
    }
    rsx! {
        div {
            class: "fixed inset-0 z-[70] flex justify-end",
            div {
                class: "absolute inset-0 bg-background/70 backdrop-blur-sm",
                onclick: move |_| on_close.call(()),
            }
            div { class: "relative w-full max-w-lg h-full bg-surface-container border-l border-outline-variant/40 shadow-raised flex flex-col slide-over-panel",
                div { class: "flex items-center justify-between px-6 py-4 border-b border-outline-variant/30",
                    h2 { class: "font-manrope text-lg font-semibold text-on-surface", "{title}" }
                    button {
                        class: "p-2 text-on-surface-variant hover:text-on-surface",
                        onclick: move |_| on_close.call(()),
                        aria_label: "Close",
                        Icon { name: "close", filled: false }
                    }
                }
                div { class: "flex-1 overflow-y-auto p-6", {children} }
                if let Some(f) = footer {
                    div { class: "px-6 py-4 border-t border-outline-variant/30 flex gap-3 justify-end", {f} }
                }
            }
        }
    }
}
