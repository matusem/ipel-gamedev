use crate::api::*;
use crate::components::ui::{ErrorBanner, Icon};
use crate::models::{LoginData, RegisterUserData, SignUpData};
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
        div { class: "auth-page",
            // Branded intro / hero
            div { class: "auth-hero",
                div { class: "auth-hero-glow" }
                div { class: "auth-hero-glow-secondary" }
                div { class: "auth-hero-inner",
                    div { class: "auth-logo-wrap",
                        img {
                            class: "auth-logo",
                            src: asset!("/assets/upjs-fp-logo.svg"),
                            alt: "UPJŠ Faculty of Philosophy logo",
                        }
                    }
                    h1 { class: "auth-hero-title", "UPJŠ GDD Platform" }
                    p { class: "auth-hero-subtitle",
                        "Multiplayer game development platform for students. Join lobbies, play together, and publish your own games."
                    }
                    ul { class: "auth-hero-features",
                        li { class: "auth-hero-feature",
                            span { class: "status-dot-online" }
                            "Jump in instantly as a guest"
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

            // Auth forms
            div { class: "auth-panel",
                div { class: "auth-panel-inner",
                    if let Some(e) = err() {
                        ErrorBanner { message: e }
                    }

                    // Quick guest path
                    div { class: "auth-card-highlight",
                        div { class: "auth-card-header",
                            span { class: "auth-card-icon-primary",
                                Icon { name: "person", filled: false }
                            }
                            div {
                                h2 { class: "auth-card-heading", "Continue as guest" }
                                p { class: "auth-card-desc",
                                    "No account needed — pick a display name and start playing."
                                }
                            }
                        }
                        label { class: "field-label", r#for: "guest-name", "Display name" }
                        input {
                            id: "guest-name",
                            class: "input-field mb-3",
                            placeholder: "Guest",
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
                            Icon { name: "play_arrow", filled: true }
                            "Continue"
                        }
                    }

                    div { class: "auth-divider", "Account" }

                    // Returning user login
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

                    // Sign up
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
                                            store_auth_session(&w.sign_up.session_token, &w.sign_up.user.id);
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
                }
            }
        }
    }
}
