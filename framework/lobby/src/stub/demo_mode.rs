use crate::models::{SESSION_TOKEN_KEY, USER_ID_KEY};

pub const DEMO_MODE_KEY: &str = "upjs_gdd_demo_mode";
pub const DEMO_USER_ID: &str = "demo-user-nova";

pub fn is_demo_mode() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(DEMO_MODE_KEY).ok().flatten())
        .is_some_and(|v| v == "1")
}

pub fn set_demo_mode(on: bool) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        if on {
            let _ = storage.set_item(DEMO_MODE_KEY, "1");
        } else {
            let _ = storage.remove_item(DEMO_MODE_KEY);
        }
    }
}

pub fn toggle_demo_mode_and_reload() {
    if is_demo_mode() {
        set_demo_mode(false);
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            if storage
                .get_item(USER_ID_KEY)
                .ok()
                .flatten()
                .as_deref()
                == Some(DEMO_USER_ID)
            {
                let _ = storage.remove_item(USER_ID_KEY);
                let _ = storage.remove_item(SESSION_TOKEN_KEY);
            }
        }
    } else {
        enter_demo_mode();
        return;
    }
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}

/// Enable demo mode, seed a demo user id, and reload into the rich synthetic dataset.
pub fn enter_demo_mode() {
    set_demo_mode(true);
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(USER_ID_KEY, DEMO_USER_ID);
    }
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}
