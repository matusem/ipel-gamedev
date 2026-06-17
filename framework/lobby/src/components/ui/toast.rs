use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::AppShellContext;

static TOAST_ID: AtomicU64 = AtomicU64::new(0);

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
}

#[derive(Clone, Copy)]
pub struct ToastContext {
    pub show: Signal<Vec<ToastMessage>>,
}

pub fn use_toast() -> ToastContext {
    use_context::<ToastContext>()
}

pub fn push_toast(mut show: Signal<Vec<ToastMessage>>, text: impl Into<String>, kind: ToastKind) {
    let id = TOAST_ID.fetch_add(1, Ordering::Relaxed);
    let msg = ToastMessage {
        id,
        text: text.into(),
        kind,
    };
    show.write().push(msg);
    let mut show = show;
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(4_000).await;
        show.write().retain(|t| t.id != id);
    });
}

#[component]
pub fn ToastProvider(children: Element) -> Element {
    let show: Signal<Vec<ToastMessage>> = use_signal(Vec::new);
    use_context_provider(|| ToastContext { show });
    let shell = use_context::<AppShellContext>();
    let in_game = (shell.playing)().is_some();

    rsx! {
        {children}
        if !in_game {
            div { class: "fixed bottom-24 left-6 md:left-[17.5rem] z-[60] flex flex-col gap-2 max-w-sm",
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
