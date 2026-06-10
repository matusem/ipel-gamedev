use crate::models::GameScreenshot;
use dioxus::prelude::*;

#[component]
pub fn MediaGallery(screenshots: Vec<GameScreenshot>) -> Element {
    let mut active = use_signal(|| 0usize);
    let len = screenshots.len().max(1);
    let idx = active().min(len.saturating_sub(1));
    let shot = screenshots.get(idx).cloned();

    rsx! {
        div { class: "space-y-3",
            div { class: "relative rounded-2xl overflow-hidden aspect-video border border-outline-variant/40 bg-surface-container-low",
                if let Some(ref s) = shot {
                    if let Some(ref url) = s.image_url {
                        img {
                            class: "absolute inset-0 w-full h-full object-cover",
                            src: "{url}",
                            alt: "{s.caption}",
                        }
                    } else {
                        div { class: "absolute inset-0 bg-gradient-to-br {s.gradient}" }
                    }
                    div { class: "absolute inset-0 bg-gradient-to-t from-background/80 via-transparent to-transparent pointer-events-none" }
                    p { class: "absolute bottom-3 left-4 text-body-sm text-on-surface font-medium z-10 drop-shadow-sm", "{s.caption}" }
                } else {
                    div { class: "absolute inset-0 bg-gradient-to-br from-surface-container-high to-background" }
                }
                if len > 1 {
                    button {
                        class: "absolute left-2 top-1/2 -translate-y-1/2 z-10 h-9 w-9 rounded-full bg-background/70 border border-outline-variant/50 flex items-center justify-center hover:bg-background",
                        onclick: move |_| active.set((idx + len - 1) % len),
                        "‹"
                    }
                    button {
                        class: "absolute right-2 top-1/2 -translate-y-1/2 z-10 h-9 w-9 rounded-full bg-background/70 border border-outline-variant/50 flex items-center justify-center hover:bg-background",
                        onclick: move |_| active.set((idx + 1) % len),
                        "›"
                    }
                }
            }
            if screenshots.len() > 1 {
                div { class: "flex gap-2 overflow-x-auto pb-1",
                    for (i, s) in screenshots.iter().enumerate() {
                        button {
                            class: if i == idx { "game-thumb-thumb active shrink-0" } else { "game-thumb-thumb shrink-0" },
                            onclick: move |_| active.set(i),
                            if let Some(ref url) = s.image_url {
                                img {
                                    class: "h-14 w-24 rounded-lg object-cover border border-outline-variant/40",
                                    src: "{url}",
                                    alt: "{s.caption}",
                                }
                            } else {
                                div { class: "h-14 w-24 rounded-lg bg-gradient-to-br {s.gradient} border border-outline-variant/40" }
                            }
                        }
                    }
                }
            }
        }
    }
}
