use crate::components::ui::Icon;
use dioxus::prelude::*;

pub const SECTION_IDS: &[(&str, &str)] = &[
    ("section-about", "About"),
    ("section-reviews", "Reviews"),
    ("section-discussions", "Discussions"),
    ("section-patch-notes", "Patch notes"),
    ("section-versions", "Versions"),
    ("section-leaderboards", "Leaderboards"),
    ("section-match-history", "Match history"),
];

pub fn scroll_to_section(id: &str) {
    let Some(window) = web_sys::window() else { return };
    let Some(doc) = window.document() else { return };
    let Some(el) = doc.get_element_by_id(id) else { return };
    let _ = el.scroll_into_view();
}

#[component]
pub fn SteamSectionNav(active: usize, on_select: EventHandler<usize>) -> Element {
    rsx! {
        nav {
            role: "tablist",
            class: "steam-section-nav",
            for (i, (id, label)) in SECTION_IDS.iter().enumerate() {
                button {
                    role: "tab",
                    aria_selected: "{i == active}",
                    class: if i == active { "tab-btn tab-btn-active" } else { "tab-btn" },
                    onclick: move |_| {
                        on_select.call(i);
                        scroll_to_section(id);
                    },
                    "{label}"
                }
            }
        }
    }
}

#[component]
pub fn SteamSection(
    id: &'static str,
    title: &'static str,
    expanded: bool,
    on_toggle: EventHandler<()>,
    meta: Element,
    children: Element,
) -> Element {
    let toggle_icon = if expanded { "expand_less" } else { "expand_more" };
    rsx! {
        section {
            id: "{id}",
            class: "steam-section scroll-mt-32",
            div { class: "steam-section-header",
                div { class: "min-w-0",
                    h2 { class: "font-manrope text-h2 text-on-surface", "{title}" }
                    {meta}
                }
                button {
                    class: "steam-section-toggle",
                    aria_expanded: "{expanded}",
                    onclick: move |_| on_toggle.call(()),
                    Icon { name: toggle_icon, filled: false }
                    if expanded { "Collapse" } else { "Expand" }
                }
            }
            if expanded {
                div { class: "steam-section-body", {children} }
            }
        }
    }
}
