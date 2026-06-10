use dioxus::prelude::*;

#[component]
pub fn SegmentedControl(
    options: Vec<&'static str>,
    active: usize,
    on_select: EventHandler<usize>,
) -> Element {
    rsx! {
        div { class: "inline-flex rounded-lg border border-outline-variant/40 bg-surface-container-low p-1 gap-1",
            for (i, opt) in options.iter().enumerate() {
                button {
                    class: if i == active {
                        "segment-pill segment-pill-active"
                    } else {
                        "segment-pill"
                    },
                    onclick: move |_| on_select.call(i),
                    "{opt}"
                }
            }
        }
    }
}
