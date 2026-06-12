mod activity_feed;
mod avatar;
mod chip;
mod empty_state;
mod forms;
mod icons;
mod json_console;
mod kpi_card;
mod page_header;
mod pagination;
mod segmented_control;
mod skeleton;
mod slide_over;
mod spider_chart;
mod toast;

use dioxus::prelude::*;

pub use activity_feed::ActivityFeed;
pub use avatar::{avatar_color, avatar_initial, Avatar, AvatarSize};
pub use chip::Chip;
pub use empty_state::EmptyState;
pub use forms::{Callout, CalloutVariant, FieldLabel, SearchInput};
pub use icons::Icon;
pub use json_console::JsonConsole;
pub use kpi_card::KpiCard;
pub use page_header::PageHeader;
pub use pagination::Pagination;
pub use segmented_control::SegmentedControl;
pub use skeleton::{Skeleton, SkeletonCard, SkeletonHero, SkeletonTableRows};
pub use slide_over::SlideOver;
pub use spider_chart::{SpiderAxis, SpiderChart};
pub use toast::{push_toast, use_toast, ToastContext, ToastKind, ToastProvider};

#[derive(Clone, Copy, PartialEq)]
pub enum StatusVariant {
    Online,
    Waiting,
    InGame,
    Full,
    Failed,
}

impl StatusVariant {
    pub fn dot_class(self) -> &'static str {
        match self {
            StatusVariant::Online => "status-dot-online",
            StatusVariant::Waiting => "status-dot-away",
            StatusVariant::InGame => "status-dot-online",
            StatusVariant::Full => "status-dot-full",
            StatusVariant::Failed => "status-dot-full",
        }
    }

    pub fn ping(self) -> bool {
        matches!(self, StatusVariant::InGame)
    }
}

pub fn status_variant_from_lobby(status: &str, seats_filled: i32, seats_total: i32) -> StatusVariant {
    let s = status.to_lowercase();
    if s.contains("fail") || s.contains("error") {
        StatusVariant::Failed
    } else if s.contains("full") || (seats_total > 0 && seats_filled >= seats_total) {
        StatusVariant::Full
    } else if s.contains("in_game") || s.contains("ingame") || s.contains("playing") {
        StatusVariant::InGame
    } else if s.contains("waiting") || s.contains("open") || s.contains("config") {
        StatusVariant::Waiting
    } else {
        StatusVariant::Online
    }
}

#[component]
pub fn ErrorBanner(message: String) -> Element {
    let text = crate::api::graphql_error_message(&message);
    rsx! {
        div { class: "rounded-xl border border-error/50 bg-error-container/30 px-4 py-3",
            p { class: "text-body-sm font-medium text-error break-words", "{text}" }
        }
    }
}

#[component]
pub fn LoadingState(title: String, subtitle: String) -> Element {
    rsx! {
        div { class: "flex flex-col items-center justify-center py-16 gap-4",
            div { class: "h-8 w-8 rounded-full border-2 border-primary-container/30 border-t-primary-container animate-spin" }
            p { class: "text-body-sm font-medium text-on-surface-variant", "{title}" }
            p { class: "text-body-sm text-outline", "{subtitle}" }
        }
    }
}

#[component]
pub fn SectionHeader(title: String, subtitle: Option<String>) -> Element {
    rsx! {
        div { class: "mb-4",
            h2 { class: "font-manrope text-h2 text-on-surface", "{title}" }
            if let Some(s) = subtitle {
                p { class: "text-body-sm text-on-surface-variant mt-1", "{s}" }
            }
        }
    }
}

#[component]
pub fn StatusBadge(label: String, variant: StatusVariant) -> Element {
    let dot = variant.dot_class();
    let ping = variant.ping();
    rsx! {
        span { class: "inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-surface-container-high text-label-caps font-label-caps text-on-surface-variant uppercase",
            span { class: "relative flex h-2 w-2 shrink-0",
                if ping {
                    span { class: "animate-ping absolute inline-flex h-full w-full rounded-full bg-tertiary opacity-60" }
                }
                span { class: "{dot}" }
            }
            "{label}"
        }
    }
}

#[component]
pub fn StatusBadgeDot(dot_class: &'static str, label: String) -> Element {
    rsx! {
        span { class: "inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-surface-container-high text-label-caps font-label-caps text-on-surface-variant uppercase",
            span { class: "{dot_class}" }
            "{label}"
        }
    }
}

#[component]
pub fn PrimaryButton(
    label: String,
    disabled: bool,
    onclick: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            class: "btn-primary active:scale-95 transition-transform",
            disabled,
            onclick: move |_| onclick.call(()),
            "{label}"
        }
    }
}

#[component]
pub fn GhostButton(label: String, onclick: EventHandler<()>) -> Element {
    rsx! {
        button {
            class: "btn-ghost",
            onclick: move |_| onclick.call(()),
            "{label}"
        }
    }
}

#[component]
pub fn SecondaryButton(
    label: String,
    disabled: bool,
    onclick: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            class: "btn-secondary active:scale-95 transition-transform",
            disabled,
            onclick: move |_| onclick.call(()),
            "{label}"
        }
    }
}

#[component]
pub fn LinkAction(label: String, icon: Option<&'static str>, onclick: EventHandler<()>) -> Element {
    rsx! {
        button {
            class: "link-action",
            onclick: move |_| onclick.call(()),
            "{label}"
            if let Some(name) = icon {
                Icon { name, filled: false }
            }
        }
    }
}

#[component]
pub fn TabBar(tabs: Vec<&'static str>, active: usize, on_select: EventHandler<usize>) -> Element {
    rsx! {
        div {
            role: "tablist",
            class: "flex gap-1 border-b border-outline-variant/40 mb-6 overflow-x-auto sticky top-16 z-30 bg-background/95 backdrop-blur-sm py-1",
            for (i, tab) in tabs.iter().enumerate() {
                button {
                    role: "tab",
                    aria_selected: "{i == active}",
                    class: if i == active { "tab-btn tab-btn-active" } else { "tab-btn" },
                    onclick: move |_| on_select.call(i),
                    "{tab}"
                }
            }
        }
    }
}

#[component]
pub fn QuickLinkCard(
    title: String,
    subtitle: String,
    icon: &'static str,
    accent: &'static str,
    onclick: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            class: "w-full text-left section-card flex items-center justify-between group hover:border-primary-container/50 transition-colors",
            onclick: move |_| onclick.call(()),
            div { class: "space-y-2",
                h3 { class: "card-title text-lg", "{title}" }
                p { class: "text-body-sm text-on-surface-variant", "{subtitle}" }
                span { class: "mt-3 link-action {accent}",
                    "Open"
                    Icon { name: "open_in_new", filled: false }
                }
            }
            div { class: "p-4 rounded-full bg-primary-container/10 group-hover:bg-primary-container/20 transition-colors",
                Icon { name: icon, filled: false }
            }
        }
    }
}
