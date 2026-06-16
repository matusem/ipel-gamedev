//! CLI browser login handoff — mint publish token and redirect to local loopback.

use crate::api::{graphql_post, stored_session_token};
use crate::pages::AuthGate;
use dioxus::prelude::*;
use serde::Deserialize;
use web_sys::window;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CliAuthPhase {
    Loading,
    Auth,
    Minting,
    Done,
    Error,
}

pub fn is_cli_auth_location() -> bool {
    window()
        .and_then(|w| w.location().pathname().ok())
        .is_some_and(|p| p.ends_with("/cli-auth"))
}

pub fn read_cli_auth_params_from_location() -> (String, String) {
    let search = window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    (
        query_param(&search, "port").unwrap_or_default(),
        query_param(&search, "state").unwrap_or_default(),
    )
}

fn query_param(search: &str, key: &str) -> Option<String> {
    let qs = search.trim_start_matches('?');
    for pair in qs.split('&') {
        let mut parts = pair.splitn(2, '=');
        let k = parts.next()?;
        if k == key {
            let v = parts.next().unwrap_or("");
            return urlencoding::decode(v)
                .ok()
                .map(|s| s.into_owned())
                .or_else(|| Some(v.to_string()));
        }
    }
    None
}

fn resolve_callback_params(route_port: &str, route_state: &str) -> (String, String) {
    let mut port = route_port.trim().to_string();
    let mut state = route_state.trim().to_string();
    if port.is_empty() || state.is_empty() {
        let (loc_port, loc_state) = read_cli_auth_params_from_location();
        if port.is_empty() {
            port = loc_port;
        }
        if state.is_empty() {
            state = loc_state;
        }
    }
    (port, state)
}

#[derive(Clone)]
struct CliAuthInit {
    port: String,
    state: String,
    phase: CliAuthPhase,
    error: Option<String>,
}

fn init_cli_auth(route_port: String, route_state: String) -> CliAuthInit {
    let (port, state) = resolve_callback_params(&route_port, &route_state);
    if port.is_empty() || state.is_empty() {
        return CliAuthInit {
            port,
            state,
            phase: CliAuthPhase::Error,
            error: Some(
                "Missing port or state in the URL. Close this tab and run `gamedev login` from your terminal."
                    .into(),
            ),
        };
    }
    let phase = if stored_session_token().is_some() {
        CliAuthPhase::Minting
    } else {
        CliAuthPhase::Auth
    };
    CliAuthInit {
        port,
        state,
        phase,
        error: None,
    }
}

async fn mint_and_redirect(
    port_val: String,
    state_val: String,
    mut phase: Signal<CliAuthPhase>,
    mut manual_url: Signal<Option<String>>,
    mut error: Signal<Option<String>>,
) {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Tok {
        token: String,
        expires_at: i64,
        user_id: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Wrap {
        create_publish_token: Tok,
    }

    let host = window()
        .and_then(|w| w.location().hostname().ok())
        .unwrap_or_else(|| "cli".into());
    let label = format!("CLI {host}");
    let q = format!(
        "mutation {{ createPublishToken(label: {:?}) {{ token expiresAt userId }} }}",
        label
    );
    match graphql_post::<Wrap>(&q).await {
        Ok(w) => {
            let token = w.create_publish_token.token;
            let expires = w.create_publish_token.expires_at;
            let user_id = w.create_publish_token.user_id;
            let callback = format!(
                "http://127.0.0.1:{port_val}/callback?token={}&state={}&expires={expires}&user={}",
                urlencoding::encode(&token),
                urlencoding::encode(&state_val),
                urlencoding::encode(&user_id),
            );
            manual_url.set(Some(callback.clone()));
            if let Some(win) = window() {
                let _ = win.location().set_href(&callback);
            }
            phase.set(CliAuthPhase::Done);
        }
        Err(e) => {
            error.set(Some(e));
            phase.set(CliAuthPhase::Error);
        }
    }
}

#[component]
pub fn CliAuthPage(port: String, state: String) -> Element {
    let init = use_hook(move || init_cli_auth(port, state));
    let mut phase = use_signal(move || init.phase);
    let mut manual_url = use_signal(|| None::<String>);
    let mut error = use_signal(|| init.error.clone());
    let port_for_mint = init.port.clone();
    let state_for_mint = init.state.clone();

    // Run once on mount when the user is already signed in to the lobby.
    use_hook({
        let port_for_mint = port_for_mint.clone();
        let state_for_mint = state_for_mint.clone();
        move || {
            if init.phase != CliAuthPhase::Minting {
                return;
            }
            let port = port_for_mint.clone();
            let state = state_for_mint.clone();
            spawn(async move {
                mint_and_redirect(port, state, phase, manual_url, error).await;
            });
        }
    });

    let on_authed = {
        let port = port_for_mint.clone();
        let state = state_for_mint.clone();
        move |_| {
            phase.set(CliAuthPhase::Minting);
            let port = port.clone();
            let state = state.clone();
            spawn(async move {
                mint_and_redirect(port, state, phase, manual_url, error).await;
            });
        }
    };

    match phase() {
        CliAuthPhase::Loading => rsx! {
            div { class: "section-card p-8 text-center max-w-lg mx-auto mt-12",
                p { class: "text-body text-outline", "Preparing CLI login…" }
            }
        },
        CliAuthPhase::Auth => rsx! {
            AuthGate { on_ready: on_authed }
        },
        CliAuthPhase::Minting => rsx! {
            div { class: "section-card p-8 text-center space-y-3 max-w-lg mx-auto mt-12",
                p { class: "text-body text-on-surface", "Creating CLI token…" }
                p { class: "text-body-sm text-outline", "You will be redirected to your terminal momentarily." }
            }
        },
        CliAuthPhase::Done => rsx! {
            div { class: "section-card p-8 space-y-4 max-w-lg mx-auto mt-12",
                h1 { class: "text-title-lg text-on-surface", "CLI login complete" }
                p { class: "text-body text-outline",
                    "You can return to your terminal. If the redirect was blocked, copy this URL into your browser address bar:"
                }
                if let Some(url) = manual_url() {
                    code { class: "block text-body-sm break-all bg-surface-container p-3 rounded-lg",
                        "{url}"
                    }
                }
            }
        },
        CliAuthPhase::Error => rsx! {
            div { class: "section-card p-8 space-y-3 max-w-lg mx-auto mt-12",
                h1 { class: "text-title-lg text-error", "CLI login failed" }
                p { class: "text-body text-outline",
                    "{error().unwrap_or_else(|| \"Unknown error\".into())}"
                }
                p { class: "text-body-sm text-outline",
                    "Close this tab and run gamedev login again from your terminal."
                }
            }
        },
    }
}
