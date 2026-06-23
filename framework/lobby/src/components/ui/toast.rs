use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::AppShellContext;

static TOAST_ID: AtomicU64 = AtomicU64::new(0);
const MAX_VISIBLE_TOASTS: usize = 4;
const TOAST_TTL_MS: u32 = 4_000;
const PRUNE_INTERVAL_MS: u32 = 500;

#[derive(Clone, Debug, PartialEq)]
pub enum ToastKind {
    Success,
    Error,
    Info,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToastMessage {
    pub id: u64,
    pub text: String,
    pub kind: ToastKind,
    /// Milliseconds since epoch; pruned by `ToastProvider` (not per-caller spawns).
    pub expires_at_ms: f64,
}

#[derive(Clone, Copy)]
pub struct ToastContext {
    pub show: Signal<Vec<ToastMessage>>,
}

pub fn use_toast() -> ToastContext {
    use_context::<ToastContext>()
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

pub fn push_toast(mut show: Signal<Vec<ToastMessage>>, text: impl Into<String>, kind: ToastKind) {
    let id = TOAST_ID.fetch_add(1, Ordering::Relaxed);
    let msg = ToastMessage {
        id,
        text: text.into(),
        kind,
        expires_at_ms: now_ms() + f64::from(TOAST_TTL_MS),
    };
    let mut list = show.write();
    list.push(msg);
    if list.len() > MAX_VISIBLE_TOASTS {
        let drop = list.len() - MAX_VISIBLE_TOASTS;
        list.drain(0..drop);
    }
}

#[component]
pub fn ToastProvider(children: Element) -> Element {
    let mut show: Signal<Vec<ToastMessage>> = use_signal(Vec::new);
    use_context_provider(|| ToastContext { show });
    let shell = use_context::<AppShellContext>();
    let in_game = (shell.playing)().is_some();

    use_hook(move || {
        spawn(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(PRUNE_INTERVAL_MS).await;
                let now = now_ms();
                show.write().retain(|t| t.expires_at_ms > now);
            }
        });
    });

    rsx! {
        {children}
        if !in_game {
            div { class: "fixed top-20 left-6 md:left-[17.5rem] z-[60] flex flex-col gap-2 max-w-sm",
                for t in show().iter() {
                    {
                        let kind_class = match t.kind {
                            ToastKind::Success => "toast toast-success",
                            ToastKind::Error => "toast toast-error",
                            ToastKind::Info => "toast toast-info",
                        };
                        rsx! {
                            div {
                                key: "{t.id}",
                                class: "{kind_class}",
                                "{t.text}"
                            }
                        }
                    }
                }
            }
        }
    }
}
