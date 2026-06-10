use dioxus::prelude::*;

#[component]
pub fn Skeleton(class: Option<String>) -> Element {
    let extra = class.unwrap_or_default();
    rsx! {
        div { class: "skeleton {extra}" }
    }
}

#[component]
pub fn SkeletonCard() -> Element {
    rsx! {
        div { class: "trending-card p-0 overflow-hidden",
            Skeleton { class: Some("aspect-video rounded-none".into()) }
            div { class: "p-4 space-y-3",
                Skeleton { class: Some("h-5 w-2/3".into()) }
                Skeleton { class: Some("h-4 w-1/2".into()) }
            }
        }
    }
}

#[component]
pub fn SkeletonHero() -> Element {
    rsx! {
        div { class: "page-hero min-h-[320px] p-8 flex flex-col justify-end gap-4",
            Skeleton { class: Some("h-6 w-32 rounded-full".into()) }
            Skeleton { class: Some("h-12 w-2/3 max-w-lg".into()) }
            Skeleton { class: Some("h-5 w-full max-w-md".into()) }
            Skeleton { class: Some("h-10 w-40 rounded-xl".into()) }
        }
    }
}

#[component]
pub fn SkeletonTableRows(count: usize) -> Element {
    rsx! {
        for _ in 0..count {
            div { class: "flex gap-4 px-4 py-3 border-b border-outline-variant/20",
                Skeleton { class: Some("h-8 w-8 rounded-lg shrink-0".into()) }
                Skeleton { class: Some("h-4 flex-1".into()) }
                Skeleton { class: Some("h-4 w-16".into()) }
                Skeleton { class: Some("h-4 w-20".into()) }
            }
        }
    }
}
