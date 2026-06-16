use crate::api::*;
use crate::components::ui::{ErrorBanner, Icon};
use crate::models::{LoginData, SignUpData};
use dioxus::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OauthAvailableData {
    oauth_available: bool,
}

#[component]
pub fn AuthGate(on_ready: EventHandler<()>) -> Element {
    let mut signup_name = use_signal(|| String::new());
    let mut signup_pass = use_signal(|| String::new());
    let mut login_name = use_signal(|| String::new());
    let mut login_pass = use_signal(|| String::new());
    let mut err = use_signal(|| None::<String>);
    let mut show_register = use_signal(|| false);
    let mut oauth_available = use_signal(|| false);

    use_effect(move || {
        spawn(async move {
            if let Ok(data) =
                graphql_exec_anonymous::<OauthAvailableData>("query { oauthAvailable }", None).await
            {
                oauth_available.set(data.oauth_available);
            }
        });
    });

    rsx! {
        div { class: "auth-page",
            div { class: "auth-hero",
                div { class: "auth-hero-glow" }
                div { class: "auth-hero-glow-secondary" }
                div { class: "auth-hero-inner",
                    div { class: "auth-logo-wrap",
                        img {
                            class: "auth-logo",
                            src: asset!("/assets/upjs-fp-logo.svg"),
                            alt: "UPJŠ Faculty of Philosophy logo",
                            width: "48",
                            height: "48",
                        }
                    }
                    h1 { class: "auth-hero-title", "UPJŠ GDD Platform" }
                    p { class: "auth-hero-subtitle",
                        "Multiplayer game development platform for students. Join lobbies, play together, and publish your own games."
                    }
                    ul { class: "auth-hero-features",
                        li { class: "auth-hero-feature",
                            span { class: "status-dot-online" }
                            "Sign in with your account or Google"
                        }
                        li { class: "auth-hero-feature",
                            span { class: "status-dot-online" }
                            "Create an account to keep your progress"
                        }
                        li { class: "auth-hero-feature",
                            span { class: "status-dot-online" }
                            "Browse games and join live lobbies"
                        }
                    }
                }
            }

            div { class: "auth-panel",
                div { class: "auth-panel-inner",
                    if let Some(e) = err() {
                        ErrorBanner { message: e }
                    }

                    if oauth_available() {
                        a {
                            class: "auth-google-btn",
                            href: "/auth/google",
                            svg {
                                class: "auth-google-icon",
                                xmlns: "http://www.w3.org/2000/svg",
                                view_box: "0 0 48 48",
                                width: "20",
                                height: "20",
                                path { fill: "#EA4335", d: "M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z" }
                                path { fill: "#4285F4", d: "M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z" }
                                path { fill: "#FBBC05", d: "M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z" }
                                path { fill: "#34A853", d: "M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z" }
                            }
                            "Sign in with Google"
                        }
                        div { class: "auth-divider", "or" }
                    }

                    if !show_register() {
                        div { class: "auth-card",
                            div { class: "auth-card-header",
                                span { class: "auth-card-icon-muted",
                                    Icon { name: "login", filled: false }
                                }
                                div {
                                    h2 { class: "auth-card-heading", "Log in" }
                                    p { class: "auth-card-desc",
                                        "Welcome back — sign in with your display name and password."
                                    }
                                }
                            }
                            label { class: "field-label", r#for: "login-name", "Display name" }
                            input {
                                id: "login-name",
                                class: "input-field mb-3",
                                placeholder: "Your display name",
                                value: "{login_name}",
                                oninput: move |e| login_name.set(e.value()),
                            }
                            label { class: "field-label", r#for: "login-pass", "Password" }
                            input {
                                id: "login-pass",
                                class: "input-field mb-4",
                                r#type: "password",
                                placeholder: "Your password",
                                value: "{login_pass}",
                                oninput: move |e| login_pass.set(e.value()),
                            }
                            button {
                                class: "btn-primary w-full",
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
                                                on_ready.call(());
                                            }
                                            Err(e) => err.set(Some(e)),
                                        }
                                    });
                                },
                                Icon { name: "login", filled: false }
                                "Log in"
                            }
                        }

                        p { class: "auth-toggle-wrap",
                            "Don't have an account? "
                            button {
                                class: "auth-toggle-link",
                                r#type: "button",
                                onclick: move |_| {
                                    err.set(None);
                                    show_register.set(true);
                                },
                                "Create one"
                            }
                        }
                    } else {
                        div { class: "auth-card",
                            div { class: "auth-card-header",
                                span { class: "auth-card-icon-tertiary",
                                    Icon { name: "person_add", filled: false }
                                }
                                div {
                                    h2 { class: "auth-card-heading", "Create account" }
                                    p { class: "auth-card-desc",
                                        "Register to save your profile across sessions."
                                    }
                                }
                            }
                            label { class: "field-label", r#for: "signup-name", "Display name" }
                            input {
                                id: "signup-name",
                                class: "input-field mb-3",
                                placeholder: "Choose a display name",
                                value: "{signup_name}",
                                oninput: move |e| signup_name.set(e.value()),
                            }
                            label { class: "field-label", r#for: "signup-pass", "Password" }
                            input {
                                id: "signup-pass",
                                class: "input-field mb-4",
                                r#type: "password",
                                placeholder: "At least 8 characters",
                                value: "{signup_pass}",
                                oninput: move |e| signup_pass.set(e.value()),
                            }
                            button {
                                class: "btn-secondary w-full",
                                onclick: move |_| {
                                    let n = signup_name();
                                    let p = signup_pass();
                                    spawn(async move {
                                        let q = "mutation SignUp($n: String!, $p: String!) { signUp(displayName: $n, password: $p) { sessionToken user { id } } }";
                                        let vars = serde_json::json!({ "n": n, "p": p });
                                        match graphql_exec_anonymous::<SignUpData>(q, Some(vars)).await {
                                            Ok(w) => {
                                                store_auth_session(
                                                    &w.sign_up.session_token,
                                                    &w.sign_up.user.id,
                                                );
                                                on_ready.call(());
                                            }
                                            Err(e) => err.set(Some(e)),
                                        }
                                    });
                                },
                                Icon { name: "person_add", filled: false }
                                "Create account"
                            }
                        }

                        p { class: "auth-toggle-wrap",
                            "Already have an account? "
                            button {
                                class: "auth-toggle-link",
                                r#type: "button",
                                onclick: move |_| {
                                    err.set(None);
                                    show_register.set(false);
                                },
                                "Log in"
                            }
                        }
                    }
                }
            }
        }
    }
}
