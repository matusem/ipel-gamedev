use dioxus::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ToastKind {
    Success,
    Error,
    Info,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToastMessage {
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
    let msg = ToastMessage {
        text: text.into(),
        kind,
    };
    let mut list = show();
    list.push(msg);
    show.set(list);
    let mut show = show;
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(4_000).await;
        let mut list = show();
        if !list.is_empty() {
            list.remove(0);
            show.set(list);
        }
    });
}

#[component]
pub fn ToastProvider(children: Element) -> Element {
    let show: Signal<Vec<ToastMessage>> = use_signal(Vec::new);
    use_context_provider(|| ToastContext { show });

    rsx! {
        {children}
        div { class: "fixed bottom-24 left-6 md:left-[17.5rem] z-[60] flex flex-col gap-2 max-w-sm",
            for (i, t) in show().iter().enumerate() {
                {
                    let kind_class = match t.kind {
                        ToastKind::Success => "toast toast-success",
                        ToastKind::Error => "toast toast-error",
                        ToastKind::Info => "toast toast-info",
                    };
                    rsx! {
                        div {
                            key: "{i}",
                            class: "{kind_class}",
                            "{t.text}"
                        }
                    }
                }
            }
        }
    }
}
