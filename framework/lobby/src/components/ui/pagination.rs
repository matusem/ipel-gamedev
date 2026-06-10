use dioxus::prelude::*;
use super::Icon;

#[component]
pub fn Pagination(
    showing: usize,
    total: usize,
    page: usize,
    page_size: usize,
    on_page: EventHandler<usize>,
) -> Element {
    let total_pages = if page_size == 0 {
        1
    } else {
        (total + page_size - 1) / page_size
    };
    let can_prev = page > 0;
    let can_next = page + 1 < total_pages;

    rsx! {
        div { class: "flex items-center justify-between px-4 py-3 border-t border-outline-variant/30 text-body-sm",
            p { class: "text-on-surface-variant",
                "Showing {showing} of {total}"
            }
            div { class: "flex items-center gap-2",
                button {
                    class: "btn-ghost py-1 px-2",
                    disabled: !can_prev,
                    onclick: move |_| {
                        if can_prev {
                            on_page.call(page - 1);
                        }
                    },
                    Icon { name: "chevron_left", filled: false }
                }
                span { class: "font-mono-code text-on-surface-variant",
                    "{page + 1} / {total_pages.max(1)}"
                }
                button {
                    class: "btn-ghost py-1 px-2",
                    disabled: !can_next,
                    onclick: move |_| {
                        if can_next {
                            on_page.call(page + 1);
                        }
                    },
                    Icon { name: "chevron_right", filled: false }
                }
            }
        }
    }
}
