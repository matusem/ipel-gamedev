use crate::api::*;
use crate::components::ui::{Callout, CalloutVariant, ErrorBanner, Icon};
use crate::models::{LoginData, RegisterUserData, SignUpData};
use crate::stub::demo_mode;
use dioxus::prelude::*;

#[component]
pub fn AuthGate(on_ready: EventHandler<()>) -> Element {
    let mut guest_name = use_signal(|| "Guest".to_string());
    let mut signup_name = use_signal(|| String::new());
    let mut signup_pass = use_signal(|| String::new());
    let mut login_name = use_signal(|| String::new());
    let mut login_pass = use_signal(|| String::new());
    let mut err = use_signal(|| None::<String>);

    rsx! {
        div { class: "max-w-lg mx-auto px-4 py-12 sm:py-20",
            div { class: "text-center mb-10",
                h1 { class: "font-manrope text-h1 text-on-surface", "UPJŠ GDD Platform" }
                p { class: "mt-2 text-body-sm text-on-surface-variant",
                    "Guest, sign up, or log in to join lobbies."
                }
            }
            if let Some(e) = err() {
                ErrorBanner { message: e }
            }
            Callout {
                variant: CalloutVariant::Secondary,
                div { class: "flex flex-col sm:flex-row sm:items-center gap-4 justify-between",
                    div {
                        p { class: "card-title", "Explore without a server" }
                        p { class: "text-body-sm text-on-surface-variant mt-1",
                            "Load rich synthetic data — busy lobbies, reviews, leaderboards, patch notes, and activity."
                        }
                    }
                    button {
                        class: "btn-secondary shrink-0",
                        onclick: move |_| demo_mode::enter_demo_mode(),
                        Icon { name: "auto_awesome", filled: false }
                        "Try demo"
                    }
                }
            }
            div { class: "page-stack",
                div { class: "section-card",
                    div { class: "flex items-center gap-2 mb-4",
                        span { class: "flex h-8 w-8 items-center justify-center rounded-lg bg-primary-container text-sm text-on-primary-container",
                            Icon { name: "person", filled: false }
                        }
                        h2 { class: "card-title", "Continue as guest" }
                    }
                    input {
                        class: "input-field mb-3",
                        placeholder: "Display name",
                        value: "{guest_name}",
                        oninput: move |e| guest_name.set(e.value()),
                    }
                    button {
                        class: "btn-primary w-full",
                        onclick: move |_| {
                            let n = guest_name();
                            spawn(async move {
                                let q = "mutation G($n: String!) { registerUser(displayName: $n) { sessionToken user { id } } }";
                                let vars = serde_json::json!({ "n": n });
                                match graphql_exec_anonymous::<RegisterUserData>(q, Some(vars)).await {
                                    Ok(data) => {
                                        let auth = data.register_user;
                                        store_auth_session(&auth.session_token, &auth.user.id);
                                        on_ready.call(());
                                    }
                                    Err(e) => err.set(Some(e)),
                                }
                            });
                        },
                        "Continue"
                    }
                }
                div { class: "section-card",
                    div { class: "flex items-center gap-2 mb-4",
                        span { class: "flex h-8 w-8 items-center justify-center rounded-lg bg-tertiary-container text-sm font-bold text-on-tertiary-container",
                            Icon { name: "person_add", filled: false }
                        }
                        h2 { class: "card-title", "Sign up" }
                    }
                    input {
                        class: "input-field mb-2",
                        placeholder: "Display name",
                        value: "{signup_name}",
                        oninput: move |e| signup_name.set(e.value()),
                    }
                    input {
                        class: "input-field mb-3",
                        r#type: "password",
                        placeholder: "Password (min 8 chars)",
                        value: "{signup_pass}",
                        oninput: move |e| signup_pass.set(e.value()),
                    }
                    button {
                        class: "btn-primary w-full",
                        onclick: move |_| {
                            let n = signup_name();
                            let p = signup_pass();
                            spawn(async move {
                                let q = "mutation SignUp($n: String!, $p: String!) { signUp(displayName: $n, password: $p) { sessionToken user { id } } }";
                                let vars = serde_json::json!({ "n": n, "p": p });
                                match graphql_exec_anonymous::<SignUpData>(q, Some(vars)).await {
                                    Ok(w) => {
                                        store_auth_session(&w.sign_up.session_token, &w.sign_up.user.id);
                                        on_ready.call(());
                                    }
                                    Err(e) => err.set(Some(e)),
                                }
                            });
                        },
                        "Create account"
                    }
                }
                div { class: "section-card",
                    div { class: "flex items-center gap-2 mb-4",
                        span { class: "flex h-8 w-8 items-center justify-center rounded-lg bg-surface-container-high text-on-surface-variant",
                            Icon { name: "login", filled: false }
                        }
                        h2 { class: "card-title", "Log in" }
                    }
                    input {
                        class: "input-field mb-2",
                        placeholder: "Display name",
                        value: "{login_name}",
                        oninput: move |e| login_name.set(e.value()),
                    }
                    input {
                        class: "input-field mb-3",
                        r#type: "password",
                        placeholder: "Password",
                        value: "{login_pass}",
                        oninput: move |e| login_pass.set(e.value()),
                    }
                    button {
                        class: "btn-secondary w-full",
                        onclick: move |_| {
                            let n = login_name();
                            let p = login_pass();
                            spawn(async move {
                                let q = "mutation Login($n: String!, $p: String!) { loginWithPassword(displayName: $n, password: $p) { sessionToken user { id } } }";
                                let vars = serde_json::json!({ "n": n, "p": p });
                                match graphql_exec_anonymous::<LoginData>(q, Some(vars)).await {
                                    Ok(l) => {
                                        store_auth_session(
                                            &l.login_with_password.session_token,
                                            &l.login_with_password.user.id,
                                        );
                                        let _ = web_sys::window().unwrap().location().reload();
                                    }
                                    Err(e) => err.set(Some(e)),
                                }
                            });
                        },
                        "Log in"
                    }
                }
            }
        }
    }
}
