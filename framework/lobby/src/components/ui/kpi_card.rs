use dioxus::prelude::*;
use super::Icon;

#[component]
pub fn KpiCard(
    label: String,
    value: String,
    icon: Option<&'static str>,
    trend: Option<String>,
    trend_up: bool,
) -> Element {
    rsx! {
        div { class: "kpi-card",
            div { class: "flex items-center justify-between mb-2",
                span { class: "text-label-caps font-label-caps text-outline uppercase", "{label}" }
                if let Some(ic) = icon {
                    Icon { name: ic, filled: false }
                }
            }
            p { class: "font-manrope text-2xl font-semibold text-on-surface", "{value}" }
            if let Some(t) = trend {
                span {
                    class: if trend_up { "text-xs font-mono-code text-tertiary mt-1 inline-flex items-center gap-1" } else { "text-xs font-mono-code text-error mt-1 inline-flex items-center gap-1" },
                    if trend_up {
                        Icon { name: "trending_up", filled: false }
                    } else {
                        Icon { name: "trending_down", filled: false }
                    }
                    "{t}"
                }
            }
        }
    }
}
