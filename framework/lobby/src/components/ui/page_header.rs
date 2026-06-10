use dioxus::prelude::*;

#[component]
pub fn PageHeader(
    title: String,
    subtitle: Option<String>,
    badge: Option<String>,
    children: Option<Element>,
) -> Element {
    rsx! {
        div { class: "flex flex-col lg:flex-row lg:items-end lg:justify-between gap-4 mb-8",
            div {
                div { class: "flex items-center gap-3 flex-wrap",
                    h1 { class: "font-manrope text-h1 text-on-surface", "{title}" }
                    if let Some(b) = badge {
                        span { class: "inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-tertiary-container/20 text-tertiary text-label-caps font-label-caps uppercase border border-tertiary-container/30",
                            span { class: "status-dot-online" }
                            "{b}"
                        }
                    }
                }
                if let Some(s) = subtitle {
                    p { class: "mt-2 text-body-sm text-on-surface-variant max-w-2xl", "{s}" }
                }
            }
            if let Some(actions) = children {
                div { class: "flex flex-wrap items-center gap-3 shrink-0", {actions} }
            }
        }
    }
}
