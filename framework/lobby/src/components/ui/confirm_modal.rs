use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
struct ConfirmPending {
    message: String,
    confirm_label: String,
    cancel_label: String,
    destructive: bool,
    result: Rc<RefCell<Option<bool>>>,
}

#[derive(Clone, Copy)]
pub struct ConfirmContext {
    request: Signal<Option<ConfirmPending>>,
}

#[derive(Clone, Copy)]
pub struct ConfirmHandle {
    request: Signal<Option<ConfirmPending>>,
}

pub fn use_confirm() -> ConfirmHandle {
    let ctx = use_context::<ConfirmContext>();
    ConfirmHandle {
        request: ctx.request,
    }
}

#[derive(Clone)]
pub struct ConfirmOptions {
    pub message: String,
    pub confirm_label: String,
    pub cancel_label: String,
    pub destructive: bool,
}

impl ConfirmOptions {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            confirm_label: "Confirm".into(),
            cancel_label: "Cancel".into(),
            destructive: false,
        }
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    pub fn confirm_label(mut self, label: impl Into<String>) -> Self {
        self.confirm_label = label.into();
        self
    }
}

impl ConfirmHandle {
    pub async fn confirm(&self, message: impl Into<String>) -> bool {
        self.confirm_with(ConfirmOptions::new(message)).await
    }

    pub async fn confirm_with(&self, options: ConfirmOptions) -> bool {
        let mut request = self.request;
        let result = Rc::new(RefCell::new(None));
        request.set(Some(ConfirmPending {
            message: options.message,
            confirm_label: options.confirm_label,
            cancel_label: options.cancel_label,
            destructive: options.destructive,
            result: result.clone(),
        }));

        loop {
            if let Some(ok) = *result.borrow() {
                request.set(None);
                return ok;
            }
            if request().is_none() {
                return false;
            }
            TimeoutFuture::new(16).await;
        }
    }
}

fn resolve_confirm(mut request: Signal<Option<ConfirmPending>>, ok: bool) {
    if let Some(req) = request() {
        *req.result.borrow_mut() = Some(ok);
    }
    request.set(None);
}

#[component]
pub fn ConfirmProvider(children: Element) -> Element {
    let request: Signal<Option<ConfirmPending>> = use_signal(|| None);
    use_context_provider(|| ConfirmContext { request });

    rsx! {
        {children}
        ConfirmModal {}
    }
}

#[component]
fn ConfirmModal() -> Element {
    let ctx = use_context::<ConfirmContext>();
    let request = ctx.request;
    let Some(req) = request() else {
        return rsx! {};
    };

    let message = req.message.clone();
    let confirm_label = req.confirm_label.clone();
    let cancel_label = req.cancel_label.clone();
    let destructive = req.destructive;

    rsx! {
        div { class: "confirm-modal-layer",
            div {
                class: "confirm-modal-backdrop",
                onclick: move |_| resolve_confirm(request, false),
            }
            div {
                class: "confirm-modal-card",
                role: "alertdialog",
                aria_modal: "true",
                p { class: "confirm-modal-message", "{message}" }
                div { class: "confirm-modal-actions",
                    button {
                        class: "btn-ghost",
                        onclick: move |_| resolve_confirm(request, false),
                        "{cancel_label}"
                    }
                    button {
                        class: if destructive { "btn-danger" } else { "btn-primary" },
                        onclick: move |_| resolve_confirm(request, true),
                        "{confirm_label}"
                    }
                }
            }
        }
    }
}
